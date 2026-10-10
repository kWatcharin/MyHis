//! กฎของ patient_group: สิทธิ์ (ต้องมี user.manage), ข้อความ (trim, ไม่ว่างถ้าบังคับ, ไม่เกิน 100 ตัวอักษร),
//! ชื่อซ้ำ, ห้ามลบข้อมูลที่ยังถูกใช้อยู่ ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 10)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็ค `user.manage`
//! ที่นี่ เหมือนโมดูล users/disease — id ขาเข้าเป็น uuid (`public_id`) แล้ว resolve เป็น id ภายในเอง

use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::domain::Permission;
use crate::patient_group::repository::{
    self, PatientGroupRow, RepositoryError, UpdatePatientGroupFields,
};

/// จำกัดความยาวที่ app ให้เท่ากันทุก master (ตรงกับ varchar(100) ของคอลัมน์ที่มี limit) — นับเป็นตัวอักษร
/// ไม่ใช่ byte เพราะข้อความไทย
const MAX_TEXT_CHARS: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum PatientGroupError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("patient group not found")]
    NotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

fn require_user_manage(actor: &CurrentUser) -> Result<(), PatientGroupError> {
    if actor.has_permission(Permission::UserManage) {
        Ok(())
    } else {
        Err(PatientGroupError::Forbidden)
    }
}

fn map_unique_violation(err: RepositoryError) -> PatientGroupError {
    if err.is_unique_violation() {
        PatientGroupError::Conflict("patient group name already taken".to_string())
    } else {
        PatientGroupError::from(err)
    }
}

/// trim แล้วต้องไม่ว่างและไม่เกิน 100 ตัวอักษร — คืนค่าที่ trim แล้วไว้เก็บลง DB
fn clean_text(value: &str, field: &str) -> Result<String, PatientGroupError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(PatientGroupError::Validation(format!(
            "{field} is required"
        )));
    }
    if trimmed.chars().count() > MAX_TEXT_CHARS {
        return Err(PatientGroupError::Validation(format!(
            "{field} must be at most {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(trimmed.to_string())
}

pub async fn list(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<PatientGroupRow>, PatientGroupError> {
    require_user_manage(actor)?;
    Ok(repository::list_patient_group_rows(pool).await?)
}

pub struct CreatePatientGroupInput<'a> {
    pub name: &'a str,
    pub sort_order: i32,
    pub is_active: bool,
}

pub async fn create(
    pool: &PgPool,
    actor: &CurrentUser,
    input: CreatePatientGroupInput<'_>,
) -> Result<(), PatientGroupError> {
    require_user_manage(actor)?;
    let name = clean_text(input.name, "name")?;

    repository::insert_patient_group(
        pool,
        actor.user.id,
        &name,
        input.sort_order,
        input.is_active,
    )
    .await
    .map_err(map_unique_violation)?;
    Ok(())
}

pub struct UpdatePatientGroupInput<'a> {
    pub name: Option<&'a str>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

pub async fn update(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: UpdatePatientGroupInput<'_>,
) -> Result<(), PatientGroupError> {
    require_user_manage(actor)?;

    let existing = repository::find_patient_group_by_public_id(pool, public_id)
        .await?
        .ok_or(PatientGroupError::NotFound)?;

    let name = input.name.map(|v| clean_text(v, "name")).transpose()?;

    repository::update_patient_group(
        pool,
        actor.user.id,
        existing.id,
        UpdatePatientGroupFields {
            name: name.as_deref(),
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
) -> Result<(), PatientGroupError> {
    require_user_manage(actor)?;

    let existing = repository::find_patient_group_by_public_id(pool, public_id)
        .await?
        .ok_or(PatientGroupError::NotFound)?;
    if repository::patient_group_in_use(pool, existing.id).await? {
        return Err(PatientGroupError::Conflict(
            "patient group is still used".to_string(),
        ));
    }

    repository::soft_delete_patient_group(pool, actor.user.id, existing.id).await?;
    Ok(())
}
