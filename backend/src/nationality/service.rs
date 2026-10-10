//! กฎของ nationality: สิทธิ์ (ต้องมี user.manage), ข้อความ (trim, ไม่ว่างถ้าบังคับ, ไม่เกิน 100 ตัวอักษร),
//! ชื่อซ้ำ, ห้ามลบข้อมูลที่ยังถูกใช้อยู่ ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 10)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็ค `user.manage`
//! ที่นี่ เหมือนโมดูล users/disease — id ขาเข้าเป็น uuid (`public_id`) แล้ว resolve เป็น id ภายในเอง

use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::domain::Permission;
use crate::nationality::repository::{
    self, NationalityRow, RepositoryError, UpdateNationalityFields,
};

/// จำกัดความยาวที่ app ให้เท่ากันทุก master (ตรงกับ varchar(100) ของคอลัมน์ที่มี limit) — นับเป็นตัวอักษร
/// ไม่ใช่ byte เพราะข้อความไทย
const MAX_TEXT_CHARS: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum NationalityError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("nationality not found")]
    NotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

fn require_user_manage(actor: &CurrentUser) -> Result<(), NationalityError> {
    if actor.has_permission(Permission::UserManage) {
        Ok(())
    } else {
        Err(NationalityError::Forbidden)
    }
}

fn map_unique_violation(err: RepositoryError) -> NationalityError {
    if err.is_unique_violation() {
        NationalityError::Conflict("nationality name already taken".to_string())
    } else {
        NationalityError::from(err)
    }
}

/// trim แล้วต้องไม่ว่างและไม่เกิน 100 ตัวอักษร — คืนค่าที่ trim แล้วไว้เก็บลง DB
fn clean_text(value: &str, field: &str) -> Result<String, NationalityError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(NationalityError::Validation(format!("{field} is required")));
    }
    if trimmed.chars().count() > MAX_TEXT_CHARS {
        return Err(NationalityError::Validation(format!(
            "{field} must be at most {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(trimmed.to_string())
}

pub async fn list(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<NationalityRow>, NationalityError> {
    require_user_manage(actor)?;
    Ok(repository::list_nationality_rows(pool).await?)
}

pub struct CreateNationalityInput<'a> {
    pub national_name: &'a str,
    pub nation: &'a str,
    pub sort_order: i32,
    pub is_active: bool,
}

pub async fn create(
    pool: &PgPool,
    actor: &CurrentUser,
    input: CreateNationalityInput<'_>,
) -> Result<(), NationalityError> {
    require_user_manage(actor)?;
    let national_name = clean_text(input.national_name, "national name")?;
    let nation = clean_text(input.nation, "nation")?;

    repository::insert_nationality(
        pool,
        actor.user.id,
        &national_name,
        &nation,
        input.sort_order,
        input.is_active,
    )
    .await
    .map_err(map_unique_violation)?;
    Ok(())
}

pub struct UpdateNationalityInput<'a> {
    pub national_name: Option<&'a str>,
    pub nation: Option<&'a str>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

pub async fn update(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: UpdateNationalityInput<'_>,
) -> Result<(), NationalityError> {
    require_user_manage(actor)?;

    let existing = repository::find_nationality_by_public_id(pool, public_id)
        .await?
        .ok_or(NationalityError::NotFound)?;

    let national_name = input
        .national_name
        .map(|v| clean_text(v, "national name"))
        .transpose()?;
    let nation = input.nation.map(|v| clean_text(v, "nation")).transpose()?;

    repository::update_nationality(
        pool,
        actor.user.id,
        existing.id,
        UpdateNationalityFields {
            national_name: national_name.as_deref(),
            nation: nation.as_deref(),
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
) -> Result<(), NationalityError> {
    require_user_manage(actor)?;

    let existing = repository::find_nationality_by_public_id(pool, public_id)
        .await?
        .ok_or(NationalityError::NotFound)?;
    if repository::nationality_in_use(pool, existing.id).await? {
        return Err(NationalityError::Conflict(
            "nationality is still used".to_string(),
        ));
    }

    repository::soft_delete_nationality(pool, actor.user.id, existing.id).await?;
    Ok(())
}
