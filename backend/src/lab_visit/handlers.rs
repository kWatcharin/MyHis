//! แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น) การตรวจสิทธิ์และตรวจค่าจริง
//! อยู่ที่ service — ที่นี่แค่ extract CurrentUser/path/body แล้วส่งต่อ
//!
//! ทุก handler ใช้ `#[tracing::instrument(skip_all)]` — ไม่ log argument ใด ๆ (body มีผล VL/สูตรยา)

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::lab_visit::dto::{
    AddVisitsRequest, BudgetYearOption, CreatedResponse, ExportRequest, GroupDetailResponse,
    GroupRequest, GroupResponse, OptionsResponse, PatientGroupOption, VisitRequest, VisitResponse,
};
use crate::lab_visit::repository::{GroupRow, VisitRow};
use crate::lab_visit::service::{self, GroupInput, VisitInput};
use crate::state::AppState;

fn to_group_response(row: GroupRow) -> GroupResponse {
    GroupResponse {
        id: row.public_id,
        budget_year_id: row.budget_year_public_id,
        budget_year: row.budget_year,
        patient_group_id: row.patient_group_public_id,
        patient_group_name: row.patient_group_name,
        cd4_first_date: row.cd4_first_date,
        cd4_second_date: row.cd4_second_date,
        vl_date: row.vl_date,
        sort_order: row.sort_order,
        patient_count: row.patient_count,
    }
}

fn to_visit_response(row: VisitRow) -> VisitResponse {
    VisitResponse {
        id: row.public_id,
        patient_id: row.patient_public_id,
        take_med_year_range: row.take_med_year_range,
        take_med_month_range: row.take_med_month_range,
        vl_result: row.vl_result,
        drug_recipe: row.drug_recipe,
        sort_order: row.sort_order,
    }
}

fn to_group_input(payload: &GroupRequest) -> GroupInput {
    GroupInput {
        budget_year_id: payload.budget_year_id,
        patient_group_id: payload.patient_group_id,
        cd4_first_date: payload.cd4_first_date,
        cd4_second_date: payload.cd4_second_date,
        vl_date: payload.vl_date,
        sort_order: payload.sort_order,
    }
}

#[tracing::instrument(skip_all)]
pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<GroupResponse>>, AppError> {
    let rows = service::list_groups(&state.pool, &current_user).await?;
    Ok(Json(rows.into_iter().map(to_group_response).collect()))
}

#[tracing::instrument(skip_all)]
pub async fn options(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<OptionsResponse>, AppError> {
    let options = service::options(&state.pool, &current_user).await?;
    Ok(Json(OptionsResponse {
        budget_years: options
            .budget_years
            .into_iter()
            .map(|r| BudgetYearOption {
                id: r.public_id,
                year: r.year,
                start_date: r.start_date,
                end_date: r.end_date,
                is_active: r.is_active,
            })
            .collect(),
        patient_groups: options
            .patient_groups
            .into_iter()
            .map(|r| PatientGroupOption {
                id: r.public_id,
                name: r.name,
                is_active: r.is_active,
            })
            .collect(),
    }))
}

#[tracing::instrument(skip_all)]
pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<GroupRequest>,
) -> Result<(StatusCode, Json<CreatedResponse>), AppError> {
    let id = service::create_group(&state.pool, &current_user, to_group_input(&payload)).await?;
    Ok((StatusCode::CREATED, Json(CreatedResponse { id })))
}

#[tracing::instrument(skip_all)]
pub async fn get(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Json<GroupDetailResponse>, AppError> {
    let detail = service::get_group(&state.pool, &current_user, id).await?;
    Ok(Json(GroupDetailResponse {
        group: to_group_response(detail.group),
        visits: detail.visits.into_iter().map(to_visit_response).collect(),
    }))
}

#[tracing::instrument(skip_all)]
pub async fn replace(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<GroupRequest>,
) -> Result<StatusCode, AppError> {
    service::update_group(&state.pool, &current_user, id, to_group_input(&payload)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip_all)]
pub async fn delete(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    service::delete_group(&state.pool, &current_user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// บันทึกการส่งออกเอกสาร (หน้าเว็บเรียกก่อนสร้าง Excel/แสดงหน้าพิมพ์ PDF) — ตอบ 204
#[tracing::instrument(skip_all)]
pub async fn record_export(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<ExportRequest>,
) -> Result<StatusCode, AppError> {
    service::record_export(
        &state.pool,
        &current_user,
        id,
        &payload.document,
        &payload.format,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip_all)]
pub async fn add_visits(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<AddVisitsRequest>,
) -> Result<StatusCode, AppError> {
    service::add_visits(&state.pool, &current_user, id, &payload.patient_ids).await?;
    Ok(StatusCode::CREATED)
}

#[tracing::instrument(skip_all)]
pub async fn replace_visit(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<VisitRequest>,
) -> Result<StatusCode, AppError> {
    let input = VisitInput {
        take_med_year_range: payload.take_med_year_range,
        take_med_month_range: payload.take_med_month_range,
        vl_result: payload.vl_result.as_deref(),
        drug_recipe: payload.drug_recipe.as_deref(),
        sort_order: payload.sort_order,
    };
    service::update_visit(&state.pool, &current_user, id, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip_all)]
pub async fn delete_visit(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    service::delete_visit(&state.pool, &current_user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
