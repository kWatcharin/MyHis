//! กฎของ patient: สิทธิ์ (`patient.view/create/update/delete`), การเข้า/ถอดรหัสคอลัมน์ระบุตัวคนไข้, ค่าที่ต้อง
//! ถูกต้อง, HN ซ้ำ, การผูกโรคประจำตัว ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 10)
//! เวลา (วันนี้) เป็นพารามิเตอร์เสมอ ไม่เรียก Utc::now() ในนี้ตรง ๆ (docs/rust-guidelines.md ข้อ 14)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็คสิทธิ์ที่นี่ — อ่าน
//! ต้อง `patient.view` สร้าง/แก้/ลบ ต้อง `patient.create` / `patient.update` / `patient.delete` ตรวจแยกกัน
//!
//! **ข้อมูลระบุตัวคนไข้ (HN, เลขประจำตัวคนไข้, ชื่อ, สกุล)** เข้ารหัสด้วย `Crypto` ก่อนส่งลง repository เสมอ
//! และถอดตอนอ่าน — ชนิดที่ถือค่าจริงในไฟล์นี้เขียน `Debug` เองให้ redact และข้อความ error ไม่แนบค่า

use std::collections::HashMap;

use chrono::{Datelike, NaiveDate};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::CurrentUser;
use crate::crypto::{Crypto, CryptoError};
use crate::domain::{Action, PatientId, Resource};
use crate::patient::repository::{
    self, ConditionRow, MasterOptions, PatientRow, PatientWrite, RepositoryError,
};

// AAD ของแต่ละคอลัมน์ที่เข้ารหัส — ผูก ciphertext กับคอลัมน์ ถ้าถูกสลับข้ามคอลัมน์ถอดรหัสจะล้มเหลว
// (blind index ของ hn/nap_no ใช้ AAD ตัวเดียวกับคอลัมน์ของมัน)
const AAD_HN: &str = "patient.hn";
const AAD_NAP_NO: &str = "patient.nap_no";
const AAD_FIRST_NAME: &str = "patient.first_name";
const AAD_LAST_NAME: &str = "patient.last_name";

const MAX_ID_CHARS: usize = 50; // hn, nap_no
const MAX_NAME_CHARS: usize = 100; // first_name, last_name — นับเป็นตัวอักษร ไม่ใช่ byte (ชื่อไทย)
const MAX_AGE: i32 = 150;
const MAX_WEIGHT_KG: f64 = 500.0;
const MAX_HEIGHT_CM: f64 = 300.0;

#[derive(Debug, thiserror::Error)]
pub enum PatientError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("patient not found")]
    NotFound,
    /// เข้า/ถอดรหัสล้มเหลว (key ผิด, ข้อมูลเสีย) — ไม่แนบรายละเอียดหรือค่าใด ๆ
    #[error("patient data could not be processed")]
    Crypto(#[from] CryptoError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// ตรวจสิทธิ์ของ action นี้ (`patient.*`) — ตรวจแยกกันทุกตัว แก้ไขไม่ implies ดู
fn require(actor: &CurrentUser, action: Action) -> Result<(), PatientError> {
    if actor.can(Resource::Patient, action) {
        Ok(())
    } else {
        Err(PatientError::Forbidden)
    }
}

fn map_unique_violation(err: RepositoryError) -> PatientError {
    if err.is_unique_violation() {
        PatientError::Conflict("hn already registered".to_string())
    } else {
        PatientError::from(err)
    }
}

fn validation(message: &str) -> PatientError {
    PatientError::Validation(message.to_string())
}

/// ผู้ป่วยที่ถอดรหัสแล้ว พร้อมโรคประจำตัว — ไม่ derive Debug (พิมพ์ [redacted] เท่านั้น)
pub struct PatientView {
    pub public_id: Uuid,
    pub hn: String,
    pub nap_no: String,
    pub prename_public_id: Uuid,
    pub prename_name: String,
    pub first_name: String,
    pub last_name: String,
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
    pub conditions: Vec<ConditionRow>,
}

impl std::fmt::Debug for PatientView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatientView")
            .field("fields", &"[redacted]")
            .finish()
    }
}

/// ข้อมูลที่ผู้ใช้กรอกตอนลงทะเบียน/แก้ไข (uuid ของ master ยังไม่ resolve) — Debug redact
pub struct PatientInput<'a> {
    pub hn: &'a str,
    pub nap_no: &'a str,
    pub prename_id: Uuid,
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub birth_date: Option<NaiveDate>,
    pub age: Option<i32>,
    pub weight: Option<f64>,
    pub height: Option<f64>,
    pub sex: i32,
    pub is_thai: bool,
    pub nationality_id: Option<Uuid>,
    pub sort_order: Option<i32>,
    pub is_active: bool,
    pub disease_ids: &'a [Uuid],
}

impl std::fmt::Debug for PatientInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatientInput")
            .field("fields", &"[redacted]")
            .finish()
    }
}

/// อายุเต็มปี ณ วัน `today` (ยังไม่ถึงวันเกิดปีนี้ = ยังไม่บวกอีกปี)
fn age_on(birth_date: NaiveDate, today: NaiveDate) -> i32 {
    let mut years = today.year() - birth_date.year();
    if (today.month(), today.day()) < (birth_date.month(), birth_date.day()) {
        years -= 1;
    }
    years
}

/// trim แล้วต้องไม่ว่างและไม่เกิน `max` ตัวอักษร — ข้อความ error ไม่แนบค่า
fn clean_text(value: &str, field: &str, max: usize) -> Result<String, PatientError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(PatientError::Validation(format!("{field} is required")));
    }
    if trimmed.chars().count() > max {
        return Err(PatientError::Validation(format!(
            "{field} must be at most {max} characters"
        )));
    }
    Ok(trimmed.to_string())
}

fn check_measure(value: Option<f64>, max: f64, field: &str) -> Result<(), PatientError> {
    match value {
        Some(v) if !v.is_finite() || v <= 0.0 || v > max => {
            Err(PatientError::Validation(format!("{field} is out of range")))
        }
        _ => Ok(()),
    }
}

/// ตรวจค่า → resolve uuid ของ master เป็น id ภายใน → เข้ารหัส/คำนวณ blind index → `PatientWrite`
/// `patient_id` เป็น Some ตอนแก้ไข (ใช้ตัดสินว่าโรค inactive ตัวไหนผูกอยู่แล้วจึงคงไว้ได้)
async fn prepare(
    pool: &PgPool,
    crypto: &Crypto,
    input: &PatientInput<'_>,
    today: NaiveDate,
    patient_id: Option<PatientId>,
) -> Result<PatientWrite, PatientError> {
    let hn = clean_text(input.hn, "hn", MAX_ID_CHARS)?;
    let nap_no = clean_text(input.nap_no, "nap no", MAX_ID_CHARS)?;
    let first_name = clean_text(input.first_name, "first name", MAX_NAME_CHARS)?;
    let last_name = clean_text(input.last_name, "last name", MAX_NAME_CHARS)?;

    if input.sex != 1 && input.sex != 2 {
        return Err(validation("sex must be 1 or 2"));
    }
    check_measure(input.weight, MAX_WEIGHT_KG, "weight")?;
    check_measure(input.height, MAX_HEIGHT_CM, "height")?;

    // มีวันเกิด → อายุคำนวณจากวันเกิดเสมอ (ไม่เชื่อ age ที่ส่งมา); ไม่มี → ใช้ age ที่กรอก
    let age = match input.birth_date {
        Some(birth_date) => {
            if birth_date > today {
                return Err(validation("birth date must not be in the future"));
            }
            let age = age_on(birth_date, today);
            if age > MAX_AGE {
                return Err(validation("birth date is too far in the past"));
            }
            Some(age)
        }
        None => match input.age {
            Some(age) if !(0..=MAX_AGE).contains(&age) => {
                return Err(validation("age is out of range"));
            }
            other => other,
        },
    };

    let prename_id = repository::find_prename_id(pool, input.prename_id)
        .await?
        .ok_or_else(|| validation("unknown prename"))?;
    let nationality_id = match input.nationality_id {
        Some(public_id) => Some(
            repository::find_nationality_id(pool, public_id)
                .await?
                .ok_or_else(|| validation("unknown nationality"))?,
        ),
        None => None,
    };

    // โรคประจำตัว: ตัดซ้ำโดยรักษาลำดับ, ต้องมีอยู่จริง, โรค inactive ผูกใหม่ไม่ได้ (ที่ผูกอยู่แล้วคงไว้ได้)
    let mut disease_public_ids: Vec<Uuid> = Vec::with_capacity(input.disease_ids.len());
    for id in input.disease_ids {
        if !disease_public_ids.contains(id) {
            disease_public_ids.push(*id);
        }
    }
    let found = repository::find_diseases(pool, &disease_public_ids).await?;
    let already_linked = match patient_id {
        Some(id) => repository::linked_disease_public_ids(pool, id).await?,
        None => Vec::new(),
    };
    let mut disease_ids: Vec<i32> = Vec::with_capacity(disease_public_ids.len());
    for public_id in &disease_public_ids {
        let disease = found
            .iter()
            .find(|d| d.public_id == *public_id)
            .ok_or_else(|| validation("unknown disease"))?;
        if !disease.is_active && !already_linked.contains(public_id) {
            return Err(validation("disease is inactive"));
        }
        disease_ids.push(disease.id);
    }

    Ok(PatientWrite {
        hn: crypto.encrypt(AAD_HN, &hn)?,
        hn_idx: crypto.blind_index(AAD_HN, &hn),
        nap_no: crypto.encrypt(AAD_NAP_NO, &nap_no)?,
        nap_no_idx: crypto.blind_index(AAD_NAP_NO, &nap_no),
        prename_id,
        first_name: crypto.encrypt(AAD_FIRST_NAME, &first_name)?,
        last_name: crypto.encrypt(AAD_LAST_NAME, &last_name)?,
        birth_date: input.birth_date,
        age,
        weight: input.weight,
        height: input.height,
        sex: input.sex,
        is_thai: input.is_thai,
        nationality_id,
        sort_order: input.sort_order,
        is_active: input.is_active,
        disease_ids,
    })
}

fn to_view(
    crypto: &Crypto,
    row: PatientRow,
    conditions: Vec<ConditionRow>,
    today: NaiveDate,
) -> Result<PatientView, PatientError> {
    Ok(PatientView {
        public_id: row.public_id,
        hn: crypto.decrypt(AAD_HN, &row.hn)?,
        nap_no: crypto.decrypt(AAD_NAP_NO, &row.nap_no)?,
        prename_public_id: row.prename_public_id,
        prename_name: row.prename_name,
        first_name: crypto.decrypt(AAD_FIRST_NAME, &row.first_name)?,
        last_name: crypto.decrypt(AAD_LAST_NAME, &row.last_name)?,
        birth_date: row.birth_date,
        // มีวันเกิด → คำนวณใหม่ทุกครั้งที่อ่าน กันอายุที่เก็บไว้ค้างเก่า; ไม่มี → ค่าที่กรอกไว้
        age: row.birth_date.map(|b| age_on(b, today)).or(row.age),
        weight: row.weight,
        height: row.height,
        sex: row.sex,
        is_thai: row.is_thai,
        nationality_public_id: row.nationality_public_id,
        nationality_name: row.nationality_name,
        sort_order: row.sort_order,
        is_active: row.is_active,
        conditions,
    })
}

/// ผู้ป่วยทั้งหมดที่ยังไม่ถูกลบ (ถอดรหัสแล้ว) พร้อมโรคประจำตัว เรียง sort_order แล้ว id —
/// ไม่มีการค้นหาฝั่ง server (ชื่อเข้ารหัส ค้นใน SQL ไม่ได้) ฝั่ง client กรองเอง
pub async fn list(
    pool: &PgPool,
    crypto: &Crypto,
    actor: &CurrentUser,
    today: NaiveDate,
) -> Result<Vec<PatientView>, PatientError> {
    require(actor, Action::View)?;

    let rows = repository::list_patients(pool).await?;
    let mut conditions_by_patient: HashMap<PatientId, Vec<ConditionRow>> = HashMap::new();
    for condition in repository::list_conditions(pool).await? {
        conditions_by_patient
            .entry(condition.patient_id)
            .or_default()
            .push(condition);
    }

    rows.into_iter()
        .map(|row| {
            let conditions = conditions_by_patient.remove(&row.id).unwrap_or_default();
            to_view(crypto, row, conditions, today)
        })
        .collect()
}

pub async fn options(pool: &PgPool, actor: &CurrentUser) -> Result<MasterOptions, PatientError> {
    require(actor, Action::View)?;
    Ok(repository::list_master_options(pool).await?)
}

pub async fn create(
    pool: &PgPool,
    crypto: &Crypto,
    actor: &CurrentUser,
    input: PatientInput<'_>,
    today: NaiveDate,
) -> Result<(), PatientError> {
    require(actor, Action::Create)?;
    let write = prepare(pool, crypto, &input, today, None).await?;
    repository::insert_patient(pool, actor.user.id, &write)
        .await
        .map_err(map_unique_violation)?;
    Ok(())
}

/// แทนที่ข้อมูลผู้ป่วยทั้งชุด (PUT) — ฟิลด์ nullable ที่ส่ง null = ล้างค่า; `sort_order` ไม่ส่ง = คงค่าเดิม
pub async fn replace(
    pool: &PgPool,
    crypto: &Crypto,
    actor: &CurrentUser,
    public_id: Uuid,
    input: PatientInput<'_>,
    today: NaiveDate,
) -> Result<(), PatientError> {
    require(actor, Action::Update)?;
    let id = repository::find_patient_id(pool, public_id)
        .await?
        .ok_or(PatientError::NotFound)?;
    let write = prepare(pool, crypto, &input, today, Some(id)).await?;

    // ชื่อ-สกุลเข้ารหัสแบบสุ่ม nonce เทียบใน DB ไม่ได้ — ถอดรหัสค่าเดิมของแถวนี้แถวเดียวมาเทียบในหน่วยความจำ
    // ได้แค่ชื่อช่องที่เปลี่ยนไปบันทึกประวัติ (ไม่มีค่าใด ๆ ออกนอกฟังก์ชันนี้)
    let (current_first, current_last) = repository::find_patient_names(pool, id)
        .await?
        .ok_or(PatientError::NotFound)?;
    let mut changed_names: Vec<&str> = Vec::new();
    if crypto.decrypt(AAD_FIRST_NAME, &current_first)?
        != clean_text(input.first_name, "first name", MAX_NAME_CHARS)?
    {
        changed_names.push("first_name");
    }
    if crypto.decrypt(AAD_LAST_NAME, &current_last)?
        != clean_text(input.last_name, "last name", MAX_NAME_CHARS)?
    {
        changed_names.push("last_name");
    }

    repository::replace_patient(pool, actor.user.id, id, public_id, &write, &changed_names)
        .await
        .map_err(map_unique_violation)?;
    Ok(())
}

pub async fn delete(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
) -> Result<(), PatientError> {
    require(actor, Action::Delete)?;
    let id = repository::find_patient_id(pool, public_id)
        .await?
        .ok_or(PatientError::NotFound)?;
    repository::soft_delete_patient(pool, actor.user.id, id, public_id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn age_counts_full_years_only() {
        let birth = date(1994, 9, 19);
        assert_eq!(age_on(birth, date(2026, 9, 18)), 31); // วันก่อนวันเกิด
        assert_eq!(age_on(birth, date(2026, 9, 19)), 32); // วันเกิดพอดี
        assert_eq!(age_on(birth, date(2026, 12, 31)), 32);
        assert_eq!(age_on(birth, birth), 0);
    }

    #[test]
    fn age_handles_leap_day_birthday() {
        let birth = date(2000, 2, 29);
        assert_eq!(age_on(birth, date(2027, 2, 28)), 26);
        assert_eq!(age_on(birth, date(2027, 3, 1)), 27);
    }

    #[test]
    fn clean_text_trims_and_counts_characters() {
        assert_eq!(clean_text("  สมชาย ", "x", 10).unwrap(), "สมชาย");
        assert!(clean_text("   ", "x", 10).is_err());
        assert!(clean_text(&"ก".repeat(11), "x", 10).is_err());
        assert!(clean_text(&"ก".repeat(10), "x", 10).is_ok()); // 30 byte แต่ 10 ตัวอักษร
    }

    #[test]
    fn measures_must_be_positive_finite_and_bounded() {
        assert!(check_measure(None, 500.0, "weight").is_ok());
        assert!(check_measure(Some(67.01), 500.0, "weight").is_ok());
        assert!(check_measure(Some(500.0), 500.0, "weight").is_ok());
        for bad in [0.0, -1.0, 500.01, f64::NAN, f64::INFINITY] {
            assert!(check_measure(Some(bad), 500.0, "weight").is_err(), "{bad}");
        }
    }

    #[test]
    fn debug_of_patient_types_never_prints_values() {
        let view = PatientView {
            public_id: Uuid::nil(),
            hn: "12345-HN".to_string(),
            nap_no: "NAP-SECRET".to_string(),
            prename_public_id: Uuid::nil(),
            prename_name: "นาย".to_string(),
            first_name: "ชื่อลับ".to_string(),
            last_name: "สกุลลับ".to_string(),
            birth_date: None,
            age: None,
            weight: None,
            height: None,
            sex: 1,
            is_thai: true,
            nationality_public_id: None,
            nationality_name: None,
            sort_order: 1,
            is_active: true,
            conditions: Vec::new(),
        };
        let printed = format!("{view:?}");
        for secret in ["12345-HN", "NAP-SECRET", "ชื่อลับ", "สกุลลับ"] {
            assert!(!printed.contains(secret), "{secret} leaked");
        }
    }
}
