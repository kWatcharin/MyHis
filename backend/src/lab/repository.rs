//! query SQLx ต่อตาราง lab (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! ตรวจตอน compile
//! ไม่ประกอบ SQL ด้วย format! (docs/rust-guidelines.md ข้อ 11) แถวที่ถูก soft delete
//! (`deleted_at is not null`) ไม่ถูกคืนจากฟังก์ชันใดในนี้

use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{LabId, UserId};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

impl RepositoryError {
    /// เช็คว่า error เกิดจาก unique constraint ชนไหม (ชื่อซ้ำ) — ให้ service แปลงเป็น Conflict
    /// แทนที่จะ pre-check แข่งกับ concurrent request เอง
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, RepositoryError::Database(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()))
    }
}

#[derive(Debug, Clone)]
pub struct LabRow {
    pub id: LabId,
    /// uuid ของแถว — id ที่ API เห็น
    pub public_id: Uuid,
    pub name: String,
    pub abbreviation: Option<String>,
    pub is_active: bool,
    pub sort_order: i32,
}

pub async fn list_lab_rows(pool: &PgPool) -> Result<Vec<LabRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        LabRow,
        r#"select id as "id: LabId", uuid as public_id, name, abbreviation, is_active, sort_order
           from lab
           where deleted_at is null
           order by sort_order, name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_lab_by_public_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<LabRow>, RepositoryError> {
    let row = sqlx::query_as!(
        LabRow,
        r#"select id as "id: LabId", uuid as public_id, name, abbreviation, is_active, sort_order
           from lab
           where uuid = $1 and deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn insert_lab(
    pool: &PgPool,
    actor: UserId,
    name: &str,
    abbreviation: Option<&str>,
    is_active: bool,
    sort_order: i32,
) -> Result<LabId, RepositoryError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query!(
        r#"insert into lab (name, abbreviation, is_active, sort_order, created_by, updated_by)
           values ($1, $2, $3, $4, $5, $5)
           returning id as "id: LabId""#,
        name,
        abbreviation,
        is_active,
        sort_order,
        actor.0
    )
    .fetch_one(&mut *tx)
    .await?;
    insert_audit(&mut tx, actor, "lab_created", row.id).await?;
    tx.commit().await?;
    Ok(row.id)
}

pub struct UpdateLabFields<'a> {
    pub name: Option<&'a str>,
    pub abbreviation: Option<Option<&'a str>>,
    pub is_active: Option<bool>,
    pub sort_order: Option<i32>,
}

/// partial update — คอลัมน์ not null ใช้ COALESCE (ค่า None ไม่แตะคอลัมน์เดิม) ส่วนคอลัมน์ที่เป็น
/// nullable ใช้ `Option<Option<_>>`: `None` = ไม่แตะ, `Some(None)` = ตั้งเป็น NULL, `Some(Some(v))` = ตั้งค่าใหม่
/// (แพทเทิร์นเดียวกับ icon ของ pages) ทั้งหมดอยู่ในทรานแซกชันเดียว
pub async fn update_lab(
    pool: &PgPool,
    actor: UserId,
    id: LabId,
    fields: UpdateLabFields<'_>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;

    let updated = sqlx::query!(
        r#"update lab
           set
               name = coalesce($3, name),
               is_active = coalesce($4, is_active),
               sort_order = coalesce($5, sort_order),
               updated_by = $2,
               updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0,
        fields.name,
        fields.is_active,
        fields.sort_order,
    )
    .execute(&mut *tx)
    .await?;

    if let Some(abbreviation) = fields.abbreviation {
        sqlx::query!(
            "update lab set abbreviation = $2, updated_by = $3, updated_at = now()
             where id = $1 and deleted_at is null",
            id.0,
            abbreviation,
            actor.0
        )
        .execute(&mut *tx)
        .await?;
    }

    if updated.rows_affected() > 0 {
        insert_audit(&mut tx, actor, "lab_updated", id).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// soft delete — ไม่ลบแถวจริง (CLAUDE.md) partial unique index นับเฉพาะแถวที่ยังไม่ถูกลบ
/// จึงสร้างชื่อเดิมซ้ำได้หลังลบ
pub async fn soft_delete_lab(
    pool: &PgPool,
    actor: UserId,
    id: LabId,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query!(
        r#"update lab
           set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() > 0 {
        insert_audit(&mut tx, actor, "lab_deleted", id).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// audit ของหน้าตั้งค่าแลป (หน้า "ประวัติการใช้งาน") — target_id = uuid ของแถว ไม่มี metadata
/// ชื่อรายการอ่านจาก join ตอนแสดงผล (soft delete แถวจึงยังอยู่เสมอ) เขียนในทรานแซกชันเดียวกับการแก้ข้อมูล
async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    action: &str,
    id: LabId,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id)
         select $1, $2, 'lab', uuid::text from lab where id = $3",
        actor.0,
        action,
        id.0
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
