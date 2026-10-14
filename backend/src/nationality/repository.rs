//! query SQLx ต่อตาราง nationality (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! ตรวจตอน compile
//! ไม่ประกอบ SQL ด้วย format! (docs/rust-guidelines.md ข้อ 11) แถวที่ถูก soft delete
//! (`deleted_at is not null`) ไม่ถูกคืนจากฟังก์ชันใดในนี้

use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use serde_json::Value;

use crate::domain::{changes, NationalityId, UserId};

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
pub struct NationalityRow {
    pub id: NationalityId,
    /// uuid ของแถว — id ที่ API เห็น
    pub public_id: Uuid,
    pub national_name: String,
    pub nation: String,
    pub sort_order: i32,
    pub is_active: bool,
}

pub async fn list_nationality_rows(pool: &PgPool) -> Result<Vec<NationalityRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        NationalityRow,
        r#"select id as "id: NationalityId", uuid as public_id, national_name, nation, sort_order, is_active
           from nationality
           where deleted_at is null
           order by sort_order, national_name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_nationality_by_public_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<NationalityRow>, RepositoryError> {
    let row = sqlx::query_as!(
        NationalityRow,
        r#"select id as "id: NationalityId", uuid as public_id, national_name, nation, sort_order, is_active
           from nationality
           where uuid = $1 and deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn insert_nationality(
    pool: &PgPool,
    actor: UserId,
    national_name: &str,
    nation: &str,
    sort_order: i32,
    is_active: bool,
) -> Result<NationalityId, RepositoryError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query!(
        r#"insert into nationality (national_name, nation, sort_order, is_active, created_by, updated_by)
           values ($1, $2, $3, $4, $5, $5)
           returning id as "id: NationalityId""#,
        national_name,
        nation,
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
        "nationality_created",
        row.id,
        Some(changes::created(after)),
    )
    .await?;
    tx.commit().await?;
    Ok(row.id)
}

pub struct UpdateNationalityFields<'a> {
    pub national_name: Option<&'a str>,
    pub nation: Option<&'a str>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

/// partial update — คอลัมน์ not null ใช้ COALESCE (ค่า None ไม่แตะคอลัมน์เดิม) ส่วนคอลัมน์ที่เป็น
/// nullable ใช้ `Option<Option<_>>`: `None` = ไม่แตะ, `Some(None)` = ตั้งเป็น NULL, `Some(Some(v))` = ตั้งค่าใหม่
/// (แพทเทิร์นเดียวกับ icon ของ pages) ทั้งหมดอยู่ในทรานแซกชันเดียว
pub async fn update_nationality(
    pool: &PgPool,
    actor: UserId,
    id: NationalityId,
    fields: UpdateNationalityFields<'_>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let before = snapshot(&mut tx, id).await?;

    let updated = sqlx::query!(
        r#"update nationality
           set
               national_name = coalesce($3, national_name),
               nation = coalesce($4, nation),
               sort_order = coalesce($5, sort_order),
               is_active = coalesce($6, is_active),
               updated_by = $2,
               updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0,
        fields.national_name,
        fields.nation,
        fields.sort_order,
        fields.is_active,
    )
    .execute(&mut *tx)
    .await?;

    if updated.rows_affected() > 0 {
        let after = snapshot(&mut tx, id).await?;
        let details = changes::updated(&before, &after);
        insert_audit(&mut tx, actor, "nationality_updated", id, Some(details)).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// ยังถูกใช้อยู่ไหม (`patient.nationality_id` ที่ยังไม่ถูกลบ) — ใช้กันลบข้อมูล master ที่ยังมีข้อมูลผูกอยู่
pub async fn nationality_in_use(pool: &PgPool, id: NationalityId) -> Result<bool, RepositoryError> {
    let row = sqlx::query!(
        r#"select (exists(select 1 from patient where nationality_id = $1 and deleted_at is null)) as "exists!""#,
        id.0
    )
    .fetch_one(pool)
    .await?;
    Ok(row.exists)
}

/// soft delete — ไม่ลบแถวจริง (CLAUDE.md) partial unique index นับเฉพาะแถวที่ยังไม่ถูกลบ
/// จึงสร้างชื่อเดิมซ้ำได้หลังลบ
pub async fn soft_delete_nationality(
    pool: &PgPool,
    actor: UserId,
    id: NationalityId,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query!(
        r#"update nationality
           set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() > 0 {
        insert_audit(&mut tx, actor, "nationality_deleted", id, None).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// audit ของหน้าตั้งค่าสัญชาติ (หน้า "ประวัติการใช้งาน") — target_id = uuid ของแถว
/// ชื่อรายการอ่านจาก join ตอนแสดงผล (soft delete แถวจึงยังอยู่เสมอ) เขียนในทรานแซกชันเดียวกับการแก้ข้อมูล
/// `details` = ค่าที่เพิ่ม / ค่าเดิม→ใหม่ (`domain::changes`) ลบไม่มี
async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    action: &str,
    id: NationalityId,
    details: Option<Value>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id, metadata)
         select $1, $2, 'nationality', uuid::text, $4 from nationality where id = $3",
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
    id: NationalityId,
) -> Result<Value, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"select jsonb_build_object('national_name', national_name, 'nation', nation, 'sort_order', sort_order, 'is_active', is_active) as "snapshot!"
           from nationality where id = $1 for update"#,
        id.0
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.unwrap_or(Value::Null))
}
