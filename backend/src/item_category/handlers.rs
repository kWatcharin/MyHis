//! แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น) การตรวจสิทธิ์และตรวจค่า
//! จริงอยู่ที่ service — ที่นี่แค่ extract CurrentUser แล้วส่งต่อ

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::item_category::dto::{
    CreateItemCategoryRequest, ItemCategoryResponse, UpdateItemCategoryRequest,
};
use crate::item_category::repository::ItemCategoryRow;
use crate::item_category::service::{self, CreateItemCategoryInput, UpdateItemCategoryInput};
use crate::state::AppState;

fn to_response(row: ItemCategoryRow) -> ItemCategoryResponse {
    ItemCategoryResponse {
        id: row.public_id,
        name: row.name,
        default_track_lot: row.default_track_lot,
        default_track_expiry: row.default_track_expiry,
        default_patient_linkable: row.default_patient_linkable,
        sort_order: row.sort_order,
        is_active: row.is_active,
    }
}

#[tracing::instrument(skip(state, current_user))]
pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<ItemCategoryResponse>>, AppError> {
    let rows = service::list(&state.pool, &current_user).await?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<CreateItemCategoryRequest>,
) -> Result<StatusCode, AppError> {
    service::create(
        &state.pool,
        &current_user,
        CreateItemCategoryInput {
            name: &payload.name,
            default_track_lot: payload.default_track_lot.unwrap_or(false),
            default_track_expiry: payload.default_track_expiry.unwrap_or(false),
            default_patient_linkable: payload.default_patient_linkable.unwrap_or(false),
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
    Json(payload): Json<UpdateItemCategoryRequest>,
) -> Result<StatusCode, AppError> {
    service::update(
        &state.pool,
        &current_user,
        id,
        UpdateItemCategoryInput {
            name: payload.name.as_deref(),
            default_track_lot: payload.default_track_lot,
            default_track_expiry: payload.default_track_expiry,
            default_patient_linkable: payload.default_patient_linkable,
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
