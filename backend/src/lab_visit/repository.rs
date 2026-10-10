//! query SQLx ต่อ lab_visit_group (head) + lab_visit (รายชื่อคนไข้ในนัดหมาย) — ห้ามรู้เรื่อง HTTP ใช้
//! query!/query_as! ตรวจตอน compile ไม่ประกอบ SQL ด้วย format! (docs/rust-guidelines.md ข้อ 11)
//! แถวที่ถูก soft delete (`deleted_at is not null`) ไม่ถูกคืนจากฟังก์ชันใดในนี้ และรายการคนไข้ join
//! `patient … deleted_at is null` เสมอ — คนไข้ที่ถูกลบจากทะเบียนจะไม่โผล่/ไม่ถูกนับในนัดหมาย
//!
//! โมดูลนี้ **ไม่อ่านคอลัมน์ข้อมูลระบุตัวคนไข้** (hn, nap_no, ชื่อ, สกุล) เลย — ใช้แค่ uuid ของคนไข้ หน้าเว็บ
//! join กับ `GET /patients` เอง (ถอดรหัสที่โมดูล patient ที่เดียว)
//!
//! การเขียนทุกครั้งเขียน `audit_log` ในทรานแซกชันเดียวกัน (ตรงจากที่นี่ เหมือน auth/patient — โมดูลฟีเจอร์
//! ไม่เรียกกันเอง) ไม่มี metadata มีแค่ action + uuid ของแถว

use chrono::NaiveDate;
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{LabVisitGroupId, LabVisitId, PatientId, UserId};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

impl RepositoryError {
    /// เช็คว่า error เกิดจาก unique constraint ชนไหม (ปี+กลุ่มซ้ำ / คนไข้ซ้ำในนัดหมาย) — ให้ service
    /// แปลงเป็น Conflict แทนที่จะ pre-check แข่งกับ concurrent request เอง
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, RepositoryError::Database(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()))
    }
}

/// นัดหมายหนึ่งรายการ (head) join ปีงบประมาณ/กลุ่มคนไข้ + จำนวนคนไข้ที่ยังไม่ถูกลบ
#[derive(Debug, Clone)]
pub struct GroupRow {
    pub id: LabVisitGroupId,
    /// uuid ของแถว — id ที่ API เห็น
    pub public_id: Uuid,
    pub budget_year_public_id: Uuid,
    pub budget_year: String,
    pub patient_group_public_id: Uuid,
    pub patient_group_name: String,
    pub cd4_first_date: NaiveDate,
    pub cd4_second_date: NaiveDate,
    pub vl_date: NaiveDate,
    pub sort_order: i32,
    pub patient_count: i64,
}

/// คนไข้หนึ่งคนในนัดหมาย — ผล VL/สูตรยาเป็นข้อมูลสุขภาพ จึงไม่ derive Debug (พิมพ์แค่ id)
#[derive(Clone)]
pub struct VisitRow {
    pub id: LabVisitId,
    /// uuid ของแถว lab_visit
    pub public_id: Uuid,
    pub patient_public_id: Uuid,
    pub take_med_year_range: Option<i32>,
    pub take_med_month_range: Option<i32>,
    pub vl_result: Option<String>,
    pub drug_recipe: Option<String>,
    pub sort_order: i32,
}

impl std::fmt::Debug for VisitRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VisitRow")
            .field("public_id", &self.public_id)
            .field("fields", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct BudgetYearOptionRow {
    pub public_id: Uuid,
    pub year: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct PatientGroupOptionRow {
    pub public_id: Uuid,
    pub name: String,
    pub is_active: bool,
}

/// ตัวเลือก master ของฟอร์มนัดหมาย (ทุกแถวที่ยังไม่ถูกลบ พร้อม is_active — ฟอร์มเลือกได้เฉพาะที่ active
/// แต่ต้องโชว์ค่าปัจจุบันของนัดหมายที่แก้อยู่แม้ถูกปิดใช้งานภายหลัง)
#[derive(Debug, Clone)]
pub struct MasterOptions {
    pub budget_years: Vec<BudgetYearOptionRow>,
    pub patient_groups: Vec<PatientGroupOptionRow>,
}

/// master ที่ resolve จาก uuid แล้ว (ยังไม่ถูกลบ)
#[derive(Debug, Clone, Copy)]
pub struct MasterLookup {
    pub id: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct PatientLookup {
    pub id: PatientId,
    pub public_id: Uuid,
    pub is_active: bool,
}

/// ค่าที่จะเขียนลง lab_visit_group (อ้าง master ด้วย id ภายในแล้ว)
#[derive(Debug, Clone, Copy)]
pub struct GroupWrite {
    pub budget_year_id: i32,
    pub patient_group_id: i32,
    pub cd4_first_date: NaiveDate,
    pub cd4_second_date: NaiveDate,
    pub vl_date: NaiveDate,
    /// สร้าง: None = ต่อท้ายลำดับสุดท้าย, แก้ไข: None = คงค่าเดิม
    pub sort_order: Option<i32>,
}

/// ค่าที่จะเขียนลง lab_visit (ตรวจ/trim แล้ว) — ไม่ derive Debug (ข้อมูลสุขภาพ)
pub struct VisitWrite {
    pub take_med_year_range: Option<i32>,
    pub take_med_month_range: Option<i32>,
    pub vl_result: Option<String>,
    pub drug_recipe: Option<String>,
    /// None = คงค่าเดิม
    pub sort_order: Option<i32>,
}

pub async fn list_groups(pool: &PgPool) -> Result<Vec<GroupRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        GroupRow,
        r#"select g.id as "id: LabVisitGroupId", g.uuid as public_id,
                  y.uuid as "budget_year_public_id!", y.year as "budget_year!",
                  pg.uuid as "patient_group_public_id!", pg.name as "patient_group_name!",
                  g.cd4_1st_date as cd4_first_date, g.cd4_2nd_date as cd4_second_date, g.vl_date,
                  g.sort_order,
                  (select count(*)
                     from lab_visit v
                     join patient p on p.id = v.patient_id
                    where v.lab_visit_group_id = g.id
                      and v.deleted_at is null and p.deleted_at is null) as "patient_count!"
           from lab_visit_group g
           join budget_year y on y.id = g.budget_year_id
           join patient_group pg on pg.id = g.patient_group_id
           where g.deleted_at is null
           order by g.sort_order, g.id"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_group(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<GroupRow>, RepositoryError> {
    let row = sqlx::query_as!(
        GroupRow,
        r#"select g.id as "id: LabVisitGroupId", g.uuid as public_id,
                  y.uuid as "budget_year_public_id!", y.year as "budget_year!",
                  pg.uuid as "patient_group_public_id!", pg.name as "patient_group_name!",
                  g.cd4_1st_date as cd4_first_date, g.cd4_2nd_date as cd4_second_date, g.vl_date,
                  g.sort_order,
                  (select count(*)
                     from lab_visit v
                     join patient p on p.id = v.patient_id
                    where v.lab_visit_group_id = g.id
                      and v.deleted_at is null and p.deleted_at is null) as "patient_count!"
           from lab_visit_group g
           join budget_year y on y.id = g.budget_year_id
           join patient_group pg on pg.id = g.patient_group_id
           where g.uuid = $1 and g.deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// รายชื่อคนไข้ในนัดหมาย (เฉพาะคนไข้ที่ยังไม่ถูกลบจากทะเบียน) เรียง sort_order แล้ว id
pub async fn list_visits(
    pool: &PgPool,
    group_id: LabVisitGroupId,
) -> Result<Vec<VisitRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        VisitRow,
        r#"select v.id as "id: LabVisitId", v.uuid as public_id, p.uuid as "patient_public_id!",
                  v.take_med_year_range, v.take_med_month_range, v.vl_result, v.drug_recipe,
                  v.sort_order
           from lab_visit v
           join patient p on p.id = v.patient_id
           where v.lab_visit_group_id = $1 and v.deleted_at is null and p.deleted_at is null
           order by v.sort_order, v.id"#,
        group_id.0
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_budget_year(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<MasterLookup>, RepositoryError> {
    let row = sqlx::query_as!(
        MasterLookup,
        "select id, is_active from budget_year where uuid = $1 and deleted_at is null",
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_patient_group(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<MasterLookup>, RepositoryError> {
    let row = sqlx::query_as!(
        MasterLookup,
        "select id, is_active from patient_group where uuid = $1 and deleted_at is null",
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_master_options(pool: &PgPool) -> Result<MasterOptions, RepositoryError> {
    let budget_years = sqlx::query_as!(
        BudgetYearOptionRow,
        r#"select uuid as public_id, year, start_date, end_date, is_active
           from budget_year
           where deleted_at is null
           order by sort_order, year desc"#
    )
    .fetch_all(pool)
    .await?;
    let patient_groups = sqlx::query_as!(
        PatientGroupOptionRow,
        r#"select uuid as public_id, name, is_active
           from patient_group
           where deleted_at is null
           order by sort_order, name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(MasterOptions {
        budget_years,
        patient_groups,
    })
}

/// คนไข้ที่มีอยู่จริง (ยังไม่ถูกลบ) จากรายการ uuid — ตัวที่ไม่พบจะไม่อยู่ในผลลัพธ์
pub async fn find_patients(
    pool: &PgPool,
    public_ids: &[Uuid],
) -> Result<Vec<PatientLookup>, RepositoryError> {
    let rows = sqlx::query_as!(
        PatientLookup,
        r#"select id as "id: PatientId", uuid as public_id, is_active
           from patient
           where uuid = any($1) and deleted_at is null"#,
        public_ids
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// id ภายในของคนไข้ที่อยู่ในนัดหมายนี้แล้ว (แถว lab_visit ที่ยังไม่ถูกลบ)
pub async fn group_patient_ids(
    pool: &PgPool,
    group_id: LabVisitGroupId,
) -> Result<Vec<PatientId>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select patient_id as "patient_id: PatientId"
           from lab_visit
           where lab_visit_group_id = $1 and deleted_at is null"#,
        group_id.0
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.patient_id).collect())
}

/// แถว lab_visit ที่แก้/ลบได้ — ตัวมันเอง นัดหมายของมัน และคนไข้ของมันต้องยังไม่ถูกลบ
pub async fn find_visit_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<LabVisitId>, RepositoryError> {
    let row = sqlx::query!(
        r#"select v.id as "id: LabVisitId"
           from lab_visit v
           join lab_visit_group g on g.id = v.lab_visit_group_id
           join patient p on p.id = v.patient_id
           where v.uuid = $1 and v.deleted_at is null
             and g.deleted_at is null and p.deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.id))
}

async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    action: &str,
    target_type: &str,
    target_public_id: Uuid,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id)
         values ($1, $2, $3, $4)",
        actor.0,
        action,
        target_type,
        target_public_id.to_string()
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// สร้างนัดหมาย + audit ในทรานแซกชันเดียว — คืน uuid ของนัดหมายใหม่
pub async fn insert_group(
    pool: &PgPool,
    actor: UserId,
    w: &GroupWrite,
) -> Result<Uuid, RepositoryError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query!(
        r#"insert into lab_visit_group
             (budget_year_id, patient_group_id, cd4_1st_date, cd4_2nd_date, vl_date, sort_order,
              created_by, updated_by)
           values
             ($1, $2, $3, $4, $5,
              coalesce($6, (select coalesce(max(sort_order), 0) + 1
                              from lab_visit_group where deleted_at is null)),
              $7, $7)
           returning uuid"#,
        w.budget_year_id,
        w.patient_group_id,
        w.cd4_first_date,
        w.cd4_second_date,
        w.vl_date,
        w.sort_order,
        actor.0
    )
    .fetch_one(&mut *tx)
    .await?;
    insert_audit(
        &mut tx,
        actor,
        "lab_visit_group_created",
        "lab_visit_group",
        row.uuid,
    )
    .await?;
    tx.commit().await?;
    Ok(row.uuid)
}

/// แทนที่ข้อมูลหัวนัดหมาย (`sort_order` None = คงค่าเดิม) + audit ในทรานแซกชันเดียว
pub async fn update_group(
    pool: &PgPool,
    actor: UserId,
    id: LabVisitGroupId,
    public_id: Uuid,
    w: &GroupWrite,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"update lab_visit_group
           set budget_year_id = $3, patient_group_id = $4,
               cd4_1st_date = $5, cd4_2nd_date = $6, vl_date = $7,
               sort_order = coalesce($8, sort_order),
               updated_by = $2, updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0,
        w.budget_year_id,
        w.patient_group_id,
        w.cd4_first_date,
        w.cd4_second_date,
        w.vl_date,
        w.sort_order
    )
    .execute(&mut *tx)
    .await?;
    insert_audit(
        &mut tx,
        actor,
        "lab_visit_group_updated",
        "lab_visit_group",
        public_id,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// soft delete นัดหมายพร้อมรายชื่อคนไข้ทั้งหมดของมัน + audit ในทรานแซกชันเดียว — partial unique ของ
/// ปี+กลุ่มนับเฉพาะแถวที่ยังไม่ถูกลบ จึงสร้างนัดหมายของปี/กลุ่มเดิมใหม่ได้
pub async fn soft_delete_group(
    pool: &PgPool,
    actor: UserId,
    id: LabVisitGroupId,
    public_id: Uuid,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "update lab_visit
         set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
         where lab_visit_group_id = $1 and deleted_at is null",
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "update lab_visit_group
         set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
         where id = $1 and deleted_at is null",
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    insert_audit(
        &mut tx,
        actor,
        "lab_visit_group_deleted",
        "lab_visit_group",
        public_id,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// เพิ่มคนไข้หลายคนเข้านัดหมาย (ลำดับต่อท้ายตามลำดับที่ส่งมา) + audit ต่อแถว ในทรานแซกชันเดียว
/// ถ้าคนไหนอยู่ในนัดหมายแล้ว unique index (`lab_visit_group_patient_uq`) ทำให้ทั้งชุดถูกยกเลิก
pub async fn insert_visits(
    pool: &PgPool,
    actor: UserId,
    group_id: LabVisitGroupId,
    patient_ids: &[PatientId],
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let last = sqlx::query!(
        r#"select coalesce(max(sort_order), 0) as "last!"
           from lab_visit
           where lab_visit_group_id = $1 and deleted_at is null"#,
        group_id.0
    )
    .fetch_one(&mut *tx)
    .await?
    .last;

    for (position, patient_id) in patient_ids.iter().enumerate() {
        let offset = i32::try_from(position).unwrap_or(i32::MAX - 1) + 1;
        let row = sqlx::query!(
            r#"insert into lab_visit
                 (patient_id, lab_visit_group_id, sort_order, created_by, updated_by)
               values ($1, $2, $3, $4, $4)
               returning uuid"#,
            patient_id.0,
            group_id.0,
            last.saturating_add(offset),
            actor.0
        )
        .fetch_one(&mut *tx)
        .await?;
        insert_audit(&mut tx, actor, "lab_visit_created", "lab_visit", row.uuid).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// แทนที่ข้อมูลของคนไข้หนึ่งคนในนัดหมาย (ฟิลด์ที่ None = ล้างค่า ยกเว้น `sort_order` None = คงเดิม) + audit
pub async fn update_visit(
    pool: &PgPool,
    actor: UserId,
    id: LabVisitId,
    public_id: Uuid,
    w: &VisitWrite,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        r#"update lab_visit
           set take_med_year_range = $3, take_med_month_range = $4,
               vl_result = $5, drug_recipe = $6,
               sort_order = coalesce($7, sort_order),
               updated_by = $2, updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0,
        w.take_med_year_range,
        w.take_med_month_range,
        w.vl_result,
        w.drug_recipe,
        w.sort_order
    )
    .execute(&mut *tx)
    .await?;
    insert_audit(&mut tx, actor, "lab_visit_updated", "lab_visit", public_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn soft_delete_visit(
    pool: &PgPool,
    actor: UserId,
    id: LabVisitId,
    public_id: Uuid,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "update lab_visit
         set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
         where id = $1 and deleted_at is null",
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    insert_audit(&mut tx, actor, "lab_visit_deleted", "lab_visit", public_id).await?;
    tx.commit().await?;
    Ok(())
}
