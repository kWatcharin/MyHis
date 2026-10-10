//! แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น) การตรวจสิทธิ์และตรวจค่า
//! จริงอยู่ที่ service — ที่นี่แค่ extract CurrentUser แล้วส่งต่อ

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::lab::dto::{CreateLabRequest, LabResponse, UpdateLabRequest};
use crate::lab::repository::LabRow;
use crate::lab::service::{self, CreateLabInput, UpdateLabInput};
use crate::state::AppState;

fn to_response(row: LabRow) -> LabResponse {
    LabResponse {
        id: row.public_id,
        name: row.name,
        abbreviation: row.abbreviation,
        is_active: row.is_active,
        sort_order: row.sort_order,
    }
}

#[tracing::instrument(skip(state, current_user))]
pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<LabResponse>>, AppError> {
    let rows = service::list(&state.pool, &current_user).await?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<CreateLabRequest>,
) -> Result<StatusCode, AppError> {
    service::create(
        &state.pool,
        &current_user,
        CreateLabInput {
            name: &payload.name,
            abbreviation: payload.abbreviation.as_deref(),
            is_active: payload.is_active.unwrap_or(true),
            sort_order: payload.sort_order.unwrap_or(0),
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
    Json(payload): Json<UpdateLabRequest>,
) -> Result<StatusCode, AppError> {
    service::update(
        &state.pool,
        &current_user,
        id,
        UpdateLabInput {
            name: payload.name.as_deref(),
            abbreviation: payload.abbreviation.as_deref(),
            is_active: payload.is_active,
            sort_order: payload.sort_order,
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
