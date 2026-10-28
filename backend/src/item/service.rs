//! กฎของ item: สิทธิ์ (`item.*` ราย action — หน่วยแปลงใช้สิทธิ์เดียวกัน), ชื่อ (trim, ไม่ว่าง, ไม่เกิน 100 ตัวอักษร),
//! ชื่อซ้ำทั้งระบบ, หมวด/หน่วยต้องมีอยู่จริง, ติดตามวันหมดอายุต้องติดตามล็อต (`ItemTracking`), จุดสั่งซื้อไม่ติดลบ,
//! หน่วยแปลง (ไม่ซ้ำหน่วย, ไม่ใช่หน่วยฐาน, มากกว่า 1 หน่วยฐาน, ไม่เกิน `MAX_CONVERSIONS_PER_ITEM`)
//! ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 10)
//!
//! ธงที่ไม่ส่งมา: ตอนเพิ่ม = ค่าตั้งต้นของหมวด, ตอนแก้ไข = ค่าเดิมของรายการ (เปลี่ยนหมวดไม่คัดลอกธงซ้ำ)
//! หมวด/หน่วยที่ปิดใช้งานเลือกใหม่ไม่ได้ แต่ค่าที่ผูกอยู่แล้วคงไว้ได้ (แบบโรคประจำตัวของ patient)
//!
//! ยังไม่มีตาราง stock: กฎ "ห้ามเปลี่ยนหน่วยฐาน/การติดตามล็อต/วันหมดอายุหลังมีการเคลื่อนไหว" และ
//! "ห้ามลบรายการที่ยังมีของ" ต้องเพิ่มที่นี่พร้อม ledger (docs/inventory-design.md)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็คสิทธิ์
//! ของ action นั้น (`item.view/create/update/delete`) ที่นี่ เหมือนโมดูล master อื่น
//! — id ขาเข้าเป็น uuid (`public_id`) แล้ว resolve เป็น id ภายในเอง

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::domain::item_tracking::ItemTracking;
use crate::domain::{Action, ItemId, ItemUnitId, Resource};
use crate::item::repository::{
    self, ConversionRow, ExistingItem, ItemOptions, ItemRow, ItemWrite, RepositoryError,
};

/// ตรงกับ varchar(100) ของคอลัมน์ name — นับเป็นตัวอักษร (ไม่ใช่ byte) เพราะชื่อไทย
const MAX_TEXT_CHARS: usize = 100;

/// หน่วยแปลงต่อรายการ — กันคำขอที่ส่งรายการมาไม่จำกัด (ใช้จริงไม่กี่หน่วย เช่น แผง กล่อง ลัง)
const MAX_CONVERSIONS_PER_ITEM: usize = 10;

#[derive(Debug, thiserror::Error)]
pub enum ItemError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("item not found")]
    NotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// ตรวจสิทธิ์ของ action นี้กับหน้านี้ (`item.view/create/update/delete`)
fn require(actor: &CurrentUser, action: Action) -> Result<(), ItemError> {
    if actor.can(Resource::Item, action) {
        Ok(())
    } else {
        Err(ItemError::Forbidden)
    }
}

fn validation(message: &str) -> ItemError {
    ItemError::Validation(message.to_string())
}

/// หน่วยแปลงถูกตัดซ้ำก่อนเขียนแล้ว unique ที่ชนได้จึงมีแค่ชื่อรายการ
fn map_unique_violation(err: RepositoryError) -> ItemError {
    if err.is_unique_violation() {
        ItemError::Conflict("item name already taken".to_string())
    } else {
        ItemError::from(err)
    }
}

/// trim แล้วต้องไม่ว่างและไม่เกิน 100 ตัวอักษร — คืนค่าที่ trim แล้วไว้เก็บลง DB
fn clean_text(value: &str, field: &str) -> Result<String, ItemError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ItemError::Validation(format!("{field} is required")));
    }
    if trimmed.chars().count() > MAX_TEXT_CHARS {
        return Err(ItemError::Validation(format!(
            "{field} must be at most {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(trimmed.to_string())
}

/// รายการหนึ่งรายการพร้อมหน่วยแปลง (เรียงตาม factor)
#[derive(Debug)]
pub struct ItemView {
    pub item: ItemRow,
    pub conversions: Vec<ConversionRow>,
}

pub async fn list(pool: &PgPool, actor: &CurrentUser) -> Result<Vec<ItemView>, ItemError> {
    require(actor, Action::View)?;
    let (items, conversions) = tokio::try_join!(
        repository::list_items(pool),
        repository::list_conversions(pool)
    )?;
    let mut by_item: HashMap<ItemId, Vec<ConversionRow>> = HashMap::new();
    for conversion in conversions {
        by_item
            .entry(conversion.item_id)
            .or_default()
            .push(conversion);
    }
    Ok(items
        .into_iter()
        .map(|item| ItemView {
            conversions: by_item.remove(&item.id).unwrap_or_default(),
            item,
        })
        .collect())
}

/// ตัวเลือกหมวด/หน่วยของฟอร์มรายการ — สิทธิ์ดูรายการพอ ไม่ต้องมีสิทธิ์หน้าหมวด/หน่วย (แบบ `/patients/options`)
pub async fn options(pool: &PgPool, actor: &CurrentUser) -> Result<ItemOptions, ItemError> {
    require(actor, Action::View)?;
    Ok(repository::list_options(pool).await?)
}

#[derive(Debug, Clone, Copy)]
pub struct ConversionInput {
    /// uuid ของ item_unit
    pub unit_id: Uuid,
    /// จำนวนหน่วยฐานต่อ 1 หน่วยนี้
    pub factor: i32,
}

/// ข้อมูลที่ผู้ใช้กรอกตอนเพิ่ม/แก้ไข (uuid ของหมวด/หน่วยยังไม่ resolve) — แก้ไขคือแทนที่ทั้งชุด
#[derive(Debug)]
pub struct ItemInput<'a> {
    pub name: &'a str,
    pub category_id: Uuid,
    pub base_unit_id: Uuid,
    /// เพิ่ม: None = ค่าตั้งต้นของหมวด, แก้ไข: None = คงค่าเดิม (สามธงเหมือนกัน)
    pub track_lot: Option<bool>,
    pub track_expiry: Option<bool>,
    pub patient_linkable: Option<bool>,
    /// หน่วยฐาน — None = ไม่ตั้งจุดสั่งซื้อ
    pub reorder_point: Option<i32>,
    pub is_active: bool,
    /// ชุดนี้แทนที่หน่วยแปลงเดิมทั้งหมด
    pub conversions: Vec<ConversionInput>,
}

/// ตรวจค่า → resolve uuid ของหมวด/หน่วยเป็น id ภายใน → `ItemWrite`
/// `existing` เป็น Some ตอนแก้ไข (ใช้เป็นค่าของธงที่ไม่ส่งมา และตัดสินว่าหมวด/หน่วย inactive ตัวไหนผูกอยู่แล้วจึงคงไว้ได้)
async fn prepare(
    pool: &PgPool,
    input: &ItemInput<'_>,
    existing: Option<&ExistingItem>,
) -> Result<ItemWrite, ItemError> {
    let name = clean_text(input.name, "name")?;
    if input.reorder_point.is_some_and(|value| value < 0) {
        return Err(validation("reorder point must not be negative"));
    }
    if input.conversions.len() > MAX_CONVERSIONS_PER_ITEM {
        return Err(validation("too many conversions"));
    }

    let category = repository::find_category(pool, input.category_id)
        .await?
        .ok_or_else(|| validation("unknown item category"))?;
    if !category.is_active && existing.is_none_or(|e| e.category_id != category.id) {
        return Err(validation("item category is inactive"));
    }

    // หน่วยฐาน + หน่วยของหน่วยแปลง ค้นในคำสั่งเดียว
    let public_ids: Vec<Uuid> = std::iter::once(input.base_unit_id)
        .chain(input.conversions.iter().map(|c| c.unit_id))
        .collect();
    let units = repository::find_units(pool, &public_ids).await?;
    let unit = |public_id: Uuid| {
        units
            .iter()
            .find(|u| u.public_id == public_id)
            .ok_or_else(|| validation("unknown item unit"))
    };

    let base_unit = unit(input.base_unit_id)?;
    if !base_unit.is_active && existing.is_none_or(|e| e.base_unit_id != base_unit.id) {
        return Err(validation("item unit is inactive"));
    }

    let mut conversions: Vec<(ItemUnitId, i32)> = Vec::with_capacity(input.conversions.len());
    for conversion in &input.conversions {
        let found = unit(conversion.unit_id)?;
        if found.id == base_unit.id {
            return Err(validation("conversion unit must differ from base unit"));
        }
        if conversions.iter().any(|(unit_id, _)| *unit_id == found.id) {
            return Err(validation("duplicate conversion unit"));
        }
        if conversion.factor <= 1 {
            return Err(validation("conversion factor must be greater than 1"));
        }
        if !found.is_active && existing.is_none_or(|e| !e.conversion_unit_ids.contains(&found.id)) {
            return Err(validation("item unit is inactive"));
        }
        conversions.push((found.id, conversion.factor));
    }

    // ธงที่ไม่ส่งมา: เพิ่ม = ค่าตั้งต้นของหมวด, แก้ไข = ค่าเดิม (เปลี่ยนหมวดตอนแก้ไขไม่คัดลอกซ้ำ)
    let (default_lot, default_expiry, default_linkable) = match existing {
        Some(e) => (e.track_lot, e.track_expiry, e.patient_linkable),
        None => (
            category.default_track_lot,
            category.default_track_expiry,
            category.default_patient_linkable,
        ),
    };
    let tracking = ItemTracking::from_flags(
        input.track_lot.unwrap_or(default_lot),
        input.track_expiry.unwrap_or(default_expiry),
    )
    .ok_or_else(|| validation("expiry tracking requires lot tracking"))?;

    Ok(ItemWrite {
        name,
        category_id: category.id,
        base_unit_id: base_unit.id,
        tracking,
        patient_linkable: input.patient_linkable.unwrap_or(default_linkable),
        reorder_point: input.reorder_point,
        is_active: input.is_active,
        conversions,
    })
}

pub async fn create(
    pool: &PgPool,
    actor: &CurrentUser,
    input: ItemInput<'_>,
) -> Result<(), ItemError> {
    require(actor, Action::Create)?;
    let write = prepare(pool, &input, None).await?;
    repository::insert_item(pool, actor.user.id, &write)
        .await
        .map_err(map_unique_violation)?;
    Ok(())
}

pub async fn replace(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: ItemInput<'_>,
) -> Result<(), ItemError> {
    require(actor, Action::Update)?;

    let existing = repository::find_item_by_public_id(pool, public_id)
        .await?
        .ok_or(ItemError::NotFound)?;
    let write = prepare(pool, &input, Some(&existing)).await?;
    repository::replace_item(pool, actor.user.id, existing.id, &write)
        .await
        .map_err(map_unique_violation)?;
    Ok(())
}

pub async fn delete(pool: &PgPool, actor: &CurrentUser, public_id: Uuid) -> Result<(), ItemError> {
    require(actor, Action::Delete)?;

    let existing = repository::find_item_by_public_id(pool, public_id)
        .await?
        .ok_or(ItemError::NotFound)?;
    // ยังไม่มีตาราง stock — ลบได้เสมอ (เพิ่ม "ห้ามลบรายการที่ยังมีของ" ตรงนี้เมื่อมี ledger)

    repository::soft_delete_item(pool, actor.user.id, existing.id).await?;
    Ok(())
}
