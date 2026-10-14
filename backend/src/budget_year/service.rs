//! กฎของ budget_year: สิทธิ์ (`budget_year.*` ราย action), รูปแบบปี, ช่วงวันที่ถูกต้อง, ปีซ้ำ, ห้ามลบปีที่ยังมี
//! นัดหมาย lab ใช้อยู่ ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 10)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็คสิทธิ์
//! ของ action นั้น (`budget_year.view/create/update/delete`) ที่นี่ เหมือนโมดูล users
//! — id ขาเข้าเป็น uuid (`public_id`) แล้ว resolve เป็น id ภายในเอง

use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::budget_year::repository::{
    self, BudgetYearRow, RepositoryError, UpdateBudgetYearFields,
};
use crate::domain::{Action, Resource};

#[derive(Debug, thiserror::Error)]
pub enum BudgetYearError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("budget year not found")]
    NotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// ตรวจสิทธิ์ของ action นี้กับหน้านี้ (`budget_year.view/create/update/delete`)
fn require(actor: &CurrentUser, action: Action) -> Result<(), BudgetYearError> {
    if actor.can(Resource::BudgetYear, action) {
        Ok(())
    } else {
        Err(BudgetYearError::Forbidden)
    }
}

fn map_unique_violation(err: RepositoryError) -> BudgetYearError {
    if err.is_unique_violation() {
        BudgetYearError::Conflict("year already taken".to_string())
    } else {
        BudgetYearError::from(err)
    }
}

fn validate_year(year: &str) -> Result<(), BudgetYearError> {
    if year.len() == 4 && year.bytes().all(|b| b.is_ascii_digit()) {
        Ok(())
    } else {
        Err(BudgetYearError::Validation(
            "year must be 4 digits".to_string(),
        ))
    }
}

fn validate_range(start_date: NaiveDate, end_date: NaiveDate) -> Result<(), BudgetYearError> {
    if end_date < start_date {
        Err(BudgetYearError::Validation(
            "end date must not be before start date".to_string(),
        ))
    } else {
        Ok(())
    }
}

pub async fn list(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<BudgetYearRow>, BudgetYearError> {
    require(actor, Action::View)?;
    Ok(repository::list_budget_years(pool).await?)
}

pub struct CreateBudgetYearInput<'a> {
    pub year: &'a str,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub sort_order: i32,
    pub is_active: bool,
}

pub async fn create(
    pool: &PgPool,
    actor: &CurrentUser,
    input: CreateBudgetYearInput<'_>,
) -> Result<(), BudgetYearError> {
    require(actor, Action::Create)?;
    validate_year(input.year)?;
    validate_range(input.start_date, input.end_date)?;

    repository::insert_budget_year(
        pool,
        actor.user.id,
        input.year,
        input.start_date,
        input.end_date,
        input.sort_order,
        input.is_active,
    )
    .await
    .map_err(map_unique_violation)?;
    Ok(())
}

pub struct UpdateBudgetYearInput<'a> {
    pub year: Option<&'a str>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

pub async fn update(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: UpdateBudgetYearInput<'_>,
) -> Result<(), BudgetYearError> {
    require(actor, Action::Update)?;

    let existing = repository::find_budget_year_by_public_id(pool, public_id)
        .await?
        .ok_or(BudgetYearError::NotFound)?;

    if let Some(year) = input.year {
        validate_year(year)?;
    }
    // ส่งมาฝั่งเดียวก็ต้องเช็คกับค่าเดิมของอีกฝั่ง ไม่งั้นแก้ทีละช่องแล้วช่วงวันที่กลับด้านได้
    validate_range(
        input.start_date.unwrap_or(existing.start_date),
        input.end_date.unwrap_or(existing.end_date),
    )?;

    repository::update_budget_year(
        pool,
        actor.user.id,
        existing.id,
        UpdateBudgetYearFields {
            year: input.year,
            start_date: input.start_date,
            end_date: input.end_date,
            sort_order: input.sort_order,
            is_active: input.is_active,
        },
    )
    .await
    .map_err(map_unique_violation)?;
    Ok(())
}

pub async fn delete(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
) -> Result<(), BudgetYearError> {
    require(actor, Action::Delete)?;

    let existing = repository::find_budget_year_by_public_id(pool, public_id)
        .await?
        .ok_or(BudgetYearError::NotFound)?;
    if repository::budget_year_in_use(pool, existing.id).await? {
        return Err(BudgetYearError::Conflict(
            "budget year is still used".to_string(),
        ));
    }

    repository::soft_delete_budget_year(pool, actor.user.id, existing.id).await?;
    Ok(())
}
