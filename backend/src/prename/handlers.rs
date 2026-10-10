//! แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น) การตรวจสิทธิ์และตรวจค่า
//! จริงอยู่ที่ service — ที่นี่แค่ extract CurrentUser แล้วส่งต่อ

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::prename::dto::{CreatePrenameRequest, PrenameResponse, UpdatePrenameRequest};
use crate::prename::repository::PrenameRow;
use crate::prename::service::{self, CreatePrenameInput, UpdatePrenameInput};
use crate::state::AppState;

fn to_response(row: PrenameRow) -> PrenameResponse {
    PrenameResponse {
        id: row.public_id,
        name: row.name,
        abbreviation: row.abbreviation,
        sort_order: row.sort_order,
    }
}

#[tracing::instrument(skip(state, current_user))]
pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<PrenameResponse>>, AppError> {
    let rows = service::list(&state.pool, &current_user).await?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<CreatePrenameRequest>,
) -> Result<StatusCode, AppError> {
    service::create(
        &state.pool,
        &current_user,
        CreatePrenameInput {
            name: &payload.name,
            abbreviation: payload.abbreviation.as_deref(),
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
    Json(payload): Json<UpdatePrenameRequest>,
) -> Result<StatusCode, AppError> {
    service::update(
        &state.pool,
        &current_user,
        id,
        UpdatePrenameInput {
            name: payload.name.as_deref(),
            abbreviation: payload.abbreviation.as_deref(),
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
