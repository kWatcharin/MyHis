//! query SQLx ต่อ patient + medical_condition (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! ตรวจตอน
//! compile ไม่ประกอบ SQL ด้วย format! (docs/rust-guidelines.md ข้อ 11) แถวที่ถูก soft delete
//! (`deleted_at is not null`) ไม่ถูกคืนจากฟังก์ชันใดในนี้
//!
//! ที่นี่เก็บ/อ่านค่าที่ **เข้ารหัสแล้ว** (`Vec<u8>`) เท่านั้น — การเข้า/ถอดรหัสอยู่ที่ service
//! ชนิดในไฟล์นี้ไม่ derive `Debug` โดยตั้งใจ (กันพิมพ์ ciphertext/ค่าระบุตัวลง log)
//!
//! การเขียนผู้ป่วย (สร้าง/แทนที่/ลบ) ทำพร้อมการซิงก์โรคประจำตัวและแถว `audit_log` ในทรานแซกชันเดียว
//! (เขียน audit_log ตรงจากที่นี่ เหมือน auth — โมดูลฟีเจอร์ไม่เรียกกันเอง) audit ไม่มี metadata เพราะ
//! ห้ามมีข้อมูลระบุตัวคนไข้ในนั้น มีแค่ action + uuid ของผู้ป่วย

use chrono::NaiveDate;
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{changes, PatientId, UserId};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

impl RepositoryError {
    /// เช็คว่า error เกิดจาก unique constraint ชนไหม (HN ซ้ำ — `patient_hn_idx_uq`) — ให้ service
    /// แปลงเป็น Conflict แทนที่จะ pre-check แข่งกับ concurrent request เอง
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, RepositoryError::Database(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()))
    }
}

/// แถวผู้ป่วยตามที่เก็บใน DB — คอลัมน์ encrypt=yes ยังเป็น ciphertext
pub struct PatientRow {
    pub id: PatientId,
    /// uuid ของแถว — id ที่ API เห็น
    pub public_id: Uuid,
    pub hn: Vec<u8>,
    pub nap_no: Vec<u8>,
    pub prename_public_id: Uuid,
    pub prename_name: String,
    pub first_name: Vec<u8>,
    pub last_name: Vec<u8>,
    pub birth_date: Option<NaiveDate>,
    pub age: Option<i32>,
    pub weight: Option<f64>,
    pub height: Option<f64>,
    pub sex: i32,
    pub is_thai: bool,
    pub nationality_public_id: Option<Uuid>,
    pub nationality_name: Option<String>,
    pub sort_order: i32,
    pub is_active: bool,
}

/// โรคประจำตัวหนึ่งรายการของผู้ป่วย (join ชื่อโรคมาแล้ว — ชื่อโรคไม่ใช่ข้อมูลระบุตัวคนไข้)
#[derive(Debug, Clone)]
pub struct ConditionRow {
    pub patient_id: PatientId,
    /// uuid ของแถว medical_condition
    pub public_id: Uuid,
    pub disease_public_id: Uuid,
    pub name: String,
    pub abbreviation: String,
}

#[derive(Debug, Clone)]
pub struct PrenameOptionRow {
    pub public_id: Uuid,
    pub name: String,
    pub abbreviation: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NationalityOptionRow {
    pub public_id: Uuid,
    pub national_name: String,
    pub nation: String,
}

#[derive(Debug, Clone)]
pub struct DiseaseOptionRow {
    pub public_id: Uuid,
    pub name: String,
    pub abbreviation: String,
}

/// ตัวเลือกของ master สำหรับฟอร์มลงทะเบียน (สัญชาติ/โรคเฉพาะที่ `is_active`)
#[derive(Debug, Clone)]
pub struct MasterOptions {
    pub prenames: Vec<PrenameOptionRow>,
    pub nationalities: Vec<NationalityOptionRow>,
    pub diseases: Vec<DiseaseOptionRow>,
}

/// ค่าที่จะเขียนลง patient (เข้ารหัส/คำนวณ blind index แล้ว, อ้าง master ด้วย id ภายในแล้ว)
pub struct PatientWrite {
    pub hn: Vec<u8>,
    pub hn_idx: Vec<u8>,
    pub nap_no: Vec<u8>,
    pub nap_no_idx: Vec<u8>,
    pub prename_id: i32,
    pub first_name: Vec<u8>,
    pub last_name: Vec<u8>,
    pub birth_date: Option<NaiveDate>,
    pub age: Option<i32>,
    pub weight: Option<f64>,
    pub height: Option<f64>,
    pub sex: i32,
    pub is_thai: bool,
    pub nationality_id: Option<i32>,
    /// สร้าง: None = ต่อท้ายลำดับสุดท้าย, แก้ไข: None = คงค่าเดิม
    pub sort_order: Option<i32>,
    pub is_active: bool,
    /// id ภายในของ disease ตามลำดับที่ต้องการ (ไม่ซ้ำ)
    pub disease_ids: Vec<i32>,
}

pub async fn list_patients(pool: &PgPool) -> Result<Vec<PatientRow>, RepositoryError> {
    // weight/height เป็น numeric(5,2) — cast เป็น float8 ตอนอ่านเพื่อไม่ต้องเพิ่ม dependency ชนิด decimal
    let rows = sqlx::query_as!(
        PatientRow,
        r#"select p.id as "id: PatientId", p.uuid as public_id, p.hn, p.nap_no,
                  pr.uuid as "prename_public_id!", pr.name as "prename_name!",
                  p.first_name, p.last_name, p.birth_date, p.age,
                  p.weight::float8 as "weight?", p.height::float8 as "height?",
                  p.sex, p.is_thai,
                  n.uuid as "nationality_public_id?", n.national_name as "nationality_name?",
                  p.sort_order, p.is_active
           from patient p
           join prename pr on pr.id = p.prename_id
           left join nationality n on n.id = p.nationality_id
           where p.deleted_at is null
           order by p.sort_order, p.id"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// โรคประจำตัวที่ยังไม่ถูกลบของผู้ป่วยทุกคน (query เดียว ไม่ N+1) เรียงตามผู้ป่วยแล้วลำดับโรค
pub async fn list_conditions(pool: &PgPool) -> Result<Vec<ConditionRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        ConditionRow,
        r#"select mc.patient_id as "patient_id: PatientId", mc.uuid as public_id,
                  d.uuid as disease_public_id, d.name, d.abbreviation
           from medical_condition mc
           join disease d on d.id = mc.disease_id
           where mc.deleted_at is null
           order by mc.patient_id, mc.sort_order, mc.id"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_patient_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<PatientId>, RepositoryError> {
    let row = sqlx::query!(
        r#"select id as "id: PatientId" from patient where uuid = $1 and deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.id))
}

pub async fn find_prename_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<i32>, RepositoryError> {
    let row = sqlx::query!(
        "select id from prename where uuid = $1 and deleted_at is null",
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.id))
}

pub async fn find_nationality_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<i32>, RepositoryError> {
    let row = sqlx::query!(
        "select id from nationality where uuid = $1 and deleted_at is null",
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.id))
}

pub struct DiseaseLookup {
    pub id: i32,
    pub public_id: Uuid,
    pub is_active: bool,
}

/// โรคที่มีอยู่จริง (ยังไม่ถูกลบ) จากรายการ uuid — ตัวที่ไม่พบจะไม่อยู่ในผลลัพธ์
pub async fn find_diseases(
    pool: &PgPool,
    public_ids: &[Uuid],
) -> Result<Vec<DiseaseLookup>, RepositoryError> {
    let rows = sqlx::query_as!(
        DiseaseLookup,
        r#"select id, uuid as public_id, is_active
           from disease
           where uuid = any($1) and deleted_at is null"#,
        public_ids
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// uuid ของโรคที่ผู้ป่วยคนนี้ผูกอยู่ตอนนี้ — ใช้ตัดสินว่าโรค inactive ตัวไหน "คงไว้ได้" (ผูกอยู่แล้ว)
pub async fn linked_disease_public_ids(
    pool: &PgPool,
    patient_id: PatientId,
) -> Result<Vec<Uuid>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select d.uuid
           from medical_condition mc
           join disease d on d.id = mc.disease_id
           where mc.patient_id = $1 and mc.deleted_at is null"#,
        patient_id.0
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.uuid).collect())
}

pub async fn list_master_options(pool: &PgPool) -> Result<MasterOptions, RepositoryError> {
    let prenames = sqlx::query_as!(
        PrenameOptionRow,
        r#"select uuid as public_id, name, abbreviation
           from prename
           where deleted_at is null
           order by sort_order, name"#
    )
    .fetch_all(pool)
    .await?;
    let nationalities = sqlx::query_as!(
        NationalityOptionRow,
        r#"select uuid as public_id, national_name, nation
           from nationality
           where deleted_at is null and is_active
           order by sort_order, national_name"#
    )
    .fetch_all(pool)
    .await?;
    let diseases = sqlx::query_as!(
        DiseaseOptionRow,
        r#"select uuid as public_id, name, abbreviation
           from disease
           where deleted_at is null and is_active
           order by sort_order, name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(MasterOptions {
        prenames,
        nationalities,
        diseases,
    })
}

async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    action: &str,
    patient_public_id: Uuid,
    details: Option<Value>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id, metadata)
         values ($1, $2, 'patient', $3, $4)",
        actor.0,
        action,
        patient_public_id.to_string(),
        details
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// ทำให้โรคประจำตัวที่ยังไม่ถูกลบของผู้ป่วย "ตรงกับ" ชุดที่ต้องการ: ที่ไม่อยู่ในชุด → soft delete,
/// ที่ใหม่ → insert, ที่คงอยู่ → ปรับ sort_order ตามลำดับที่ส่งมา (partial unique ของ medical_condition
/// นับเฉพาะแถวที่ยังไม่ถูกลบ จึงผูกโรคที่เคยถอดไปแล้วกลับมาได้)
async fn sync_conditions(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    patient_id: PatientId,
    desired: &[i32],
) -> Result<(), RepositoryError> {
    let existing = sqlx::query!(
        "select id, disease_id from medical_condition
         where patient_id = $1 and deleted_at is null",
        patient_id.0
    )
    .fetch_all(&mut **tx)
    .await?;

    for row in &existing {
        if !desired.contains(&row.disease_id) {
            sqlx::query!(
                "update medical_condition
                 set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
                 where id = $1",
                row.id,
                actor.0
            )
            .execute(&mut **tx)
            .await?;
        }
    }

    for (position, disease_id) in desired.iter().enumerate() {
        let sort_order = i32::try_from(position).unwrap_or(i32::MAX);
        match existing.iter().find(|row| row.disease_id == *disease_id) {
            Some(row) => {
                sqlx::query!(
                    "update medical_condition
                     set sort_order = $2, updated_by = $3, updated_at = now()
                     where id = $1",
                    row.id,
                    sort_order,
                    actor.0
                )
                .execute(&mut **tx)
                .await?;
            }
            None => {
                sqlx::query!(
                    "insert into medical_condition
                       (disease_id, patient_id, sort_order, created_by, updated_by)
                     values ($1, $2, $3, $4, $4)",
                    disease_id,
                    patient_id.0,
                    sort_order,
                    actor.0
                )
                .execute(&mut **tx)
                .await?;
            }
        }
    }
    Ok(())
}

/// สร้างผู้ป่วย + ผูกโรค + audit ในทรานแซกชันเดียว — คืน uuid ของผู้ป่วย
pub async fn insert_patient(
    pool: &PgPool,
    actor: UserId,
    w: &PatientWrite,
) -> Result<Uuid, RepositoryError> {
    let mut tx = pool.begin().await?;

    let row = sqlx::query!(
        r#"insert into patient
             (hn, hn_idx, nap_no, nap_no_idx, sort_order, prename_id, first_name, last_name,
              birth_date, age, weight, height, sex, is_thai, nationality_id, is_active,
              created_by, updated_by)
           values
             ($1, $2, $3, $4,
              coalesce($5, (select coalesce(max(sort_order), 0) + 1 from patient where deleted_at is null)),
              $6, $7, $8, $9, $10, $11::float8::numeric(5,2), $12::float8::numeric(5,2),
              $13, $14, $15, $16, $17, $17)
           returning id as "id: PatientId", uuid"#,
        w.hn,
        w.hn_idx,
        w.nap_no,
        w.nap_no_idx,
        w.sort_order,
        w.prename_id,
        w.first_name,
        w.last_name,
        w.birth_date,
        w.age,
        w.weight,
        w.height,
        w.sex,
        w.is_thai,
        w.nationality_id,
        w.is_active,
        actor.0
    )
    .fetch_one(&mut *tx)
    .await?;

    sync_conditions(&mut tx, actor, row.id, &w.disease_ids).await?;
    insert_audit(&mut tx, actor, "patient_created", row.uuid, None).await?;
    tx.commit().await?;
    Ok(row.uuid)
}

/// แทนที่ข้อมูลผู้ป่วยทั้งชุด + โรคประจำตัว + audit ในทรานแซกชันเดียว
/// `changed_names` = ชื่อช่องชื่อ-สกุลที่ service เทียบแล้วว่าเปลี่ยน (ciphertext สุ่ม nonce ทุกครั้ง เทียบใน DB ไม่ได้)
pub async fn replace_patient(
    pool: &PgPool,
    actor: UserId,
    id: PatientId,
    public_id: Uuid,
    w: &PatientWrite,
    changed_names: &[&str],
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let before = snapshot(&mut tx, id).await?;

    sqlx::query!(
        r#"update patient
           set hn = $3, hn_idx = $4, nap_no = $5, nap_no_idx = $6,
               sort_order = coalesce($7, sort_order),
               prename_id = $8, first_name = $9, last_name = $10,
               birth_date = $11, age = $12,
               weight = $13::float8::numeric(5,2), height = $14::float8::numeric(5,2),
               sex = $15, is_thai = $16, nationality_id = $17, is_active = $18,
               updated_by = $2, updated_at = now()
           where id = $1 and deleted_at is null"#,
        id.0,
        actor.0,
        w.hn,
        w.hn_idx,
        w.nap_no,
        w.nap_no_idx,
        w.sort_order,
        w.prename_id,
        w.first_name,
        w.last_name,
        w.birth_date,
        w.age,
        w.weight,
        w.height,
        w.sex,
        w.is_thai,
        w.nationality_id,
        w.is_active
    )
    .execute(&mut *tx)
    .await?;

    sync_conditions(&mut tx, actor, id, &w.disease_ids).await?;
    // ข้อมูลผู้ป่วย — เก็บแค่ชื่อช่องที่เปลี่ยน ไม่เก็บค่า (CLAUDE.md)
    let after = snapshot(&mut tx, id).await?;
    let details = changes::changed_fields(&before, &after, changed_names);
    insert_audit(&mut tx, actor, "patient_updated", public_id, Some(details)).await?;
    tx.commit().await?;
    Ok(())
}

/// soft delete ผู้ป่วยพร้อมโรคประจำตัวของเขา + audit ในทรานแซกชันเดียว — partial unique ของ HN
/// นับเฉพาะแถวที่ยังไม่ถูกลบ จึงใช้ HN เดิมลงทะเบียนใหม่ได้
pub async fn soft_delete_patient(
    pool: &PgPool,
    actor: UserId,
    id: PatientId,
    public_id: Uuid,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        "update medical_condition
         set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
         where patient_id = $1 and deleted_at is null",
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        "update patient
         set deleted_at = now(), deleted_by = $2, updated_by = $2, updated_at = now()
         where id = $1 and deleted_at is null",
        id.0,
        actor.0
    )
    .execute(&mut *tx)
    .await?;

    insert_audit(&mut tx, actor, "patient_deleted", public_id, None).await?;
    tx.commit().await?;
    Ok(())
}

/// ชื่อ-สกุล (ciphertext) ของผู้ป่วยหนึ่งคน — ให้ service ถอดรหัสเทียบกับค่าใหม่ หาว่าช่องไหนเปลี่ยน (แถวเดียว)
pub async fn find_patient_names(
    pool: &PgPool,
    id: PatientId,
) -> Result<Option<(Vec<u8>, Vec<u8>)>, RepositoryError> {
    let row = sqlx::query!(
        "select first_name, last_name from patient where id = $1 and deleted_at is null",
        id.0
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| (r.first_name, r.last_name)))
}

/// ค่าของผู้ป่วยไว้เทียบหาชื่อช่องที่เปลี่ยนเท่านั้น — อยู่ในหน่วยความจำระหว่างทรานแซกชัน **ไม่ถูกบันทึก**
/// (HN/NAP NO เทียบด้วย blind index, ชื่อ-สกุลเทียบที่ service) ล็อกแถวไว้จนจบทรานแซกชัน
async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    id: PatientId,
) -> Result<Value, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"select jsonb_build_object(
                  'hn', encode(p.hn_idx, 'hex'), 'nap_no', encode(p.nap_no_idx, 'hex'),
                  'prename', p.prename_id, 'birth_date', p.birth_date, 'age', p.age,
                  'weight', p.weight, 'height', p.height, 'sex', p.sex, 'is_thai', p.is_thai,
                  'nationality', p.nationality_id, 'sort_order', p.sort_order, 'is_active', p.is_active,
                  'diseases', coalesce((select jsonb_agg(mc.disease_id order by mc.disease_id)
                                        from medical_condition mc
                                        where mc.patient_id = p.id and mc.deleted_at is null), '[]'::jsonb)
              ) as "snapshot!"
           from patient p where p.id = $1 for update of p"#,
        id.0
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.unwrap_or(Value::Null))
}
