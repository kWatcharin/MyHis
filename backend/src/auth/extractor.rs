//! อ่าน cookie, โหลด session, เตรียม CurrentUser — เงื่อนไขตาม docs/auth-design.md ข้อ 7
//! ห้าม query ที่ซับซ้อน (เรียก repository/service ที่มีอยู่แล้วเท่านั้น ไม่เขียน SQL ตรงนี้)
//!
//! `CurrentUser` คือโครงสร้างพื้นฐานเดียวที่โมดูลอื่นข้ามฟีเจอร์เรียกใช้ได้ (docs/rust-guidelines.md
//! — โมดูลฟีเจอร์ไม่เรียกกันเอง ยกเว้น auth::CurrentUser)

use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;
use chrono::{Duration, Utc};

use crate::auth::{repository, service, session};
use crate::domain::{Permission, User};
use crate::error::AppError;
use crate::state::AppState;

/// เกณฑ์เขียน DB สำหรับเลื่อน idle_expires_at — ไม่ต้องเขียนทุก request (docs/auth-design.md ข้อ 7)
const TOUCH_THRESHOLD: Duration = Duration::minutes(1);

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub user: User,
    pub permissions: Vec<Permission>,
}

impl CurrentUser {
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }
}

impl<S> FromRequestParts<S> for CurrentUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let jar = CookieJar::from_headers(&parts.headers);

        let token = jar
            .get(session::COOKIE_NAME)
            .map(|cookie| cookie.value().to_string())
            .ok_or(AppError::Unauthenticated)?;

        let token_hash = session::hash_token(&token);

        let session_row = repository::find_session_by_token_hash(&app_state.pool, &token_hash)
            .await?
            .ok_or(AppError::Unauthenticated)?;

        let now = Utc::now();
        if now > session_row.idle_expires_at || now > session_row.absolute_expires_at {
            return Err(AppError::Unauthenticated);
        }

        let Some((user, permissions)) =
            service::current_user_with_permissions(&app_state.pool, session_row.user_id).await?
        else {
            return Err(AppError::Unauthenticated);
        };

        if now - session_row.last_seen_at > TOUCH_THRESHOLD {
            let new_idle_expires_at = now + app_state.config.session_idle_ttl;
            // เขียนล้มเหลวไม่ควรทำให้ request ทั้งเส้นพัง — แค่ข้ามการเลื่อนอายุรอบนี้
            let _ =
                repository::touch_session(&app_state.pool, &token_hash, now, new_idle_expires_at)
                    .await;
        }

        Ok(CurrentUser { user, permissions })
    }
}
