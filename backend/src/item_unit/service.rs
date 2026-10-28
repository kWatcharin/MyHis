//! กฎของ item_unit: สิทธิ์ (`item_unit.*` ราย action), ชื่อ (trim, ไม่ว่าง, ไม่เกิน 100 ตัวอักษร),
//! ชื่อซ้ำ, ห้ามลบหน่วยที่ยังเป็นหน่วยฐานหรือหน่วยแปลงของรายการใดอยู่ ห้าม import ชนิดข้อมูลของ Axum
//! (docs/auth-design.md ข้อ 10)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็คสิทธิ์
//! ของ action นั้น (`item_unit.view/create/update/delete`) ที่นี่ เหมือนโมดูล master อื่น
//! — id ขาเข้าเป็น uuid (`public_id`) แล้ว resolve เป็น id ภายในเอง

use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::domain::{Action, Resource};
use crate::item_unit::repository::{self, ItemUnitRow, RepositoryError, UpdateItemUnitFields};

/// ตรงกับ varchar(100) ของคอลัมน์ name — นับเป็นตัวอักษร (ไม่ใช่ byte) เพราะชื่อไทย
const MAX_TEXT_CHARS: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum ItemUnitError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("item unit not found")]
    NotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// ตรวจสิทธิ์ของ action นี้กับหน้านี้ (`item_unit.view/create/update/delete`)
fn require(actor: &CurrentUser, action: Action) -> Result<(), ItemUnitError> {
    if actor.can(Resource::ItemUnit, action) {
        Ok(())
    } else {
        Err(ItemUnitError::Forbidden)
    }
}

fn map_unique_violation(err: RepositoryError) -> ItemUnitError {
    if err.is_unique_violation() {
        ItemUnitError::Conflict("item unit name already taken".to_string())
    } else {
        ItemUnitError::from(err)
    }
}

/// trim แล้วต้องไม่ว่างและไม่เกิน 100 ตัวอักษร — คืนค่าที่ trim แล้วไว้เก็บลง DB
fn clean_text(value: &str, field: &str) -> Result<String, ItemUnitError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ItemUnitError::Validation(format!("{field} is required")));
    }
    if trimmed.chars().count() > MAX_TEXT_CHARS {
        return Err(ItemUnitError::Validation(format!(
            "{field} must be at most {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(trimmed.to_string())
}

pub async fn list(pool: &PgPool, actor: &CurrentUser) -> Result<Vec<ItemUnitRow>, ItemUnitError> {
    require(actor, Action::View)?;
    Ok(repository::list_item_units(pool).await?)
}

pub struct CreateItemUnitInput<'a> {
    pub name: &'a str,
    pub sort_order: i32,
    pub is_active: bool,
}

pub async fn create(
    pool: &PgPool,
    actor: &CurrentUser,
    input: CreateItemUnitInput<'_>,
) -> Result<(), ItemUnitError> {
    require(actor, Action::Create)?;
    let name = clean_text(input.name, "name")?;

    repository::insert_item_unit(
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

pub struct UpdateItemUnitInput<'a> {
    pub name: Option<&'a str>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

pub async fn update(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: UpdateItemUnitInput<'_>,
) -> Result<(), ItemUnitError> {
    require(actor, Action::Update)?;

    let existing = repository::find_item_unit_by_public_id(pool, public_id)
        .await?
        .ok_or(ItemUnitError::NotFound)?;

    let name = input.name.map(|v| clean_text(v, "name")).transpose()?;

    repository::update_item_unit(
        pool,
        actor.user.id,
        existing.id,
        UpdateItemUnitFields {
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
) -> Result<(), ItemUnitError> {
    require(actor, Action::Delete)?;

    let existing = repository::find_item_unit_by_public_id(pool, public_id)
        .await?
        .ok_or(ItemUnitError::NotFound)?;
    if repository::item_unit_in_use(pool, existing.id).await? {
        return Err(ItemUnitError::Conflict(
            "item unit is still used".to_string(),
        ));
    }

    repository::soft_delete_item_unit(pool, actor.user.id, existing.id).await?;
    Ok(())
}
