//! query SQLx ต่อตาราง item_unit (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! ตรวจตอน compile
//! ไม่ประกอบ SQL ด้วย format! (docs/rust-guidelines.md ข้อ 11) แถวที่ถูก soft delete
//! (`deleted_at is not null`) ไม่ถูกคืนจากฟังก์ชันใดในนี้

use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use serde_json::Value;

use crate::domain::{changes, ItemUnitId, UserId};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

impl RepositoryError {
    /// เช็คว่า error เกิดจาก unique constraint ชนไหม (ชื่อหน่วยซ้ำ) — ให้ service แปลงเป็น Conflict
    /// แทนที่จะ pre-check แข่งกับ concurrent request เอง
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, RepositoryError::Database(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()))
    }
}

#[derive(Debug, Clone)]
pub struct ItemUnitRow {
    pub id: ItemUnitId,
    /// uuid ของแถว — id ที่ API เห็น
    pub public_id: Uuid,
    pub name: String,
    pub sort_order: i32,
    pub is_active: bool,
}

pub async fn list_item_units(pool: &PgPool) -> Result<Vec<ItemUnitRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        ItemUnitRow,
        r#"select id as "id: ItemUnitId", uuid as public_id, name, sort_order, is_active
           from item_unit
           where deleted_at is null
           order by sort_order, name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_item_unit_by_public_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<ItemUnitRow>, RepositoryError> {
    let row = sqlx::query_as!(
        ItemUnitRow,
        r#"select id as "id: ItemUnitId", uuid as public_id, name, sort_order, is_active
           from item_unit
           where uuid = $1 and deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn insert_item_unit(
    pool: &PgPool,
    actor: UserId,
    name: &str,
    sort_order: i32,
    is_active: bool,
) -> Result<ItemUnitId, RepositoryError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query!(
        r#"insert into item_unit (name, sort_order, is_active, created_by, updated_by)
           values ($1, $2, $3, $4, $4)
           returning id as "id: ItemUnitId""#,
        name,
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
        "item_unit_created",
        row.id,
        Some(changes::created(after)),
    )
    .await?;
    tx.commit().await?;
    Ok(row.id)
}

pub struct UpdateItemUnitFields<'a> {
    pub name: Option<&'a str>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

/// partial update ผ่าน COALESCE — ค่า None ไม่แตะคอลัมน์เดิม (ทุกคอลัมน์ที่แก้ได้เป็น not null
/// จึงไม่มีกรณี "ตั้งเป็น null")
pub async fn update_item_unit(
    pool: &PgPool,
    actor: UserId,
    id: ItemUnitId,
    fields: UpdateItemUnitFields<'_>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let before = snapshot(&mut tx, id).await?;
    let updated = sqlx::query!(
        r#"update item_unit
           set name = coalesce($3, name),
               sort_order = coalesce($4, sort_order),
               is_active = coalesce($5, is_active),
               updated_by = $2,
               updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0,
        fields.name,
        fields.sort_order,
        fields.is_active
    )
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() > 0 {
        let after = snapshot(&mut tx, id).await?;
        let details = changes::updated(&before, &after);
        insert_audit(&mut tx, actor, "item_unit_updated", id, Some(details)).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// หน่วยนี้ยังเป็นหน่วยฐานของรายการ หรือเป็นหน่วยแปลงของรายการที่ยังไม่ถูกลบอยู่ไหม — ใช้กันลบหน่วยที่ยังถูกใช้
/// (ลบรายการแล้วหน่วยแปลงของรายการนั้นถูก soft delete ตามไปด้วย แต่ join item ไว้อีกชั้นกันแถวค้าง)
pub async fn item_unit_in_use(pool: &PgPool, id: ItemUnitId) -> Result<bool, RepositoryError> {
    let row = sqlx::query!(
        r#"select (
             exists(select 1 from item where base_unit_id = $1 and deleted_at is null)
             or exists(
                 select 1
                 from item_unit_conversion c
                 join item i on i.id = c.item_id
                 where c.unit_id = $1 and c.deleted_at is null and i.deleted_at is null
             )
           ) as "in_use!""#,
        id.0
    )
    .fetch_one(pool)
    .await?;
    Ok(row.in_use)
}

/// soft delete — ไม่ลบแถวจริง (CLAUDE.md) partial unique index นับเฉพาะแถวที่ยังไม่ถูกลบ
/// จึงสร้างชื่อเดิมซ้ำได้หลังลบ
pub async fn soft_delete_item_unit(
    pool: &PgPool,
    actor: UserId,
    id: ItemUnitId,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query!(
        r#"update item_unit
           set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() > 0 {
        insert_audit(&mut tx, actor, "item_unit_deleted", id, None).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// audit ของหน้าตั้งค่าหน่วยนับ (หน้า "ประวัติการใช้งาน") — target_id = uuid ของแถว
/// ชื่อรายการอ่านจาก join ตอนแสดงผล (soft delete แถวจึงยังอยู่เสมอ) เขียนในทรานแซกชันเดียวกับการแก้ข้อมูล
/// `details` = ค่าที่เพิ่ม / ค่าเดิม→ใหม่ (`domain::changes`) ลบไม่มี
async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    action: &str,
    id: ItemUnitId,
    details: Option<Value>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id, metadata)
         select $1, $2, 'item_unit', uuid::text, $4 from item_unit where id = $3",
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
    id: ItemUnitId,
) -> Result<Value, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"select jsonb_build_object('name', name, 'sort_order', sort_order, 'is_active', is_active) as "snapshot!"
           from item_unit where id = $1 for update"#,
        id.0
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.unwrap_or(Value::Null))
}
