//! รูปแบบ JSON เข้า/ออกของโมดูล patient — แยกจาก struct ของตาราง (docs/rust-guidelines.md ข้อ 4)
//! `id` ที่ออก API คือ uuid เสมอ (id int ภายในไม่ออกนอก backend)
//!
//! **ชนิดที่ถือข้อมูลระบุตัวคนไข้ (`PatientRequest`, `PatientResponse`) เขียน `Debug` เองให้พิมพ์ [redacted]**
//! เหมือนชนิดที่ถือรหัสผ่าน (docs/rust-guidelines.md ข้อ 9) — กันหลุดลง log ถ้ามีใครเผลอ `{:?}`
//! การตรวจค่าจริงทำที่ service ที่เดียว เพราะเทสระดับ service เรียกตรงโดยไม่ผ่าน DTO

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// ใช้ทั้งสร้าง (POST) และแทนที่ทั้งชุด (PUT) — ฟอร์มส่งครบทุกฟิลด์ และฟิลด์ nullable ต้องล้างเป็น null ได้
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PatientRequest {
    pub hn: String,
    pub nap_no: String,
    pub prename_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub birth_date: Option<NaiveDate>,
    pub age: Option<i32>,
    pub weight: Option<f64>,
    pub height: Option<f64>,
    pub sex: i32,
    pub is_thai: bool,
    pub nationality_id: Option<Uuid>,
    /// สร้าง: ไม่ส่ง = ต่อท้ายลำดับสุดท้าย, แก้ไข: ไม่ส่ง = คงลำดับเดิม
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
    /// โรคประจำตัว (uuid ของ disease) ตามลำดับที่ต้องการ — ชุดนี้แทนที่โรคที่ผูกอยู่เดิมทั้งหมด
    #[serde(default)]
    pub disease_ids: Vec<Uuid>,
}

impl std::fmt::Debug for PatientRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatientRequest")
            .field("fields", &"[redacted]")
            .finish()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionResponse {
    /// uuid ของแถว medical_condition
    pub id: Uuid,
    pub disease_id: Uuid,
    pub name: String,
    pub abbreviation: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientResponse {
    pub id: Uuid,
    pub hn: String,
    pub nap_no: String,
    pub prename_id: Uuid,
    pub prename_name: String,
    pub first_name: String,
    pub last_name: String,
    pub birth_date: Option<NaiveDate>,
    pub age: Option<i32>,
    pub weight: Option<f64>,
    pub height: Option<f64>,
    pub sex: i32,
    pub is_thai: bool,
    pub nationality_id: Option<Uuid>,
    pub nationality_name: Option<String>,
    pub sort_order: i32,
    pub is_active: bool,
    pub conditions: Vec<ConditionResponse>,
}

impl std::fmt::Debug for PatientResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatientResponse")
            .field("fields", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrenameOption {
    pub id: Uuid,
    pub name: String,
    pub abbreviation: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NationalityOption {
    pub id: Uuid,
    pub national_name: String,
    pub nation: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiseaseOption {
    pub id: Uuid,
    pub name: String,
    pub abbreviation: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsResponse {
    pub prenames: Vec<PrenameOption>,
    pub nationalities: Vec<NationalityOption>,
    pub diseases: Vec<DiseaseOption>,
}
