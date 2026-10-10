//! ตรวจรูปแบบ input, แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น)
//! การตรวจสิทธิ์จริงอยู่ที่ service — ที่นี่แค่ extract CurrentUser แล้วส่งต่อ

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;
use validator::Validate;

use crate::auth::CurrentUser;
use crate::budget_year::dto::{
    BudgetYearResponse, CreateBudgetYearRequest, UpdateBudgetYearRequest,
};
use crate::budget_year::repository::BudgetYearRow;
use crate::budget_year::service::{self, CreateBudgetYearInput, UpdateBudgetYearInput};
use crate::error::AppError;
use crate::state::AppState;

fn to_response(row: BudgetYearRow) -> BudgetYearResponse {
    BudgetYearResponse {
        id: row.public_id,
        year: row.year,
        start_date: row.start_date,
        end_date: row.end_date,
        sort_order: row.sort_order,
        is_active: row.is_active,
    }
}

#[tracing::instrument(skip(state, current_user))]
pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<BudgetYearResponse>>, AppError> {
    let rows = service::list(&state.pool, &current_user).await?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<CreateBudgetYearRequest>,
) -> Result<StatusCode, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    service::create(
        &state.pool,
        &current_user,
        CreateBudgetYearInput {
            year: &payload.year,
            start_date: payload.start_date,
            end_date: payload.end_date,
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
    Json(payload): Json<UpdateBudgetYearRequest>,
) -> Result<StatusCode, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    service::update(
        &state.pool,
        &current_user,
        id,
        UpdateBudgetYearInput {
            year: payload.year.as_deref(),
            start_date: payload.start_date,
            end_date: payload.end_date,
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
