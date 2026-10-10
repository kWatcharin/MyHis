//! กฎของ lab: สิทธิ์ (ต้องมี user.manage), ข้อความ (trim, ไม่ว่างถ้าบังคับ, ไม่เกิน 100 ตัวอักษร),
//! ชื่อซ้ำ, ห้ามลบข้อมูลที่ยังถูกใช้อยู่ ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 10)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็ค `user.manage`
//! ที่นี่ เหมือนโมดูล users/disease — id ขาเข้าเป็น uuid (`public_id`) แล้ว resolve เป็น id ภายในเอง

use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::domain::Permission;
use crate::lab::repository::{self, LabRow, RepositoryError, UpdateLabFields};

/// จำกัดความยาวที่ app ให้เท่ากันทุก master (ตรงกับ varchar(100) ของคอลัมน์ที่มี limit) — นับเป็นตัวอักษร
/// ไม่ใช่ byte เพราะข้อความไทย
const MAX_TEXT_CHARS: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum LabError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("lab not found")]
    NotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

fn require_user_manage(actor: &CurrentUser) -> Result<(), LabError> {
    if actor.has_permission(Permission::UserManage) {
        Ok(())
    } else {
        Err(LabError::Forbidden)
    }
}

fn map_unique_violation(err: RepositoryError) -> LabError {
    if err.is_unique_violation() {
        LabError::Conflict("lab name already taken".to_string())
    } else {
        LabError::from(err)
    }
}

/// trim แล้วต้องไม่ว่างและไม่เกิน 100 ตัวอักษร — คืนค่าที่ trim แล้วไว้เก็บลง DB
fn clean_text(value: &str, field: &str) -> Result<String, LabError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(LabError::Validation(format!("{field} is required")));
    }
    if trimmed.chars().count() > MAX_TEXT_CHARS {
        return Err(LabError::Validation(format!(
            "{field} must be at most {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(trimmed.to_string())
}

/// ฟิลด์ที่ไม่บังคับ: trim แล้วว่าง → NULL, ยาวเกิน 100 ตัวอักษร → 400
fn clean_optional(value: &str, field: &str) -> Result<Option<String>, LabError> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    clean_text(value, field).map(Some)
}

pub async fn list(pool: &PgPool, actor: &CurrentUser) -> Result<Vec<LabRow>, LabError> {
    require_user_manage(actor)?;
    Ok(repository::list_lab_rows(pool).await?)
}

pub struct CreateLabInput<'a> {
    pub name: &'a str,
    pub abbreviation: Option<&'a str>,
    pub is_active: bool,
    pub sort_order: i32,
}

pub async fn create(
    pool: &PgPool,
    actor: &CurrentUser,
    input: CreateLabInput<'_>,
) -> Result<(), LabError> {
    require_user_manage(actor)?;
    let name = clean_text(input.name, "name")?;
    let abbreviation = input
        .abbreviation
        .map(|v| clean_optional(v, "abbreviation"))
        .transpose()?
        .flatten();

    repository::insert_lab(
        pool,
        actor.user.id,
        &name,
        abbreviation.as_deref(),
        input.is_active,
        input.sort_order,
    )
    .await
    .map_err(map_unique_violation)?;
    Ok(())
}

pub struct UpdateLabInput<'a> {
    pub name: Option<&'a str>,
    pub abbreviation: Option<&'a str>,
    pub is_active: Option<bool>,
    pub sort_order: Option<i32>,
}

pub async fn update(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: UpdateLabInput<'_>,
) -> Result<(), LabError> {
    require_user_manage(actor)?;

    let existing = repository::find_lab_by_public_id(pool, public_id)
        .await?
        .ok_or(LabError::NotFound)?;

    let name = input.name.map(|v| clean_text(v, "name")).transpose()?;
    let abbreviation = input
        .abbreviation
        .map(|v| clean_optional(v, "abbreviation"))
        .transpose()?;

    repository::update_lab(
        pool,
        actor.user.id,
        existing.id,
        UpdateLabFields {
            name: name.as_deref(),
            abbreviation: abbreviation.as_ref().map(|v| v.as_deref()),
            is_active: input.is_active,
            sort_order: input.sort_order,
        },
    )
    .await
    .map_err(map_unique_violation)?;
    Ok(())
}

pub async fn delete(pool: &PgPool, actor: &CurrentUser, public_id: Uuid) -> Result<(), LabError> {
    require_user_manage(actor)?;

    let existing = repository::find_lab_by_public_id(pool, public_id)
        .await?
        .ok_or(LabError::NotFound)?;
    // ยังไม่มีตารางใดอ้างอิงตารางนี้ — ลบได้เสมอ (เพิ่มการตรวจตรงนี้เมื่อมีตารางอ้างอิง)

    repository::soft_delete_lab(pool, actor.user.id, existing.id).await?;
    Ok(())
}
