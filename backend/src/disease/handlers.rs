//! แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น) การตรวจสิทธิ์และตรวจค่า
//! จริงอยู่ที่ service — ที่นี่แค่ extract CurrentUser แล้วส่งต่อ

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::disease::dto::{CreateDiseaseRequest, DiseaseResponse, UpdateDiseaseRequest};
use crate::disease::repository::DiseaseRow;
use crate::disease::service::{self, CreateDiseaseInput, UpdateDiseaseInput};
use crate::error::AppError;
use crate::state::AppState;

fn to_response(row: DiseaseRow) -> DiseaseResponse {
    DiseaseResponse {
        id: row.public_id,
        name: row.name,
        abbreviation: row.abbreviation,
        sort_order: row.sort_order,
        is_active: row.is_active,
    }
}

#[tracing::instrument(skip(state, current_user))]
pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<DiseaseResponse>>, AppError> {
    let rows = service::list(&state.pool, &current_user).await?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<CreateDiseaseRequest>,
) -> Result<StatusCode, AppError> {
    service::create(
        &state.pool,
        &current_user,
        CreateDiseaseInput {
            name: &payload.name,
            abbreviation: &payload.abbreviation,
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
    Json(payload): Json<UpdateDiseaseRequest>,
) -> Result<StatusCode, AppError> {
    service::update(
        &state.pool,
        &current_user,
        id,
        UpdateDiseaseInput {
            name: payload.name.as_deref(),
            abbreviation: payload.abbreviation.as_deref(),
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
