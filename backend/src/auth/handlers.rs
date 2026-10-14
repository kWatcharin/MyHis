//! ตรวจรูปแบบ input, แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น)

use std::net::IpAddr;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::Json;
use axum_extra::extract::CookieJar;
use chrono::Utc;
use validator::Validate;

use crate::auth::dto::{ChangePasswordRequest, LoginRequest, UserResponse};
use crate::auth::{service, session, CurrentUser};
use crate::error::AppError;
use crate::state::AppState;

#[tracing::instrument(skip(state, jar, headers, payload))]
pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(payload): Json<LoginRequest>,
) -> Result<(CookieJar, Json<UserResponse>), AppError> {
    payload
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let result = service::login(
        &state.pool,
        &state.dummy_password_hash,
        &state.config,
        service::LoginParams {
            username: &payload.username,
            password: &payload.password,
            client: client_info(&headers),
            now: Utc::now(),
        },
    )
    .await?;

    let jar = jar.add(session::build_cookie(result.session_token));
    let response = UserResponse::from_domain(&result.user, &result.permissions);

    Ok((jar, Json(response)))
}

// headers ต้องอยู่ใน skip เสมอ — มี Cookie (session token) ห้ามเข้า span/log
#[tracing::instrument(skip(state, jar, headers))]
pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
) -> Result<CookieJar, AppError> {
    if let Some(cookie) = jar.get(session::COOKIE_NAME) {
        let token_hash = session::hash_token(cookie.value());
        service::logout(&state.pool, &token_hash, client_info(&headers)).await?;
    }
    Ok(jar.remove(session::removal_marker()))
}

#[tracing::instrument(skip(state, current_user, jar, headers))]
pub async fn logout_all(
    State(state): State<AppState>,
    current_user: CurrentUser,
    jar: CookieJar,
    headers: HeaderMap,
) -> Result<CookieJar, AppError> {
    service::logout_all(&state.pool, current_user.user.id, client_info(&headers)).await?;
    Ok(jar.remove(session::removal_marker()))
}

/// ความยาว User-Agent สูงสุดที่เก็บ (header ยาวผิดปกติไม่ให้บวม audit_log)
const MAX_USER_AGENT_CHARS: usize = 512;

/// IP + User-Agent จาก header สำหรับประวัติการเข้าออก — อ่านแค่สอง header นี้ ไม่แตะ Cookie
fn client_info(headers: &HeaderMap) -> service::ClientInfo<'_> {
    service::ClientInfo {
        ip: client_ip(headers),
        user_agent: headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .map(
                |value| match value.char_indices().nth(MAX_USER_AGENT_CHARS) {
                    Some((cut, _)) => &value[..cut],
                    None => value,
                },
            ),
    }
}

/// IP ของผู้ใช้ = ค่าขวาสุดของ X-Forwarded-For คือค่าที่ Caddy (proxy ชั้นเดียวหน้า backend) เติมจากการเชื่อมต่อจริง
/// ค่าทางซ้ายผู้ใช้ส่งมาเองได้จึงไม่เชื่อ — ถ้าวันหน้ามี proxy ซ้อนสองชั้น ตัวในต้องตั้ง trusted_proxies
/// ไม่มี header/แปลงไม่ได้ = None (เช่น เรียก backend ตรงไม่ผ่าน proxy)
fn client_ip(headers: &HeaderMap) -> Option<IpAddr> {
    headers
        .get_all("x-forwarded-for")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .next_back()
        .and_then(|last| last.trim().parse().ok())
}

#[tracing::instrument(skip(current_user))]
pub async fn me(current_user: CurrentUser) -> Json<UserResponse> {
    Json(UserResponse::from_domain(
        &current_user.user,
        &current_user.permissions,
    ))
}

#[tracing::instrument(skip(state, current_user, jar, payload))]
pub async fn change_password(
    State(state): State<AppState>,
    current_user: CurrentUser,
    jar: CookieJar,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<StatusCode, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let token_hash = jar
        .get(session::COOKIE_NAME)
        .map(|cookie| session::hash_token(cookie.value()))
        .ok_or(AppError::Unauthenticated)?;

    service::change_password(
        &state.pool,
        current_user.user.id,
        &payload.current_password,
        &payload.new_password,
        &token_hash,
    )
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(forwarded: &[&str]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for value in forwarded {
            map.append("x-forwarded-for", value.parse().unwrap());
        }
        map
    }

    #[test]
    fn client_ip_takes_the_rightmost_forwarded_address() {
        assert_eq!(client_ip(&headers(&[])), None);
        assert_eq!(
            client_ip(&headers(&["203.0.113.7"])),
            Some("203.0.113.7".parse().unwrap())
        );
        // ค่าซ้ายสุดผู้ใช้ปลอมมาได้ — ใช้ค่าที่ proxy เติมท้าย
        assert_eq!(
            client_ip(&headers(&["1.1.1.1, 203.0.113.7"])),
            Some("203.0.113.7".parse().unwrap())
        );
        assert_eq!(
            client_ip(&headers(&["1.1.1.1", "203.0.113.8"])),
            Some("203.0.113.8".parse().unwrap())
        );
        assert_eq!(
            client_ip(&headers(&["2001:db8::1"])),
            Some("2001:db8::1".parse().unwrap())
        );
        assert_eq!(client_ip(&headers(&["not-an-ip"])), None);
        assert_eq!(client_ip(&headers(&["203.0.113.7, unknown"])), None);
    }

    #[test]
    fn user_agent_is_capped() {
        let mut map = HeaderMap::new();
        map.insert(header::USER_AGENT, "a".repeat(2000).parse().unwrap());
        assert_eq!(
            client_info(&map).user_agent.map(str::len),
            Some(MAX_USER_AGENT_CHARS)
        );
    }
}
