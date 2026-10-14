//! query SQLx ต่อตาราง disease (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! ตรวจตอน compile
//! ไม่ประกอบ SQL ด้วย format! (docs/rust-guidelines.md ข้อ 11) แถวที่ถูก soft delete
//! (`deleted_at is not null`) ไม่ถูกคืนจากฟังก์ชันใดในนี้

use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use serde_json::Value;

use crate::domain::{changes, DiseaseId, UserId};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

impl RepositoryError {
    /// เช็คว่า error เกิดจาก unique constraint ชนไหม (ชื่อโรคซ้ำ) — ให้ service แปลงเป็น Conflict
    /// แทนที่จะ pre-check แข่งกับ concurrent request เอง
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, RepositoryError::Database(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()))
    }
}

#[derive(Debug, Clone)]
pub struct DiseaseRow {
    pub id: DiseaseId,
    /// uuid ของแถว — id ที่ API เห็น
    pub public_id: Uuid,
    pub name: String,
    pub abbreviation: String,
    pub sort_order: i32,
    pub is_active: bool,
}

pub async fn list_diseases(pool: &PgPool) -> Result<Vec<DiseaseRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        DiseaseRow,
        r#"select id as "id: DiseaseId", uuid as public_id, name, abbreviation, sort_order,
                  is_active
           from disease
           where deleted_at is null
           order by sort_order, name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_disease_by_public_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<DiseaseRow>, RepositoryError> {
    let row = sqlx::query_as!(
        DiseaseRow,
        r#"select id as "id: DiseaseId", uuid as public_id, name, abbreviation, sort_order,
                  is_active
           from disease
           where uuid = $1 and deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn insert_disease(
    pool: &PgPool,
    actor: UserId,
    name: &str,
    abbreviation: &str,
    sort_order: i32,
    is_active: bool,
) -> Result<DiseaseId, RepositoryError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query!(
        r#"insert into disease (name, abbreviation, sort_order, is_active, created_by, updated_by)
           values ($1, $2, $3, $4, $5, $5)
           returning id as "id: DiseaseId""#,
        name,
        abbreviation,
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
        "disease_created",
        row.id,
        Some(changes::created(after)),
    )
    .await?;
    tx.commit().await?;
    Ok(row.id)
}

pub struct UpdateDiseaseFields<'a> {
    pub name: Option<&'a str>,
    pub abbreviation: Option<&'a str>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

/// partial update ผ่าน COALESCE — ค่า None ไม่แตะคอลัมน์เดิม (ทุกคอลัมน์ที่แก้ได้เป็น not null
/// จึงไม่มีกรณี "ตั้งเป็น null")
pub async fn update_disease(
    pool: &PgPool,
    actor: UserId,
    id: DiseaseId,
    fields: UpdateDiseaseFields<'_>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let before = snapshot(&mut tx, id).await?;
    let updated = sqlx::query!(
        r#"update disease
           set name = coalesce($3, name),
               abbreviation = coalesce($4, abbreviation),
               sort_order = coalesce($5, sort_order),
               is_active = coalesce($6, is_active),
               updated_by = $2,
               updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0,
        fields.name,
        fields.abbreviation,
        fields.sort_order,
        fields.is_active
    )
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() > 0 {
        let after = snapshot(&mut tx, id).await?;
        let details = changes::updated(&before, &after);
        insert_audit(&mut tx, actor, "disease_updated", id, Some(details)).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// โรคนี้ยังถูกบันทึกเป็นโรคประจำตัว (`medical_condition` แถวที่ยังไม่ถูกลบ) อยู่ไหม — ใช้กันลบโรค
/// ที่ยังมีข้อมูลคนไข้ผูกอยู่
pub async fn disease_in_use(pool: &PgPool, id: DiseaseId) -> Result<bool, RepositoryError> {
    let row = sqlx::query!(
        r#"select exists(
             select 1 from medical_condition where disease_id = $1 and deleted_at is null
           ) as "exists!""#,
        id.0
    )
    .fetch_one(pool)
    .await?;
    Ok(row.exists)
}

/// soft delete — ไม่ลบแถวจริง (CLAUDE.md) partial unique index นับเฉพาะแถวที่ยังไม่ถูกลบ
/// จึงสร้างชื่อเดิมซ้ำได้หลังลบ
pub async fn soft_delete_disease(
    pool: &PgPool,
    actor: UserId,
    id: DiseaseId,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query!(
        r#"update disease
           set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() > 0 {
        insert_audit(&mut tx, actor, "disease_deleted", id, None).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// audit ของหน้าตั้งค่าโรค (หน้า "ประวัติการใช้งาน") — target_id = uuid ของแถว
/// ชื่อรายการอ่านจาก join ตอนแสดงผล (soft delete แถวจึงยังอยู่เสมอ) เขียนในทรานแซกชันเดียวกับการแก้ข้อมูล
/// `details` = ค่าที่เพิ่ม / ค่าเดิม→ใหม่ (`domain::changes`) ลบไม่มี
async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    action: &str,
    id: DiseaseId,
    details: Option<Value>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id, metadata)
         select $1, $2, 'disease', uuid::text, $4 from disease where id = $3",
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
    id: DiseaseId,
) -> Result<Value, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"select jsonb_build_object('name', name, 'abbreviation', abbreviation, 'sort_order', sort_order, 'is_active', is_active) as "snapshot!"
           from disease where id = $1 for update"#,
        id.0
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.unwrap_or(Value::Null))
}
