//! แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น) การตรวจสิทธิ์และตรวจค่า
//! จริงอยู่ที่ service — ที่นี่แค่ extract CurrentUser แล้วส่งต่อ

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::item::dto::{
    CategoryOption, ConversionResponse, ItemRequest, ItemResponse, OptionsResponse, UnitOption,
};
use crate::item::service::{self, ConversionInput, ItemInput, ItemView};
use crate::state::AppState;

fn to_response(view: ItemView) -> ItemResponse {
    let item = view.item;
    ItemResponse {
        id: item.public_id,
        name: item.name,
        category_id: item.category_public_id,
        category_name: item.category_name,
        base_unit_id: item.base_unit_public_id,
        base_unit_name: item.base_unit_name,
        track_lot: item.track_lot,
        track_expiry: item.track_expiry,
        patient_linkable: item.patient_linkable,
        reorder_point: item.reorder_point,
        is_active: item.is_active,
        conversions: view
            .conversions
            .into_iter()
            .map(|c| ConversionResponse {
                unit_id: c.unit_public_id,
                unit_name: c.unit_name,
                factor: c.factor,
            })
            .collect(),
    }
}

fn to_input(payload: &ItemRequest) -> ItemInput<'_> {
    ItemInput {
        name: &payload.name,
        category_id: payload.category_id,
        base_unit_id: payload.base_unit_id,
        track_lot: payload.track_lot,
        track_expiry: payload.track_expiry,
        patient_linkable: payload.patient_linkable,
        reorder_point: payload.reorder_point,
        is_active: payload.is_active.unwrap_or(true),
        conversions: payload
            .conversions
            .iter()
            .map(|c| ConversionInput {
                unit_id: c.unit_id,
                factor: c.factor,
            })
            .collect(),
    }
}

#[tracing::instrument(skip(state, current_user))]
pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<ItemResponse>>, AppError> {
    let views = service::list(&state.pool, &current_user).await?;
    Ok(Json(views.into_iter().map(to_response).collect()))
}

#[tracing::instrument(skip(state, current_user))]
pub async fn options(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<OptionsResponse>, AppError> {
    let options = service::options(&state.pool, &current_user).await?;
    Ok(Json(OptionsResponse {
        categories: options
            .categories
            .into_iter()
            .map(|c| CategoryOption {
                id: c.public_id,
                name: c.name,
                default_track_lot: c.default_track_lot,
                default_track_expiry: c.default_track_expiry,
                default_patient_linkable: c.default_patient_linkable,
            })
            .collect(),
        units: options
            .units
            .into_iter()
            .map(|u| UnitOption {
                id: u.public_id,
                name: u.name,
            })
            .collect(),
    }))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<ItemRequest>,
) -> Result<StatusCode, AppError> {
    service::create(&state.pool, &current_user, to_input(&payload)).await?;
    Ok(StatusCode::CREATED)
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn replace(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<ItemRequest>,
) -> Result<StatusCode, AppError> {
    service::replace(&state.pool, &current_user, id, to_input(&payload)).await?;
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
