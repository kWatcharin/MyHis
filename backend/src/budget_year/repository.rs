//! query SQLx ต่อตาราง budget_year (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! ตรวจตอน compile
//! ไม่ประกอบ SQL ด้วย format! (docs/rust-guidelines.md ข้อ 11) แถวที่ถูก soft delete
//! (`deleted_at is not null`) ไม่ถูกคืนจากฟังก์ชันใดในนี้

use chrono::NaiveDate;
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{BudgetYearId, UserId};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

impl RepositoryError {
    /// เช็คว่า error เกิดจาก unique constraint ชนไหม (ปีซ้ำ) — ให้ service แปลงเป็น Conflict
    /// แทนที่จะ pre-check แข่งกับ concurrent request เอง
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, RepositoryError::Database(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()))
    }
}

#[derive(Debug, Clone)]
pub struct BudgetYearRow {
    pub id: BudgetYearId,
    /// uuid ของแถว — id ที่ API เห็น
    pub public_id: Uuid,
    pub year: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub sort_order: i32,
    pub is_active: bool,
}

pub async fn list_budget_years(pool: &PgPool) -> Result<Vec<BudgetYearRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        BudgetYearRow,
        r#"select id as "id: BudgetYearId", uuid as public_id, year, start_date, end_date,
                  sort_order, is_active
           from budget_year
           where deleted_at is null
           order by sort_order, year desc"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_budget_year_by_public_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<BudgetYearRow>, RepositoryError> {
    let row = sqlx::query_as!(
        BudgetYearRow,
        r#"select id as "id: BudgetYearId", uuid as public_id, year, start_date, end_date,
                  sort_order, is_active
           from budget_year
           where uuid = $1 and deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn insert_budget_year(
    pool: &PgPool,
    actor: UserId,
    year: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    sort_order: i32,
    is_active: bool,
) -> Result<BudgetYearId, RepositoryError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query!(
        r#"insert into budget_year
             (year, start_date, end_date, sort_order, is_active, created_by, updated_by)
           values ($1, $2, $3, $4, $5, $6, $6)
           returning id as "id: BudgetYearId""#,
        year,
        start_date,
        end_date,
        sort_order,
        is_active,
        actor.0
    )
    .fetch_one(&mut *tx)
    .await?;
    insert_audit(&mut tx, actor, "budget_year_created", row.id).await?;
    tx.commit().await?;
    Ok(row.id)
}

pub struct UpdateBudgetYearFields<'a> {
    pub year: Option<&'a str>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

/// partial update ผ่าน COALESCE — ค่า None ไม่แตะคอลัมน์เดิม (ทุกคอลัมน์ที่แก้ได้เป็น not null
/// จึงไม่มีกรณี "ตั้งเป็น null")
pub async fn update_budget_year(
    pool: &PgPool,
    actor: UserId,
    id: BudgetYearId,
    fields: UpdateBudgetYearFields<'_>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query!(
        r#"update budget_year
           set year = coalesce($3, year),
               start_date = coalesce($4, start_date),
               end_date = coalesce($5, end_date),
               sort_order = coalesce($6, sort_order),
               is_active = coalesce($7, is_active),
               updated_by = $2,
               updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0,
        fields.year,
        fields.start_date,
        fields.end_date,
        fields.sort_order,
        fields.is_active
    )
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() > 0 {
        insert_audit(&mut tx, actor, "budget_year_updated", id).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// ปีนี้ยังถูกนัดหมาย lab (แถวที่ยังไม่ถูกลบ) อ้างอยู่ไหม — ใช้กันลบปีที่ยังมีข้อมูลผูกอยู่
pub async fn budget_year_in_use(pool: &PgPool, id: BudgetYearId) -> Result<bool, RepositoryError> {
    let row = sqlx::query!(
        r#"select exists(
             select 1 from lab_visit_group where budget_year_id = $1 and deleted_at is null
           ) as "exists!""#,
        id.0
    )
    .fetch_one(pool)
    .await?;
    Ok(row.exists)
}

/// soft delete — ไม่ลบแถวจริง (CLAUDE.md) partial unique index นับเฉพาะแถวที่ยังไม่ถูกลบ
/// จึงสร้างปีเดิมซ้ำได้หลังลบ
pub async fn soft_delete_budget_year(
    pool: &PgPool,
    actor: UserId,
    id: BudgetYearId,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query!(
        r#"update budget_year
           set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() > 0 {
        insert_audit(&mut tx, actor, "budget_year_deleted", id).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// audit ของหน้าตั้งค่าปีงบประมาณ (หน้า "ประวัติการใช้งาน") — target_id = uuid ของแถว ไม่มี metadata
/// ชื่อรายการอ่านจาก join ตอนแสดงผล (soft delete แถวจึงยังอยู่เสมอ) เขียนในทรานแซกชันเดียวกับการแก้ข้อมูล
async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    action: &str,
    id: BudgetYearId,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id)
         select $1, $2, 'budget_year', uuid::text from budget_year where id = $3",
        actor.0,
        action,
        id.0
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
