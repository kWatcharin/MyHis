//! กฎของนัดหมายแล็ป: สิทธิ์ (`lab_visit.view/create/update/delete` ตรวจแยกกัน), master ที่อ้างถึง,
//! วันนัด, คนไข้ในนัดหมาย, ช่วงค่า/ความยาวของข้อมูลรายคน ห้าม import ชนิดข้อมูลของ Axum
//! (docs/auth-design.md ข้อ 3)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` และเช็คสิทธิ์ที่นี่ — สร้างนัดหมาย = `create`,
//! แก้หัวนัดหมายและเพิ่ม/แก้/เอาคนไข้ออกจากนัดหมาย = `update`, ลบนัดหมาย = `delete`, บันทึกการส่งออกเอกสาร =
//! `export` หน้าเว็บที่แสดงชื่อ/HN
//! คนไข้ join กับ `GET /patients` จึงต้องมี `patient.view` ด้วย (ตรวจที่ endpoint นั้น)
//!
//! วันนัด CD4 ครั้งที่ 2 ค่าเริ่มต้น "ครั้งที่ 1 + 6 เดือน" เป็นแค่ค่าตั้งต้นในฟอร์ม — ที่นี่บังคับแค่ว่าครั้งที่ 2
//! ต้องหลังครั้งที่ 1 (ผู้ใช้เลื่อนวันจริงได้)

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::domain::{Action, PatientId, Resource};
use crate::lab_visit::repository::{
    self, GroupRow, GroupWrite, MasterOptions, RepositoryError, VisitRow, VisitWrite,
};

/// เพิ่มคนไข้ต่อครั้งได้ไม่เกินนี้ (กันคำขอใหญ่ผิดปกติ — กลุ่มจริงมีหลักสิบคน)
pub const MAX_PATIENTS_PER_REQUEST: usize = 500;
const MAX_TAKE_MED_YEARS: i32 = 100;
/// ส่วนที่เป็นเดือนของระยะเวลากินยา (ปีแยกอีกช่อง) จึงไม่เกิน 11
const MAX_TAKE_MED_MONTHS: i32 = 11;
const MAX_VL_RESULT_CHARS: usize = 100;
const MAX_DRUG_RECIPE_CHARS: usize = 200;

#[derive(Debug, thiserror::Error)]
pub enum LabVisitError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("lab visit group not found")]
    GroupNotFound,
    #[error("lab visit not found")]
    VisitNotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// ตรวจสิทธิ์ของ action นี้ (`lab_visit.*`) — ตรวจแยกกันทุกตัว แก้ไขไม่ implies ดู
/// (export ตรวจตอนบันทึกการส่งออกที่ `record_export` — ตัวเอกสารสร้างฝั่งหน้าเว็บจากข้อมูลที่อ่านด้วยสิทธิ์ดู)
fn require(actor: &CurrentUser, action: Action) -> Result<(), LabVisitError> {
    if actor.can(Resource::LabVisit, action) {
        Ok(())
    } else {
        Err(LabVisitError::Forbidden)
    }
}

fn validation(message: &str) -> LabVisitError {
    LabVisitError::Validation(message.to_string())
}

/// unique violation → Conflict ด้วยข้อความของจุดนั้น (ปี+กลุ่มซ้ำ / คนไข้ซ้ำในนัดหมาย)
fn conflict_on_unique(message: &'static str) -> impl Fn(RepositoryError) -> LabVisitError {
    move |err| {
        if err.is_unique_violation() {
            LabVisitError::Conflict(message.to_string())
        } else {
            LabVisitError::from(err)
        }
    }
}

/// หัวนัดหมายพร้อมรายชื่อคนไข้ (ยังไม่ถูกลบ) เรียงตามลำดับ
#[derive(Debug)]
pub struct GroupDetail {
    pub group: GroupRow,
    pub visits: Vec<VisitRow>,
}

/// ข้อมูลหัวนัดหมายที่ผู้ใช้กรอก (uuid ของ master ยังไม่ resolve)
#[derive(Debug, Clone, Copy)]
pub struct GroupInput {
    pub budget_year_id: Uuid,
    pub patient_group_id: Uuid,
    pub cd4_first_date: NaiveDate,
    pub cd4_second_date: NaiveDate,
    pub vl_date: NaiveDate,
    /// สร้าง: None = ต่อท้ายลำดับสุดท้าย, แก้ไข: None = คงลำดับเดิม
    pub sort_order: Option<i32>,
}

/// ข้อมูลรายคนในนัดหมาย (แทนที่ทั้งชุด: None = ล้างค่า ยกเว้น `sort_order` None = คงเดิม) — Debug redact
pub struct VisitInput<'a> {
    pub take_med_year_range: Option<i32>,
    pub take_med_month_range: Option<i32>,
    pub vl_result: Option<&'a str>,
    pub drug_recipe: Option<&'a str>,
    pub sort_order: Option<i32>,
}

impl std::fmt::Debug for VisitInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VisitInput")
            .field("fields", &"[redacted]")
            .finish()
    }
}

/// trim แล้ว ว่าง = None; ยาวเกิน `max` ตัวอักษร (ไม่ใช่ byte) → error ที่ไม่แนบค่า
fn optional_text(
    value: Option<&str>,
    field: &str,
    max: usize,
) -> Result<Option<String>, LabVisitError> {
    let Some(trimmed) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    if trimmed.chars().count() > max {
        return Err(LabVisitError::Validation(format!(
            "{field} must be at most {max} characters"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

fn check_range(value: Option<i32>, max: i32, message: &str) -> Result<(), LabVisitError> {
    match value {
        Some(v) if !(0..=max).contains(&v) => Err(validation(message)),
        _ => Ok(()),
    }
}

/// ตรวจวันที่ → resolve master เป็น id ภายใน → `GroupWrite`
/// `current` เป็น Some ตอนแก้ไข: ปีงบ/กลุ่มที่ถูกปิดใช้งานภายหลัง "คงไว้" ได้ แต่เลือกใหม่ไม่ได้
async fn prepare_group(
    pool: &PgPool,
    input: &GroupInput,
    current: Option<&GroupRow>,
) -> Result<GroupWrite, LabVisitError> {
    if input.cd4_second_date <= input.cd4_first_date {
        return Err(validation("cd4 second date must be after first date"));
    }

    let budget_year = repository::find_budget_year(pool, input.budget_year_id)
        .await?
        .ok_or_else(|| validation("unknown budget year"))?;
    let keeps_budget_year =
        current.is_some_and(|g| g.budget_year_public_id == input.budget_year_id);
    if !budget_year.is_active && !keeps_budget_year {
        return Err(validation("budget year is inactive"));
    }

    let patient_group = repository::find_patient_group(pool, input.patient_group_id)
        .await?
        .ok_or_else(|| validation("unknown patient group"))?;
    let keeps_patient_group =
        current.is_some_and(|g| g.patient_group_public_id == input.patient_group_id);
    if !patient_group.is_active && !keeps_patient_group {
        return Err(validation("patient group is inactive"));
    }

    Ok(GroupWrite {
        budget_year_id: budget_year.id,
        patient_group_id: patient_group.id,
        cd4_first_date: input.cd4_first_date,
        cd4_second_date: input.cd4_second_date,
        vl_date: input.vl_date,
        sort_order: input.sort_order,
    })
}

async fn existing_group(pool: &PgPool, public_id: Uuid) -> Result<GroupRow, LabVisitError> {
    repository::find_group(pool, public_id)
        .await?
        .ok_or(LabVisitError::GroupNotFound)
}

pub async fn list_groups(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<GroupRow>, LabVisitError> {
    require(actor, Action::View)?;
    Ok(repository::list_groups(pool).await?)
}

pub async fn options(pool: &PgPool, actor: &CurrentUser) -> Result<MasterOptions, LabVisitError> {
    require(actor, Action::View)?;
    Ok(repository::list_master_options(pool).await?)
}

pub async fn get_group(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
) -> Result<GroupDetail, LabVisitError> {
    require(actor, Action::View)?;
    let group = existing_group(pool, public_id).await?;
    let visits = repository::list_visits(pool, group.id).await?;
    Ok(GroupDetail { group, visits })
}

/// สร้างนัดหมาย — คืน uuid ของนัดหมายใหม่ (หน้าเว็บพาไปหน้ารายชื่อของนัดหมายนั้นต่อ)
pub async fn create_group(
    pool: &PgPool,
    actor: &CurrentUser,
    input: GroupInput,
) -> Result<Uuid, LabVisitError> {
    require(actor, Action::Create)?;
    let write = prepare_group(pool, &input, None).await?;
    let public_id = repository::insert_group(pool, actor.user.id, &write)
        .await
        .map_err(conflict_on_unique("lab visit group already exists"))?;
    Ok(public_id)
}

pub async fn update_group(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: GroupInput,
) -> Result<(), LabVisitError> {
    require(actor, Action::Update)?;
    let current = existing_group(pool, public_id).await?;
    let write = prepare_group(pool, &input, Some(&current)).await?;
    repository::update_group(pool, actor.user.id, current.id, public_id, &write)
        .await
        .map_err(conflict_on_unique("lab visit group already exists"))?;
    Ok(())
}

/// ลบนัดหมายพร้อมรายชื่อคนไข้ทั้งหมดในนั้น (soft delete)
pub async fn delete_group(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
) -> Result<(), LabVisitError> {
    require(actor, Action::Delete)?;
    let current = existing_group(pool, public_id).await?;
    repository::soft_delete_group(pool, actor.user.id, current.id, public_id).await?;
    Ok(())
}

/// เอกสารที่ส่งออกได้ (ตรงกับ `LAB_DOCUMENTS` ฝั่งหน้าเว็บ) และรูปแบบไฟล์ — ค่าที่ยอมรับใน `record_export`
const EXPORT_DOCUMENTS: &[&str] = &["cd4", "roster"];
const EXPORT_FORMATS: &[&str] = &["excel", "pdf"];

/// บันทึกการส่งออกเอกสาร (หน้า "ประวัติการใช้งาน") — เอกสาร Excel/PDF สร้างฝั่งหน้าเว็บ หน้าเว็บเรียกที่นี่ก่อน
/// สร้างไฟล์/แสดงหน้าพิมพ์ทุกครั้ง ต้องมี `lab_visit.export` และนัดหมายต้องยังไม่ถูกลบ
pub async fn record_export(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    document: &str,
    format: &str,
) -> Result<(), LabVisitError> {
    require(actor, Action::Export)?;
    if !EXPORT_DOCUMENTS.contains(&document) {
        return Err(validation("document must be cd4 or roster"));
    }
    if !EXPORT_FORMATS.contains(&format) {
        return Err(validation("format must be excel or pdf"));
    }
    existing_group(pool, public_id).await?;
    repository::insert_export_audit(pool, actor.user.id, public_id, document, format).await?;
    Ok(())
}

/// เพิ่มคนไข้หลายคนเข้านัดหมาย ตามลำดับที่ส่งมา (uuid ซ้ำในคำขอตัดทิ้ง) — คนไข้ต้องมีอยู่จริง ใช้งานอยู่
/// และยังไม่อยู่ในนัดหมายนี้ ไม่งั้นไม่เพิ่มสักคน
///
/// ต้องมี `patient.view` ด้วย: คำตอบ ‘unknown patient’ / ‘patient is inactive’ / ‘already in group’ ขึ้นกับสถานะในทะเบียนผู้ป่วย
/// (โดยเฉพาะ `is_active` ซึ่งอ่านได้ผ่าน `GET /patients` ที่ต้องมี `patient.view` เท่านั้น) — กฎเดียวกับ create/replace ผู้ป่วย
/// (patient/service.rs): ผู้เขียนที่ดูไม่ได้ต้องไม่รู้สถานะที่ตัวเองอ่านไม่ได้ (security audit รอบ 2) หน้าเว็บของนัดหมายต้องมี
/// `patient.view` อยู่แล้ว (join ชื่อ/HN) จึงไม่กระทบการใช้งานปกติ
pub async fn add_visits(
    pool: &PgPool,
    actor: &CurrentUser,
    group_public_id: Uuid,
    patient_ids: &[Uuid],
) -> Result<(), LabVisitError> {
    require(actor, Action::Update)?;
    if !actor.can(Resource::Patient, Action::View) {
        return Err(LabVisitError::Forbidden);
    }

    // เพดานก่อนวนลูปใด ๆ ที่โตตามความยาวอาร์เรย์ (คำขอ 2 MiB ใส่ได้หลายหมื่น uuid) แล้วตัดซ้ำแบบเส้นตรงด้วย HashSet
    if patient_ids.len() > MAX_PATIENTS_PER_REQUEST {
        return Err(validation("too many patients"));
    }
    let mut seen = HashSet::with_capacity(patient_ids.len());
    let unique_ids: Vec<Uuid> = patient_ids
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect();
    if unique_ids.is_empty() {
        return Err(validation("patient ids are required"));
    }

    let group = existing_group(pool, group_public_id).await?;

    let found: HashMap<Uuid, repository::PatientLookup> =
        repository::find_patients(pool, &unique_ids)
            .await?
            .into_iter()
            .map(|p| (p.public_id, p))
            .collect();
    let already_in_group = repository::group_patient_ids(pool, group.id).await?;

    let mut ids: Vec<PatientId> = Vec::with_capacity(unique_ids.len());
    for public_id in &unique_ids {
        let patient = found
            .get(public_id)
            .ok_or_else(|| validation("unknown patient"))?;
        if !patient.is_active {
            return Err(validation("patient is inactive"));
        }
        if already_in_group.contains(&patient.id) {
            return Err(LabVisitError::Conflict(
                "patient already in group".to_string(),
            ));
        }
        ids.push(patient.id);
    }

    repository::insert_visits(pool, actor.user.id, group.id, &ids)
        .await
        .map_err(conflict_on_unique("patient already in group"))?;
    Ok(())
}

/// แทนที่ข้อมูลของคนไข้หนึ่งคนในนัดหมาย (ระยะเวลากินยา, ผล VL, สูตรยา, ลำดับ)
pub async fn update_visit(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: VisitInput<'_>,
) -> Result<(), LabVisitError> {
    require(actor, Action::Update)?;
    check_range(
        input.take_med_year_range,
        MAX_TAKE_MED_YEARS,
        "take med years is out of range",
    )?;
    check_range(
        input.take_med_month_range,
        MAX_TAKE_MED_MONTHS,
        "take med months is out of range",
    )?;
    let write = VisitWrite {
        take_med_year_range: input.take_med_year_range,
        take_med_month_range: input.take_med_month_range,
        vl_result: optional_text(input.vl_result, "vl result", MAX_VL_RESULT_CHARS)?,
        drug_recipe: optional_text(input.drug_recipe, "drug recipe", MAX_DRUG_RECIPE_CHARS)?,
        sort_order: input.sort_order,
    };

    let id = repository::find_visit_id(pool, public_id)
        .await?
        .ok_or(LabVisitError::VisitNotFound)?;
    repository::update_visit(pool, actor.user.id, id, public_id, &write).await?;
    Ok(())
}

/// เอาคนไข้ออกจากนัดหมาย (soft delete แถว lab_visit — เพิ่มกลับได้)
pub async fn delete_visit(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
) -> Result<(), LabVisitError> {
    require(actor, Action::Update)?;
    let id = repository::find_visit_id(pool, public_id)
        .await?
        .ok_or(LabVisitError::VisitNotFound)?;
    repository::soft_delete_visit(pool, actor.user.id, id, public_id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_text_trims_blanks_to_none_and_counts_characters() {
        assert_eq!(optional_text(None, "x", 5).unwrap(), None);
        assert_eq!(optional_text(Some("   "), "x", 5).unwrap(), None);
        assert_eq!(
            optional_text(Some(" <20 "), "x", 5).unwrap().as_deref(),
            Some("<20")
        );
        // 5 ตัวอักษรไทย = 15 byte แต่ยังผ่าน
        assert!(optional_text(Some("กขคงจ"), "x", 5).is_ok());
        assert!(optional_text(Some("กขคงจฉ"), "x", 5).is_err());
    }

    #[test]
    fn ranges_are_inclusive_and_none_passes() {
        assert!(check_range(None, 11, "m").is_ok());
        assert!(check_range(Some(0), 11, "m").is_ok());
        assert!(check_range(Some(11), 11, "m").is_ok());
        assert!(check_range(Some(12), 11, "m").is_err());
        assert!(check_range(Some(-1), 11, "m").is_err());
    }

    #[test]
    fn debug_of_visit_types_never_prints_health_values() {
        let input = VisitInput {
            take_med_year_range: Some(3),
            take_med_month_range: Some(4),
            vl_result: Some("VL-SECRET"),
            drug_recipe: Some("RECIPE-SECRET"),
            sort_order: None,
        };
        let row = VisitRow {
            id: crate::domain::LabVisitId(1),
            public_id: Uuid::nil(),
            patient_public_id: Uuid::nil(),
            take_med_year_range: None,
            take_med_month_range: None,
            vl_result: Some("VL-SECRET".to_string()),
            drug_recipe: Some("RECIPE-SECRET".to_string()),
            sort_order: 1,
        };
        for printed in [format!("{input:?}"), format!("{row:?}")] {
            assert!(!printed.contains("VL-SECRET"), "{printed}");
            assert!(!printed.contains("RECIPE-SECRET"), "{printed}");
        }
    }
}
