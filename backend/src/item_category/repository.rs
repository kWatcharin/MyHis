//! query SQLx ต่อตาราง item_category (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! ตรวจตอน compile
//! ไม่ประกอบ SQL ด้วย format! (docs/rust-guidelines.md ข้อ 11) แถวที่ถูก soft delete
//! (`deleted_at is not null`) ไม่ถูกคืนจากฟังก์ชันใดในนี้

use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use serde_json::Value;

use crate::domain::item_tracking::ItemTracking;
use crate::domain::{changes, ItemCategoryId, UserId};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

impl RepositoryError {
    /// เช็คว่า error เกิดจาก unique constraint ชนไหม (ชื่อหมวดซ้ำ) — ให้ service แปลงเป็น Conflict
    /// แทนที่จะ pre-check แข่งกับ concurrent request เอง
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, RepositoryError::Database(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()))
    }
}

#[derive(Debug, Clone)]
pub struct ItemCategoryRow {
    pub id: ItemCategoryId,
    /// uuid ของแถว — id ที่ API เห็น
    pub public_id: Uuid,
    pub name: String,
    pub default_track_lot: bool,
    pub default_track_expiry: bool,
    pub default_patient_linkable: bool,
    pub sort_order: i32,
    pub is_active: bool,
}

pub async fn list_item_categories(pool: &PgPool) -> Result<Vec<ItemCategoryRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        ItemCategoryRow,
        r#"select id as "id: ItemCategoryId", uuid as public_id, name, default_track_lot,
                  default_track_expiry, default_patient_linkable, sort_order, is_active
           from item_category
           where deleted_at is null
           order by sort_order, name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_item_category_by_public_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<ItemCategoryRow>, RepositoryError> {
    let row = sqlx::query_as!(
        ItemCategoryRow,
        r#"select id as "id: ItemCategoryId", uuid as public_id, name, default_track_lot,
                  default_track_expiry, default_patient_linkable, sort_order, is_active
           from item_category
           where uuid = $1 and deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn insert_item_category(
    pool: &PgPool,
    actor: UserId,
    name: &str,
    default_tracking: ItemTracking,
    default_patient_linkable: bool,
    sort_order: i32,
    is_active: bool,
) -> Result<ItemCategoryId, RepositoryError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query!(
        r#"insert into item_category
             (name, default_track_lot, default_track_expiry, default_patient_linkable, sort_order,
              is_active, created_by, updated_by)
           values ($1, $2, $3, $4, $5, $6, $7, $7)
           returning id as "id: ItemCategoryId""#,
        name,
        default_tracking.track_lot(),
        default_tracking.track_expiry(),
        default_patient_linkable,
        sort_order,
        is_active,
        actor.0
    )
    .fetch_one(&mut *tx)
    .await?;
    let after = snapshot(&mut tx, row.id).await?;
    insert_audit(
        &mut tx,
        actor,
        "item_category_created",
        row.id,
        Some(changes::created(after)),
    )
    .await?;
    tx.commit().await?;
    Ok(row.id)
}

pub struct UpdateItemCategoryFields<'a> {
    pub name: Option<&'a str>,
    /// แก้ธงติดตามล็อต/วันหมดอายุพร้อมกันสองตัวเสมอ (service รวมกับค่าเดิมแล้วตรวจคู่กันมาแล้ว)
    pub default_tracking: Option<ItemTracking>,
    pub default_patient_linkable: Option<bool>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

/// partial update ผ่าน COALESCE — ค่า None ไม่แตะคอลัมน์เดิม (ทุกคอลัมน์ที่แก้ได้เป็น not null
/// จึงไม่มีกรณี "ตั้งเป็น null")
pub async fn update_item_category(
    pool: &PgPool,
    actor: UserId,
    id: ItemCategoryId,
    fields: UpdateItemCategoryFields<'_>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let before = snapshot(&mut tx, id).await?;
    let updated = sqlx::query!(
        r#"update item_category
           set name = coalesce($3, name),
               default_track_lot = coalesce($4, default_track_lot),
               default_track_expiry = coalesce($5, default_track_expiry),
               default_patient_linkable = coalesce($6, default_patient_linkable),
               sort_order = coalesce($7, sort_order),
               is_active = coalesce($8, is_active),
               updated_by = $2,
               updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0,
        fields.name,
        fields.default_tracking.map(ItemTracking::track_lot),
        fields.default_tracking.map(ItemTracking::track_expiry),
        fields.default_patient_linkable,
        fields.sort_order,
        fields.is_active
    )
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() > 0 {
        let after = snapshot(&mut tx, id).await?;
        let details = changes::updated(&before, &after);
        insert_audit(&mut tx, actor, "item_category_updated", id, Some(details)).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// หมวดนี้ยังมีรายการคลัง (`item` แถวที่ยังไม่ถูกลบ) อยู่ไหม — ใช้กันลบหมวดที่ยังถูกใช้
pub async fn item_category_in_use(
    pool: &PgPool,
    id: ItemCategoryId,
) -> Result<bool, RepositoryError> {
    let row = sqlx::query!(
        r#"select exists(
             select 1 from item where category_id = $1 and deleted_at is null
           ) as "exists!""#,
        id.0
    )
    .fetch_one(pool)
    .await?;
    Ok(row.exists)
}

/// soft delete — ไม่ลบแถวจริง (CLAUDE.md) partial unique index นับเฉพาะแถวที่ยังไม่ถูกลบ
/// จึงสร้างชื่อเดิมซ้ำได้หลังลบ
pub async fn soft_delete_item_category(
    pool: &PgPool,
    actor: UserId,
    id: ItemCategoryId,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query!(
        r#"update item_category
           set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() > 0 {
        insert_audit(&mut tx, actor, "item_category_deleted", id, None).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// audit ของหน้าตั้งค่าหมวดคลัง (หน้า "ประวัติการใช้งาน") — target_id = uuid ของแถว
/// ชื่อรายการอ่านจาก join ตอนแสดงผล (soft delete แถวจึงยังอยู่เสมอ) เขียนในทรานแซกชันเดียวกับการแก้ข้อมูล
/// `details` = ค่าที่เพิ่ม / ค่าเดิม→ใหม่ (`domain::changes`) ลบไม่มี
async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    action: &str,
    id: ItemCategoryId,
    details: Option<Value>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id, metadata)
         select $1, $2, 'item_category', uuid::text, $4 from item_category where id = $3",
        actor.0,
        action,
        id.0,
        details
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// ค่าของแถวสำหรับรายละเอียดในประวัติการใช้งาน (ไม่มี id/ผู้แก้/เวลา) — ล็อกแถวไว้จนจบทรานแซกชัน
/// ไม่พบแถว = null
async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    id: ItemCategoryId,
) -> Result<Value, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"select jsonb_build_object(
                  'name', name, 'default_track_lot', default_track_lot,
                  'default_track_expiry', default_track_expiry,
                  'default_patient_linkable', default_patient_linkable,
                  'sort_order', sort_order, 'is_active', is_active
              ) as "snapshot!"
           from item_category where id = $1 for update"#,
        id.0
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.unwrap_or(Value::Null))
}
