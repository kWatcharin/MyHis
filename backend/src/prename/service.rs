//! กฎของ prename: สิทธิ์ (`prename.*` ราย action), ข้อความ (trim, ไม่ว่างถ้าบังคับ, ไม่เกิน 100 ตัวอักษร),
//! ชื่อซ้ำ, ห้ามลบข้อมูลที่ยังถูกใช้อยู่ ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 10)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็คสิทธิ์
//! ของ action นั้น (`prename.view/create/update/delete`) ที่นี่ เหมือนโมดูล users/disease
//! — id ขาเข้าเป็น uuid (`public_id`) แล้ว resolve เป็น id ภายในเอง

use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::domain::{Action, Resource};
use crate::prename::repository::{self, PrenameRow, RepositoryError, UpdatePrenameFields};

/// จำกัดความยาวที่ app ให้เท่ากันทุก master (ตรงกับ varchar(100) ของคอลัมน์ที่มี limit) — นับเป็นตัวอักษร
/// ไม่ใช่ byte เพราะข้อความไทย
const MAX_TEXT_CHARS: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum PrenameError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("prename not found")]
    NotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// ตรวจสิทธิ์ของ action นี้กับหน้านี้ (`prename.view/create/update/delete`)
fn require(actor: &CurrentUser, action: Action) -> Result<(), PrenameError> {
    if actor.can(Resource::Prename, action) {
        Ok(())
    } else {
        Err(PrenameError::Forbidden)
    }
}

fn map_unique_violation(err: RepositoryError) -> PrenameError {
    if err.is_unique_violation() {
        PrenameError::Conflict("prename name already taken".to_string())
    } else {
        PrenameError::from(err)
    }
}

/// trim แล้วต้องไม่ว่างและไม่เกิน 100 ตัวอักษร — คืนค่าที่ trim แล้วไว้เก็บลง DB
fn clean_text(value: &str, field: &str) -> Result<String, PrenameError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(PrenameError::Validation(format!("{field} is required")));
    }
    if trimmed.chars().count() > MAX_TEXT_CHARS {
        return Err(PrenameError::Validation(format!(
            "{field} must be at most {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(trimmed.to_string())
}

/// ฟิลด์ที่ไม่บังคับ: trim แล้วว่าง → NULL, ยาวเกิน 100 ตัวอักษร → 400
fn clean_optional(value: &str, field: &str) -> Result<Option<String>, PrenameError> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    clean_text(value, field).map(Some)
}

/// อ่านรายการได้ทั้งคนที่ดูหน้านี้ได้ และคนที่ดูหน้าผู้ใช้งานได้ (ฟอร์มผู้ใช้เลือกคำนำหน้าจากรายการนี้)
pub async fn list(pool: &PgPool, actor: &CurrentUser) -> Result<Vec<PrenameRow>, PrenameError> {
    if !actor.can(Resource::User, Action::View) {
        require(actor, Action::View)?;
    }
    Ok(repository::list_prename_rows(pool).await?)
}

pub struct CreatePrenameInput<'a> {
    pub name: &'a str,
    pub abbreviation: Option<&'a str>,
    pub sort_order: i32,
}

pub async fn create(
    pool: &PgPool,
    actor: &CurrentUser,
    input: CreatePrenameInput<'_>,
) -> Result<(), PrenameError> {
    require(actor, Action::Create)?;
    let name = clean_text(input.name, "name")?;
    let abbreviation = input
        .abbreviation
        .map(|v| clean_optional(v, "abbreviation"))
        .transpose()?
        .flatten();

    repository::insert_prename(
        pool,
        actor.user.id,
        &name,
        abbreviation.as_deref(),
        input.sort_order,
    )
    .await
    .map_err(map_unique_violation)?;
    Ok(())
}

pub struct UpdatePrenameInput<'a> {
    pub name: Option<&'a str>,
    pub abbreviation: Option<&'a str>,
    pub sort_order: Option<i32>,
}

pub async fn update(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: UpdatePrenameInput<'_>,
) -> Result<(), PrenameError> {
    require(actor, Action::Update)?;

    let existing = repository::find_prename_by_public_id(pool, public_id)
        .await?
        .ok_or(PrenameError::NotFound)?;

    let name = input.name.map(|v| clean_text(v, "name")).transpose()?;
    let abbreviation = input
        .abbreviation
        .map(|v| clean_optional(v, "abbreviation"))
        .transpose()?;

    repository::update_prename(
        pool,
        actor.user.id,
        existing.id,
        UpdatePrenameFields {
            name: name.as_deref(),
            abbreviation: abbreviation.as_ref().map(|v| v.as_deref()),
            sort_order: input.sort_order,
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
) -> Result<(), PrenameError> {
    require(actor, Action::Delete)?;

    let existing = repository::find_prename_by_public_id(pool, public_id)
        .await?
        .ok_or(PrenameError::NotFound)?;
    if repository::prename_in_use(pool, existing.id).await? {
        return Err(PrenameError::Conflict("prename is still used".to_string()));
    }

    repository::soft_delete_prename(pool, actor.user.id, existing.id).await?;
    Ok(())
}
