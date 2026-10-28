//! แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น) การตรวจสิทธิ์และตรวจค่า
//! จริงอยู่ที่ service — ที่นี่แค่ extract CurrentUser แล้วส่งต่อ

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::state::AppState;
use crate::stock::dto::{
    BudgetYearOption, CancelReceiptRequest, CreateReceiptRequest, CreatedReceiptResponse,
    ItemOption, LotResponse, LotsQuery, OptionsResponse, ReceiptDetailResponse,
    ReceiptLineResponse, ReceiptResponse, UnitOption, UpdateReceiptRequest,
};
use crate::stock::repository::ReceiptRow;
use crate::stock::service::{self, HeaderInput, ReceiptInput, ReceiptLineInput};

fn to_response(row: ReceiptRow) -> ReceiptResponse {
    ReceiptResponse {
        id: row.public_id,
        doc_no: row.doc_no,
        budget_year_id: row.budget_year_public_id,
        budget_year: row.budget_year,
        receipt_type: row.receipt_type,
        received_on: row.received_on,
        source: row.source,
        ref_no: row.ref_no,
        note: row.note,
        line_count: row.line_count,
        created_by_name: row.created_by_name,
        created_at: row.created_at,
        cancelled_by_name: row.cancelled_by_name,
        cancelled_at: row.cancelled_at,
        cancel_reason: row.cancel_reason,
    }
}

#[tracing::instrument(skip(state, current_user))]
pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<ReceiptResponse>>, AppError> {
    let rows = service::list(&state.pool, &current_user).await?;
    Ok(Json(rows.into_iter().map(to_response).collect()))
}

#[tracing::instrument(skip(state, current_user))]
pub async fn detail(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<ReceiptDetailResponse>, AppError> {
    let view = service::get(&state.pool, &current_user, id).await?;
    Ok(Json(ReceiptDetailResponse {
        receipt: to_response(view.receipt),
        lines: view
            .lines
            .into_iter()
            .map(|line| ReceiptLineResponse {
                item_id: line.item_public_id,
                item_name: line.item_name,
                lot_no: line.lot_no,
                expiry_date: line.expiry_date,
                unit_name: line.unit_name,
                unit_factor: line.unit_factor,
                quantity: line.quantity,
                base_quantity: line.base_quantity,
                base_unit_name: line.base_unit_name,
            })
            .collect(),
    }))
}

#[tracing::instrument(skip(state, current_user))]
pub async fn options(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<OptionsResponse>, AppError> {
    let options = service::options(&state.pool, &current_user).await?;
    Ok(Json(OptionsResponse {
        items: options
            .items
            .into_iter()
            .map(|view| ItemOption {
                id: view.item.public_id,
                name: view.item.name,
                category_name: view.item.category_name,
                base_unit_id: view.item.base_unit_public_id,
                base_unit_name: view.item.base_unit_name,
                track_lot: view.item.track_lot,
                track_expiry: view.item.track_expiry,
                conversions: view
                    .conversions
                    .into_iter()
                    .map(|c| UnitOption {
                        id: c.unit_public_id,
                        name: c.unit_name,
                        factor: c.factor,
                    })
                    .collect(),
            })
            .collect(),
        budget_years: options
            .budget_years
            .into_iter()
            .map(|b| BudgetYearOption {
                id: b.public_id,
                year: b.year,
                start_date: b.start_date,
                end_date: b.end_date,
            })
            .collect(),
        sources: options.sources,
    }))
}

#[tracing::instrument(skip(state, current_user))]
pub async fn lots(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Query(query): Query<LotsQuery>,
) -> Result<Json<Vec<LotResponse>>, AppError> {
    let rows = service::lots(&state.pool, &current_user, query.item).await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| LotResponse {
                lot_no: row.lot_no,
                expiry_date: row.expiry_date,
                qty_on_hand: row.qty_on_hand,
            })
            .collect(),
    ))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<CreateReceiptRequest>,
) -> Result<(StatusCode, Json<CreatedReceiptResponse>), AppError> {
    let input = ReceiptInput {
        receipt_type: payload.receipt_type,
        budget_year_id: payload.budget_year_id,
        received_on: payload.received_on,
        source: payload.source.as_deref(),
        ref_no: payload.ref_no.as_deref(),
        note: payload.note.as_deref(),
        lines: payload
            .lines
            .iter()
            .map(|line| ReceiptLineInput {
                item_id: line.item_id,
                unit_id: line.unit_id,
                quantity: line.quantity,
                lot_no: line.lot_no.as_deref(),
                expiry_date: line.expiry_date,
            })
            .collect(),
    };
    let created = service::create(&state.pool, &current_user, input).await?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedReceiptResponse {
            id: created.public_id,
            doc_no: created.doc_no,
        }),
    ))
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn update(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateReceiptRequest>,
) -> Result<StatusCode, AppError> {
    let input = HeaderInput {
        source: payload.source.as_deref(),
        ref_no: payload.ref_no.as_deref(),
        note: payload.note.as_deref(),
    };
    service::update_header(&state.pool, &current_user, id, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip(state, current_user, payload))]
pub async fn cancel(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<CancelReceiptRequest>,
) -> Result<StatusCode, AppError> {
    service::cancel(&state.pool, &current_user, id, &payload.reason).await?;
    Ok(StatusCode::NO_CONTENT)
}
