//! กฎของ disease: สิทธิ์ (ต้องมี user.manage), ชื่อ/คำย่อ (trim, ไม่ว่าง, ไม่เกิน 100 ตัวอักษร),
//! ชื่อโรคซ้ำ, ห้ามลบโรคที่ยังถูกบันทึกเป็นโรคประจำตัวอยู่ ห้าม import ชนิดข้อมูลของ Axum
//! (docs/auth-design.md ข้อ 10)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็ค `user.manage`
//! ที่นี่ เหมือนโมดูล users/budget_year — id ขาเข้าเป็น uuid (`public_id`) แล้ว resolve เป็น id ภายในเอง

use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::disease::repository::{self, DiseaseRow, RepositoryError, UpdateDiseaseFields};
use crate::domain::Permission;

/// ตรงกับ varchar(100) ของคอลัมน์ name/abbreviation — นับเป็นตัวอักษร (ไม่ใช่ byte) เพราะชื่อไทย
const MAX_TEXT_CHARS: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum DiseaseError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("disease not found")]
    NotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

fn require_user_manage(actor: &CurrentUser) -> Result<(), DiseaseError> {
    if actor.has_permission(Permission::UserManage) {
        Ok(())
    } else {
        Err(DiseaseError::Forbidden)
    }
}

fn map_unique_violation(err: RepositoryError) -> DiseaseError {
    if err.is_unique_violation() {
        DiseaseError::Conflict("disease name already taken".to_string())
    } else {
        DiseaseError::from(err)
    }
}

/// trim แล้วต้องไม่ว่างและไม่เกิน 100 ตัวอักษร — คืนค่าที่ trim แล้วไว้เก็บลง DB
fn clean_text(value: &str, field: &str) -> Result<String, DiseaseError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DiseaseError::Validation(format!("{field} is required")));
    }
    if trimmed.chars().count() > MAX_TEXT_CHARS {
        return Err(DiseaseError::Validation(format!(
            "{field} must be at most {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(trimmed.to_string())
}

pub async fn list(pool: &PgPool, actor: &CurrentUser) -> Result<Vec<DiseaseRow>, DiseaseError> {
    require_user_manage(actor)?;
    Ok(repository::list_diseases(pool).await?)
}

pub struct CreateDiseaseInput<'a> {
    pub name: &'a str,
    pub abbreviation: &'a str,
    pub sort_order: i32,
    pub is_active: bool,
}

pub async fn create(
    pool: &PgPool,
    actor: &CurrentUser,
    input: CreateDiseaseInput<'_>,
) -> Result<(), DiseaseError> {
    require_user_manage(actor)?;
    let name = clean_text(input.name, "name")?;
    let abbreviation = clean_text(input.abbreviation, "abbreviation")?;

    repository::insert_disease(
        pool,
        actor.user.id,
        &name,
        &abbreviation,
        input.sort_order,
        input.is_active,
    )
    .await
    .map_err(map_unique_violation)?;
    Ok(())
}

pub struct UpdateDiseaseInput<'a> {
    pub name: Option<&'a str>,
    pub abbreviation: Option<&'a str>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

pub async fn update(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: UpdateDiseaseInput<'_>,
) -> Result<(), DiseaseError> {
    require_user_manage(actor)?;

    let existing = repository::find_disease_by_public_id(pool, public_id)
        .await?
        .ok_or(DiseaseError::NotFound)?;

    let name = input.name.map(|v| clean_text(v, "name")).transpose()?;
    let abbreviation = input
        .abbreviation
        .map(|v| clean_text(v, "abbreviation"))
        .transpose()?;

    repository::update_disease(
        pool,
        actor.user.id,
        existing.id,
        UpdateDiseaseFields {
            name: name.as_deref(),
            abbreviation: abbreviation.as_deref(),
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
) -> Result<(), DiseaseError> {
    require_user_manage(actor)?;

    let existing = repository::find_disease_by_public_id(pool, public_id)
        .await?
        .ok_or(DiseaseError::NotFound)?;
    if repository::disease_in_use(pool, existing.id).await? {
        return Err(DiseaseError::Conflict("disease is still used".to_string()));
    }

    repository::soft_delete_disease(pool, actor.user.id, existing.id).await?;
    Ok(())
}
