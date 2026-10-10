//! แปลงผลลัพธ์และ error เป็น HTTP (ห้ามแตะ SQL — เรียก service เท่านั้น) การตรวจสิทธิ์และตรวจค่า
//! จริงอยู่ที่ service — ที่นี่แค่ extract CurrentUser, หา "วันนี้" ตามเวลาไทย แล้วส่งต่อ
//!
//! ทุก handler ใช้ `#[tracing::instrument(skip_all)]` — ไม่ log argument ใด ๆ (payload มีข้อมูลระบุตัวคนไข้)

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, NaiveDate, Utc};
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::patient::dto::{
    ConditionResponse, DiseaseOption, NationalityOption, OptionsResponse, PatientRequest,
    PatientResponse, PrenameOption,
};
use crate::patient::service::{self, PatientInput, PatientView};
use crate::state::AppState;

/// วันนี้ตามเวลาไทย (UTC+7) — ใช้คำนวณอายุ/ตรวจวันเกิด เวลาในระบบเก็บ UTC แต่ "วัน" ของคนไข้คือวันไทย
fn today_in_thailand() -> NaiveDate {
    (Utc::now() + Duration::hours(7)).date_naive()
}

fn to_response(view: PatientView) -> PatientResponse {
    PatientResponse {
        id: view.public_id,
        hn: view.hn,
        nap_no: view.nap_no,
        prename_id: view.prename_public_id,
        prename_name: view.prename_name,
        first_name: view.first_name,
        last_name: view.last_name,
        birth_date: view.birth_date,
        age: view.age,
        weight: view.weight,
        height: view.height,
        sex: view.sex,
        is_thai: view.is_thai,
        nationality_id: view.nationality_public_id,
        nationality_name: view.nationality_name,
        sort_order: view.sort_order,
        is_active: view.is_active,
        conditions: view
            .conditions
            .into_iter()
            .map(|c| ConditionResponse {
                id: c.public_id,
                disease_id: c.disease_public_id,
                name: c.name,
                abbreviation: c.abbreviation,
            })
            .collect(),
    }
}

fn to_input(payload: &PatientRequest) -> PatientInput<'_> {
    PatientInput {
        hn: &payload.hn,
        nap_no: &payload.nap_no,
        prename_id: payload.prename_id,
        first_name: &payload.first_name,
        last_name: &payload.last_name,
        birth_date: payload.birth_date,
        age: payload.age,
        weight: payload.weight,
        height: payload.height,
        sex: payload.sex,
        is_thai: payload.is_thai,
        nationality_id: payload.nationality_id,
        sort_order: payload.sort_order,
        is_active: payload.is_active.unwrap_or(true),
        disease_ids: &payload.disease_ids,
    }
}

#[tracing::instrument(skip_all)]
pub async fn list(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<Vec<PatientResponse>>, AppError> {
    let views = service::list(
        &state.pool,
        &state.config.crypto,
        &current_user,
        today_in_thailand(),
    )
    .await?;
    Ok(Json(views.into_iter().map(to_response).collect()))
}

#[tracing::instrument(skip_all)]
pub async fn options(
    State(state): State<AppState>,
    current_user: CurrentUser,
) -> Result<Json<OptionsResponse>, AppError> {
    let options = service::options(&state.pool, &current_user).await?;
    Ok(Json(OptionsResponse {
        prenames: options
            .prenames
            .into_iter()
            .map(|r| PrenameOption {
                id: r.public_id,
                name: r.name,
                abbreviation: r.abbreviation,
            })
            .collect(),
        nationalities: options
            .nationalities
            .into_iter()
            .map(|r| NationalityOption {
                id: r.public_id,
                national_name: r.national_name,
                nation: r.nation,
            })
            .collect(),
        diseases: options
            .diseases
            .into_iter()
            .map(|r| DiseaseOption {
                id: r.public_id,
                name: r.name,
                abbreviation: r.abbreviation,
            })
            .collect(),
    }))
}

#[tracing::instrument(skip_all)]
pub async fn create(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Json(payload): Json<PatientRequest>,
) -> Result<StatusCode, AppError> {
    service::create(
        &state.pool,
        &state.config.crypto,
        &current_user,
        to_input(&payload),
        today_in_thailand(),
    )
    .await?;
    Ok(StatusCode::CREATED)
}

#[tracing::instrument(skip_all)]
pub async fn replace(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<PatientRequest>,
) -> Result<StatusCode, AppError> {
    service::replace(
        &state.pool,
        &state.config.crypto,
        &current_user,
        id,
        to_input(&payload),
        today_in_thailand(),
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[tracing::instrument(skip_all)]
pub async fn delete(
    State(state): State<AppState>,
    current_user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    service::delete(&state.pool, &current_user, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
