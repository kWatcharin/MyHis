//! กฎของ item_category: สิทธิ์ (`item_category.*` ราย action), ชื่อ (trim, ไม่ว่าง, ไม่เกิน 100 ตัวอักษร),
//! ชื่อซ้ำ, ติดตามวันหมดอายุต้องติดตามล็อต (`ItemTracking`), ห้ามลบหมวดที่ยังมีรายการใช้อยู่
//! ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 10)
//!
//! ธงของหมวดเป็นแค่ค่าตั้งต้นที่คัดลอกไปตอนสร้างรายการ (โมดูล item) — แก้หมวดภายหลังไม่กระทบรายการเดิม
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็คสิทธิ์
//! ของ action นั้น (`item_category.view/create/update/delete`) ที่นี่ เหมือนโมดูล master อื่น
//! — id ขาเข้าเป็น uuid (`public_id`) แล้ว resolve เป็น id ภายในเอง

use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::domain::item_tracking::ItemTracking;
use crate::domain::{Action, Resource};
use crate::item_category::repository::{
    self, ItemCategoryRow, RepositoryError, UpdateItemCategoryFields,
};

/// ตรงกับ varchar(100) ของคอลัมน์ name — นับเป็นตัวอักษร (ไม่ใช่ byte) เพราะชื่อไทย
const MAX_TEXT_CHARS: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum ItemCategoryError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("item category not found")]
    NotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// ตรวจสิทธิ์ของ action นี้กับหน้านี้ (`item_category.view/create/update/delete`)
fn require(actor: &CurrentUser, action: Action) -> Result<(), ItemCategoryError> {
    if actor.can(Resource::ItemCategory, action) {
        Ok(())
    } else {
        Err(ItemCategoryError::Forbidden)
    }
}

fn map_unique_violation(err: RepositoryError) -> ItemCategoryError {
    if err.is_unique_violation() {
        ItemCategoryError::Conflict("item category name already taken".to_string())
    } else {
        ItemCategoryError::from(err)
    }
}

/// trim แล้วต้องไม่ว่างและไม่เกิน 100 ตัวอักษร — คืนค่าที่ trim แล้วไว้เก็บลง DB
fn clean_text(value: &str, field: &str) -> Result<String, ItemCategoryError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ItemCategoryError::Validation(format!(
            "{field} is required"
        )));
    }
    if trimmed.chars().count() > MAX_TEXT_CHARS {
        return Err(ItemCategoryError::Validation(format!(
            "{field} must be at most {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(trimmed.to_string())
}

/// ธงสองตัวจากฟอร์ม → `ItemTracking` (ติดตามวันหมดอายุโดยไม่ติดตามล็อต = 400)
fn tracking(track_lot: bool, track_expiry: bool) -> Result<ItemTracking, ItemCategoryError> {
    ItemTracking::from_flags(track_lot, track_expiry).ok_or_else(|| {
        ItemCategoryError::Validation("expiry tracking requires lot tracking".to_string())
    })
}

pub async fn list(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<ItemCategoryRow>, ItemCategoryError> {
    require(actor, Action::View)?;
    Ok(repository::list_item_categories(pool).await?)
}

pub struct CreateItemCategoryInput<'a> {
    pub name: &'a str,
    pub default_track_lot: bool,
    pub default_track_expiry: bool,
    pub default_patient_linkable: bool,
    pub sort_order: i32,
    pub is_active: bool,
}

pub async fn create(
    pool: &PgPool,
    actor: &CurrentUser,
    input: CreateItemCategoryInput<'_>,
) -> Result<(), ItemCategoryError> {
    require(actor, Action::Create)?;
    let name = clean_text(input.name, "name")?;
    let default_tracking = tracking(input.default_track_lot, input.default_track_expiry)?;

    repository::insert_item_category(
        pool,
        actor.user.id,
        &name,
        default_tracking,
        input.default_patient_linkable,
        input.sort_order,
        input.is_active,
    )
    .await
    .map_err(map_unique_violation)?;
    Ok(())
}

pub struct UpdateItemCategoryInput<'a> {
    pub name: Option<&'a str>,
    pub default_track_lot: Option<bool>,
    pub default_track_expiry: Option<bool>,
    pub default_patient_linkable: Option<bool>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

pub async fn update(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: UpdateItemCategoryInput<'_>,
) -> Result<(), ItemCategoryError> {
    require(actor, Action::Update)?;

    let existing = repository::find_item_category_by_public_id(pool, public_id)
        .await?
        .ok_or(ItemCategoryError::NotFound)?;

    let name = input.name.map(|v| clean_text(v, "name")).transpose()?;
    // ส่งมาธงเดียวก็ต้องตรวจคู่กับค่าเดิมของอีกธง (เช่น ติ๊กวันหมดอายุในหมวดที่ไม่ได้ติดตามล็อต)
    let default_tracking =
        if input.default_track_lot.is_some() || input.default_track_expiry.is_some() {
            Some(tracking(
                input
                    .default_track_lot
                    .unwrap_or(existing.default_track_lot),
                input
                    .default_track_expiry
                    .unwrap_or(existing.default_track_expiry),
            )?)
        } else {
            None
        };

    repository::update_item_category(
        pool,
        actor.user.id,
        existing.id,
        UpdateItemCategoryFields {
            name: name.as_deref(),
            default_tracking,
            default_patient_linkable: input.default_patient_linkable,
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
) -> Result<(), ItemCategoryError> {
    require(actor, Action::Delete)?;

    let existing = repository::find_item_category_by_public_id(pool, public_id)
        .await?
        .ok_or(ItemCategoryError::NotFound)?;
    if repository::item_category_in_use(pool, existing.id).await? {
        return Err(ItemCategoryError::Conflict(
            "item category is still used".to_string(),
        ));
    }

    repository::soft_delete_item_category(pool, actor.user.id, existing.id).await?;
    Ok(())
}
