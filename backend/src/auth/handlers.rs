//! ตรวจรูปแบบ input, แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น)

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
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

    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok());

    let result = service::login(
        &state.pool,
        &state.dummy_password_hash,
        &state.config,
        service::LoginParams {
            username: &payload.username,
            password: &payload.password,
            user_agent,
            now: Utc::now(),
        },
    )
    .await?;

    let jar = jar.add(session::build_cookie(result.session_token));
    let response = UserResponse::from_domain(&result.user, &result.permissions);

    Ok((jar, Json(response)))
}

#[tracing::instrument(skip(state, jar))]
pub async fn logout(State(state): State<AppState>, jar: CookieJar) -> Result<CookieJar, AppError> {
    if let Some(cookie) = jar.get(session::COOKIE_NAME) {
        let token_hash = session::hash_token(cookie.value());
        service::logout(&state.pool, &token_hash).await?;
    }
    Ok(jar.remove(session::removal_marker()))
}

#[tracing::instrument(skip(state, current_user, jar))]
pub async fn logout_all(
    State(state): State<AppState>,
    current_user: CurrentUser,
    jar: CookieJar,
) -> Result<CookieJar, AppError> {
    service::logout_all(&state.pool, current_user.user.id).await?;
    Ok(jar.remove(session::removal_marker()))
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
