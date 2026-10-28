//! query SQLx ต่อตาราง item + item_unit_conversion (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! ตรวจตอน compile
//! ไม่ประกอบ SQL ด้วย format! (docs/rust-guidelines.md ข้อ 11) แถวที่ถูก soft delete
//! (`deleted_at is not null`) ไม่ถูกคืนจากฟังก์ชันใดในนี้
//!
//! อ่านตาราง item_category / item_unit ตรง ๆ (ชื่อ, ค่าตั้งต้นของธง, สถานะ) ไม่เรียก service ของโมดูลนั้น
//! (โมดูลฟีเจอร์ไม่เรียกกันเอง — แบบเดียวกับที่ patient อ่าน prename/disease)

use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use serde_json::Value;

use crate::domain::item_tracking::ItemTracking;
use crate::domain::{changes, ItemCategoryId, ItemId, ItemUnitId, UserId};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

impl RepositoryError {
    /// เช็คว่า error เกิดจาก unique constraint ชนไหม (ชื่อรายการซ้ำ) — ให้ service แปลงเป็น Conflict
    /// แทนที่จะ pre-check แข่งกับ concurrent request เอง
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, RepositoryError::Database(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()))
    }
}

/// รายการหนึ่งแถวพร้อมชื่อหมวด/หน่วยฐาน (หน่วยแปลงอยู่ใน `ConversionRow`)
#[derive(Debug, Clone)]
pub struct ItemRow {
    pub id: ItemId,
    /// uuid ของแถว — id ที่ API เห็น
    pub public_id: Uuid,
    pub name: String,
    pub category_public_id: Uuid,
    pub category_name: String,
    pub base_unit_public_id: Uuid,
    pub base_unit_name: String,
    pub track_lot: bool,
    pub track_expiry: bool,
    pub patient_linkable: bool,
    pub reorder_point: Option<i32>,
    pub is_active: bool,
}

/// หน่วยแปลงหนึ่งแถวของรายการ `item_id`
#[derive(Debug, Clone)]
pub struct ConversionRow {
    pub item_id: ItemId,
    pub unit_public_id: Uuid,
    pub unit_name: String,
    pub factor: i32,
}

/// เรียงตามหมวด (ลำดับของหน้าตั้งค่าหมวด) แล้วตามชื่อรายการ
pub async fn list_items(pool: &PgPool) -> Result<Vec<ItemRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        ItemRow,
        r#"select i.id as "id: ItemId", i.uuid as public_id, i.name,
                  c.uuid as category_public_id, c.name as category_name,
                  u.uuid as base_unit_public_id, u.name as base_unit_name,
                  i.track_lot, i.track_expiry, i.patient_linkable, i.reorder_point, i.is_active
           from item i
           join item_category c on c.id = i.category_id
           join item_unit u on u.id = i.base_unit_id
           where i.deleted_at is null
           order by c.sort_order, c.name, i.name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// หน่วยแปลงของทุกรายการที่ยังไม่ถูกลบ (คำสั่งเดียว ไม่ query ทีละรายการ) เรียงตาม factor
pub async fn list_conversions(pool: &PgPool) -> Result<Vec<ConversionRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        ConversionRow,
        r#"select cv.item_id as "item_id: ItemId", u.uuid as unit_public_id, u.name as unit_name,
                  cv.factor
           from item_unit_conversion cv
           join item i on i.id = cv.item_id
           join item_unit u on u.id = cv.unit_id
           where cv.deleted_at is null and i.deleted_at is null
           order by cv.item_id, cv.factor, u.name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// ค่าเดิมของรายการที่ service ใช้ตอนแก้ไข/ลบ
#[derive(Debug)]
pub struct ExistingItem {
    pub id: ItemId,
    pub category_id: ItemCategoryId,
    pub base_unit_id: ItemUnitId,
    pub track_lot: bool,
    pub track_expiry: bool,
    pub patient_linkable: bool,
    /// หน่วยของหน่วยแปลงที่ยังไม่ถูกลบ — ใช้ตัดสินว่าหน่วย inactive ตัวไหน "คงไว้ได้" (ผูกอยู่แล้ว)
    pub conversion_unit_ids: Vec<ItemUnitId>,
}

pub async fn find_item_by_public_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<ExistingItem>, RepositoryError> {
    let row = sqlx::query!(
        r#"select i.id as "id: ItemId", i.category_id as "category_id: ItemCategoryId",
                  i.base_unit_id as "base_unit_id: ItemUnitId", i.track_lot, i.track_expiry,
                  i.patient_linkable,
                  array(select cv.unit_id from item_unit_conversion cv
                        where cv.item_id = i.id and cv.deleted_at is null) as "conversion_unit_ids!"
           from item i
           where i.uuid = $1 and i.deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| ExistingItem {
        id: r.id,
        category_id: r.category_id,
        base_unit_id: r.base_unit_id,
        track_lot: r.track_lot,
        track_expiry: r.track_expiry,
        patient_linkable: r.patient_linkable,
        conversion_unit_ids: r.conversion_unit_ids.into_iter().map(ItemUnitId).collect(),
    }))
}

/// หมวดที่มีอยู่จริง (ยังไม่ถูกลบ) พร้อมค่าตั้งต้นของธง
#[derive(Debug)]
pub struct CategoryLookup {
    pub id: ItemCategoryId,
    pub is_active: bool,
    pub default_track_lot: bool,
    pub default_track_expiry: bool,
    pub default_patient_linkable: bool,
}

pub async fn find_category(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<CategoryLookup>, RepositoryError> {
    let row = sqlx::query_as!(
        CategoryLookup,
        r#"select id as "id: ItemCategoryId", is_active, default_track_lot, default_track_expiry,
                  default_patient_linkable
           from item_category
           where uuid = $1 and deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

#[derive(Debug)]
pub struct UnitLookup {
    pub id: ItemUnitId,
    pub public_id: Uuid,
    pub is_active: bool,
}

/// หน่วยที่มีอยู่จริง (ยังไม่ถูกลบ) จากรายการ uuid — ตัวที่ไม่พบจะไม่อยู่ในผลลัพธ์
pub async fn find_units(
    pool: &PgPool,
    public_ids: &[Uuid],
) -> Result<Vec<UnitLookup>, RepositoryError> {
    let rows = sqlx::query_as!(
        UnitLookup,
        r#"select id as "id: ItemUnitId", uuid as public_id, is_active
           from item_unit
           where uuid = any($1) and deleted_at is null"#,
        public_ids
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug)]
pub struct CategoryOptionRow {
    pub public_id: Uuid,
    pub name: String,
    pub default_track_lot: bool,
    pub default_track_expiry: bool,
    pub default_patient_linkable: bool,
}

#[derive(Debug)]
pub struct UnitOptionRow {
    pub public_id: Uuid,
    pub name: String,
}

#[derive(Debug)]
pub struct ItemOptions {
    pub categories: Vec<CategoryOptionRow>,
    pub units: Vec<UnitOptionRow>,
}

/// หมวด/หน่วยที่ใช้งานอยู่สำหรับฟอร์มรายการ เรียงตามลำดับของหน้าตั้งค่านั้น
pub async fn list_options(pool: &PgPool) -> Result<ItemOptions, RepositoryError> {
    let categories = sqlx::query_as!(
        CategoryOptionRow,
        r#"select uuid as public_id, name, default_track_lot, default_track_expiry,
                  default_patient_linkable
           from item_category
           where deleted_at is null and is_active
           order by sort_order, name"#
    )
    .fetch_all(pool)
    .await?;
    let units = sqlx::query_as!(
        UnitOptionRow,
        r#"select uuid as public_id, name
           from item_unit
           where deleted_at is null and is_active
           order by sort_order, name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(ItemOptions { categories, units })
}

/// ค่าที่ service ตรวจและ resolve id แล้ว พร้อมเขียนลง DB
#[derive(Debug)]
pub struct ItemWrite {
    pub name: String,
    pub category_id: ItemCategoryId,
    pub base_unit_id: ItemUnitId,
    pub tracking: ItemTracking,
    pub patient_linkable: bool,
    pub reorder_point: Option<i32>,
    pub is_active: bool,
    /// (หน่วย, จำนวนหน่วยฐานต่อ 1 หน่วยนั้น) — ไม่ซ้ำหน่วย ไม่มีหน่วยฐาน
    pub conversions: Vec<(ItemUnitId, i32)>,
}

/// สร้างรายการ + หน่วยแปลง + audit ในทรานแซกชันเดียว
pub async fn insert_item(
    pool: &PgPool,
    actor: UserId,
    w: &ItemWrite,
) -> Result<ItemId, RepositoryError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query!(
        r#"insert into item
             (name, category_id, base_unit_id, track_lot, track_expiry, patient_linkable,
              reorder_point, is_active, created_by, updated_by)
           values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
           returning id as "id: ItemId""#,
        w.name,
        w.category_id.0,
        w.base_unit_id.0,
        w.tracking.track_lot(),
        w.tracking.track_expiry(),
        w.patient_linkable,
        w.reorder_point,
        w.is_active,
        actor.0
    )
    .fetch_one(&mut *tx)
    .await?;
    sync_conversions(&mut tx, actor, row.id, &w.conversions).await?;
    let after = snapshot(&mut tx, row.id).await?;
    insert_audit(
        &mut tx,
        actor,
        "item_created",
        row.id,
        Some(changes::created(after)),
    )
    .await?;
    tx.commit().await?;
    Ok(row.id)
}

/// ผลของการแทนที่/ลบรายการ — กฎที่ขึ้นกับ ledger (ตาราง stock อ่านตรง — โมดูลฟีเจอร์ไม่เรียกกันเอง) ตรวจหลังล็อก
/// แถว item แล้ว การรับเข้าคลังล็อกแถวเดียวกันก่อนลงยอด จึงไม่มีจังหวะที่ตรวจผ่านแล้วมี movement แทรก
#[derive(Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    Done,
    /// มีการเคลื่อนไหวในคลังแล้ว — เปลี่ยนหน่วยฐาน/ธงติดตามล็อต/วันหมดอายุไม่ได้ (ไม่ได้บันทึกอะไร)
    HasMovements,
    /// ยังมีของคงเหลือ — ลบไม่ได้ (ไม่ได้บันทึกอะไร)
    HasStock,
}

/// แทนที่ข้อมูลรายการทั้งชุด + หน่วยแปลง + audit ในทรานแซกชันเดียว — หลังมี movement ห้ามเปลี่ยนหน่วยฐาน
/// (ยอดเดิมนับเป็นหน่วยฐานเดิม: 100 เม็ดจะกลายเป็น 100 กล่อง) และห้ามเปลี่ยนการติดตามล็อต/วันหมดอายุ
pub async fn replace_item(
    pool: &PgPool,
    actor: UserId,
    id: ItemId,
    w: &ItemWrite,
) -> Result<WriteOutcome, RepositoryError> {
    let mut tx = pool.begin().await?;
    let before = snapshot(&mut tx, id).await?;
    let locked = sqlx::query_scalar!(
        r#"select (i.base_unit_id <> $2 or i.track_lot <> $3 or i.track_expiry <> $4)
                  and exists (select 1 from stock_lot sl
                              join stock_movement m on m.lot_id = sl.id
                              where sl.item_id = i.id) as "locked!"
           from item i
           where i.id = $1"#,
        id.0,
        w.base_unit_id.0,
        w.tracking.track_lot(),
        w.tracking.track_expiry()
    )
    .fetch_optional(&mut *tx)
    .await?;
    if locked == Some(true) {
        return Ok(WriteOutcome::HasMovements);
    }
    let updated = sqlx::query!(
        r#"update item
           set name = $3, category_id = $4, base_unit_id = $5, track_lot = $6, track_expiry = $7,
               patient_linkable = $8, reorder_point = $9, is_active = $10,
               updated_by = $2, updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0,
        w.name,
        w.category_id.0,
        w.base_unit_id.0,
        w.tracking.track_lot(),
        w.tracking.track_expiry(),
        w.patient_linkable,
        w.reorder_point,
        w.is_active
    )
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() > 0 {
        sync_conversions(&mut tx, actor, id, &w.conversions).await?;
        let after = snapshot(&mut tx, id).await?;
        let details = changes::updated(&before, &after);
        insert_audit(&mut tx, actor, "item_updated", id, Some(details)).await?;
    }
    tx.commit().await?;
    Ok(WriteOutcome::Done)
}

/// soft delete รายการพร้อมหน่วยแปลงของรายการ + audit ในทรานแซกชันเดียว — partial unique ของชื่อ
/// นับเฉพาะแถวที่ยังไม่ถูกลบ จึงสร้างชื่อเดิมซ้ำได้หลังลบ รายการที่ยังมีของคงเหลือลบไม่ได้ (ของหมดแล้วลบได้ ประวัติ
/// ใน ledger ยังอ้างแถวเดิม)
pub async fn soft_delete_item(
    pool: &PgPool,
    actor: UserId,
    id: ItemId,
) -> Result<WriteOutcome, RepositoryError> {
    let mut tx = pool.begin().await?;
    let has_stock = sqlx::query_scalar!(
        r#"select exists (select 1 from stock_lot sl
                          join stock_balance b on b.lot_id = sl.id
                          where sl.item_id = i.id and b.qty_on_hand > 0) as "has_stock!"
           from item i
           where i.id = $1
           for update of i"#,
        id.0
    )
    .fetch_optional(&mut *tx)
    .await?;
    if has_stock == Some(true) {
        return Ok(WriteOutcome::HasStock);
    }
    sqlx::query!(
        "update item_unit_conversion
         set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
         where item_id = $1 and deleted_at is null",
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    let deleted = sqlx::query!(
        "update item
         set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
         where id = $1 and deleted_at is null",
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() > 0 {
        insert_audit(&mut tx, actor, "item_deleted", id, None).await?;
    }
    tx.commit().await?;
    Ok(WriteOutcome::Done)
}

/// ทำให้หน่วยแปลงที่ยังไม่ถูกลบของรายการ "ตรงกับ" ชุดที่ต้องการ: หน่วยที่ไม่อยู่ในชุด → soft delete,
/// หน่วยใหม่ → insert, หน่วยเดิมที่ factor เปลี่ยน → update (แบบ sync_conditions ของ patient — partial unique
/// นับเฉพาะแถวที่ยังไม่ถูกลบ จึงเพิ่มหน่วยที่เคยเอาออกกลับมาได้) ยอดใน ledger เป็นหน่วยฐาน แก้ factor จึงไม่กระทบยอดเดิม
async fn sync_conversions(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    item_id: ItemId,
    desired: &[(ItemUnitId, i32)],
) -> Result<(), RepositoryError> {
    let existing = sqlx::query!(
        r#"select id, unit_id as "unit_id: ItemUnitId", factor
           from item_unit_conversion
           where item_id = $1 and deleted_at is null"#,
        item_id.0
    )
    .fetch_all(&mut **tx)
    .await?;

    for row in &existing {
        if !desired.iter().any(|(unit_id, _)| *unit_id == row.unit_id) {
            sqlx::query!(
                "update item_unit_conversion
                 set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
                 where id = $1",
                row.id,
                actor.0
            )
            .execute(&mut **tx)
            .await?;
        }
    }

    for (unit_id, factor) in desired {
        match existing.iter().find(|row| row.unit_id == *unit_id) {
            Some(row) if row.factor == *factor => {}
            Some(row) => {
                sqlx::query!(
                    "update item_unit_conversion
                     set factor = $2, updated_by = $3, updated_at = now()
                     where id = $1",
                    row.id,
                    factor,
                    actor.0
                )
                .execute(&mut **tx)
                .await?;
            }
            None => {
                sqlx::query!(
                    "insert into item_unit_conversion (item_id, unit_id, factor, created_by, updated_by)
                     values ($1, $2, $3, $4, $4)",
                    item_id.0,
                    unit_id.0,
                    factor,
                    actor.0
                )
                .execute(&mut **tx)
                .await?;
            }
        }
    }
    Ok(())
}

/// audit ของหน้าตั้งค่ารายการคลัง (หน้า "ประวัติการใช้งาน") — target_id = uuid ของแถว
/// ชื่อรายการอ่านจาก join ตอนแสดงผล (soft delete แถวจึงยังอยู่เสมอ) เขียนในทรานแซกชันเดียวกับการแก้ข้อมูล
/// `details` = ค่าที่เพิ่ม / ค่าเดิม→ใหม่ (`domain::changes`) ลบไม่มี
async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    action: &str,
    id: ItemId,
    details: Option<Value>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id, metadata)
         select $1, $2, 'item', uuid::text, $4 from item where id = $3",
        actor.0,
        action,
        id.0,
        details
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// ค่าของแถวสำหรับรายละเอียดในประวัติการใช้งาน — FK เป็นชื่อ, หน่วยแปลงเป็นข้อความ เช่น "1 กล่อง = 100 เม็ด"
/// (ไม่มี id/ผู้แก้/เวลา) ล็อกแถวไว้จนจบทรานแซกชัน ไม่พบแถว = null
async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    id: ItemId,
) -> Result<Value, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"select jsonb_build_object(
                  'name', i.name, 'category', c.name, 'base_unit', u.name,
                  'track_lot', i.track_lot, 'track_expiry', i.track_expiry,
                  'patient_linkable', i.patient_linkable, 'reorder_point', i.reorder_point,
                  'is_active', i.is_active,
                  'conversions', coalesce((
                      select jsonb_agg('1 ' || cu.name || ' = ' || cv.factor || ' ' || u.name
                                       order by cv.factor, cu.name)
                      from item_unit_conversion cv
                      join item_unit cu on cu.id = cv.unit_id
                      where cv.item_id = i.id and cv.deleted_at is null
                  ), '[]'::jsonb)
              ) as "snapshot!"
           from item i
           join item_category c on c.id = i.category_id
           join item_unit u on u.id = i.base_unit_id
           where i.id = $1
           for update of i"#,
        id.0
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.unwrap_or(Value::Null))
}
