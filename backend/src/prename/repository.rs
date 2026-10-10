//! query SQLx ต่อตาราง prename (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! ตรวจตอน compile
//! ไม่ประกอบ SQL ด้วย format! (docs/rust-guidelines.md ข้อ 11) แถวที่ถูก soft delete
//! (`deleted_at is not null`) ไม่ถูกคืนจากฟังก์ชันใดในนี้

use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{PrenameId, UserId};

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
pub struct PrenameRow {
    pub id: PrenameId,
    /// uuid ของแถว — id ที่ API เห็น
    pub public_id: Uuid,
    pub name: String,
    pub abbreviation: Option<String>,
    pub sort_order: i32,
}

pub async fn list_prename_rows(pool: &PgPool) -> Result<Vec<PrenameRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        PrenameRow,
        r#"select id as "id: PrenameId", uuid as public_id, name, abbreviation, sort_order
           from prename
           where deleted_at is null
           order by sort_order, name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_prename_by_public_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<PrenameRow>, RepositoryError> {
    let row = sqlx::query_as!(
        PrenameRow,
        r#"select id as "id: PrenameId", uuid as public_id, name, abbreviation, sort_order
           from prename
           where uuid = $1 and deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn insert_prename(
    pool: &PgPool,
    actor: UserId,
    name: &str,
    abbreviation: Option<&str>,
    sort_order: i32,
) -> Result<PrenameId, RepositoryError> {
    let row = sqlx::query!(
        r#"insert into prename (name, abbreviation, sort_order, created_by, updated_by)
           values ($1, $2, $3, $4, $4)
           returning id as "id: PrenameId""#,
        name,
        abbreviation,
        sort_order,
        actor.0
    )
    .fetch_one(pool)
    .await?;
    Ok(row.id)
}

pub struct UpdatePrenameFields<'a> {
    pub name: Option<&'a str>,
    pub abbreviation: Option<Option<&'a str>>,
    pub sort_order: Option<i32>,
}

/// partial update — คอลัมน์ not null ใช้ COALESCE (ค่า None ไม่แตะคอลัมน์เดิม) ส่วนคอลัมน์ที่เป็น
/// nullable ใช้ `Option<Option<_>>`: `None` = ไม่แตะ, `Some(None)` = ตั้งเป็น NULL, `Some(Some(v))` = ตั้งค่าใหม่
/// (แพทเทิร์นเดียวกับ icon ของ pages) ทั้งหมดอยู่ในทรานแซกชันเดียว
pub async fn update_prename(
    pool: &PgPool,
    actor: UserId,
    id: PrenameId,
    fields: UpdatePrenameFields<'_>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        r#"update prename
           set
               name = coalesce($3, name),
               sort_order = coalesce($4, sort_order),
               updated_by = $2,
               updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0,
        fields.name,
        fields.sort_order,
    )
    .execute(&mut *tx)
    .await?;

    if let Some(abbreviation) = fields.abbreviation {
        sqlx::query!(
            "update prename set abbreviation = $2, updated_by = $3, updated_at = now()
             where id = $1 and deleted_at is null",
            id.0,
            abbreviation,
            actor.0
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// ยังถูกใช้อยู่ไหม (`patient.prename_id` ที่ยังไม่ถูกลบ หรือ `users.prename_id` — ผู้ใช้ไม่มีการลบจริง นับทุกคน)
/// ใช้กันลบข้อมูล master ที่ยังมีข้อมูลผูกอยู่
pub async fn prename_in_use(pool: &PgPool, id: PrenameId) -> Result<bool, RepositoryError> {
    let row = sqlx::query!(
        r#"select (exists(select 1 from patient where prename_id = $1 and deleted_at is null)
                   or exists(select 1 from users where prename_id = $1)) as "exists!""#,
        id.0
    )
    .fetch_one(pool)
    .await?;
    Ok(row.exists)
}

/// soft delete — ไม่ลบแถวจริง (CLAUDE.md) partial unique index นับเฉพาะแถวที่ยังไม่ถูกลบ
/// จึงสร้างชื่อเดิมซ้ำได้หลังลบ
pub async fn soft_delete_prename(
    pool: &PgPool,
    actor: UserId,
    id: PrenameId,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"update prename
           set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0
    )
    .execute(pool)
    .await?;
    Ok(())
}
