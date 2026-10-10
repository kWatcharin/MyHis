//! แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น) การตรวจสิทธิ์และตรวจค่า
//! จริงอยู่ที่ service — ที่นี่แค่ extract CurrentUser แล้วส่งต่อ

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::nationality::dto::{
    CreateNationalityRequest, NationalityResponse, UpdateNationalityRequest,
};
use crate::nationality::repository::NationalityRow;
use crate::nationality::service::{self, CreateNationalityInput, UpdateNationalityInput};
use crate::state::AppState;

fn to_response(row: NationalityRow) -> NationalityResponse {
    NationalityResponse {
        id: row.public_id,
        national_name: row.national_name,
        nation: row.nation,
        sort_order: row.sort_order,
        is_active: row.is_active,
    }
}

#[tracing::instrument(skip(state, current_user))]
pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<NationalityResponse>>, AppError> {
    let rows = service::list(&state.pool, &current_user).await?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<CreateNationalityRequest>,
) -> Result<StatusCode, AppError> {
    service::create(
        &state.pool,
        &current_user,
        CreateNationalityInput {
            national_name: &payload.national_name,
            nation: &payload.nation,
            sort_order: payload.sort_order.unwrap_or(0),
            is_active: payload.is_active.unwrap_or(true),
        },
    )
    .await?;
    Ok(StatusCode::CREATED)
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn update(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateNationalityRequest>,
) -> Result<StatusCode, AppError> {
    service::update(
        &state.pool,
        &current_user,
        id,
        UpdateNationalityInput {
            national_name: payload.national_name.as_deref(),
            nation: payload.nation.as_deref(),
            sort_order: payload.sort_order,
            is_active: payload.is_active,
        },
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip(state, current_user))]
pub async fn delete(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    service::delete(&state.pool, &current_user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
