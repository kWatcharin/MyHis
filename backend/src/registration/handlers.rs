//! ตรวจรูปแบบ input, แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น)
//! endpoint สาธารณะ (ไม่มี CurrentUser) — token ในลิงก์คือสิ่งที่ยืนยันว่าเป็นเจ้าของบัญชี
//! `skip_all` ทุกตัว: body มี token และรหัสผ่าน ห้ามเข้า span/log

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use validator::Validate;

use crate::error::AppError;
use crate::registration::dto::{CompleteRequest, LookupRequest, LookupResponse, PrenameOption};
use crate::registration::service::{self, CompleteInput};
use crate::state::AppState;

#[tracing::instrument(skip_all)]
pub async fn lookup(
    State(state): State<AppState>,
    Json(payload): Json<LookupRequest>,
) -> Result<Json<LookupResponse>, AppError> {
    let info = service::lookup(&state.pool, &payload.token, Utc::now()).await?;
    Ok(Json(LookupResponse {
        username: info.username,
        display_name: info.display_name,
        mode: info.mode.as_str(),
        expires_at: info.expires_at,
        prename_id: info.prename_public_id,
        first_name: info.first_name,
        last_name: info.last_name,
        position: info.position,
        prenames: info
            .prenames
            .into_iter()
            .map(|prename| PrenameOption {
                id: prename.public_id,
                name: prename.name,
            })
            .collect(),
    }))
}

#[tracing::instrument(skip_all)]
pub async fn complete(
    State(state): State<AppState>,
    Json(payload): Json<CompleteRequest>,
) -> Result<StatusCode, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    service::complete(
        &state.pool,
        CompleteInput {
            token: &payload.token,
            new_password: &payload.new_password,
            prename_public_id: payload.prename_id,
            first_name: payload.first_name.as_deref(),
            last_name: payload.last_name.as_deref(),
            position: payload.position.as_deref(),
        },
        Utc::now(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
