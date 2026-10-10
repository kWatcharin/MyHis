//! รูปแบบ JSON เข้า/ออกของโมดูลนัดหมายแล็ป — แยกจาก struct ของตาราง (docs/rust-guidelines.md ข้อ 4)
//! `id` ที่ออก API คือ uuid เสมอ (id int ภายในไม่ออกนอก backend) และไม่มีข้อมูลระบุตัวคนไข้ในนี้เลย —
//! มีแค่ `patientId` (uuid) ให้หน้าเว็บ join กับ `GET /patients` เอง
//!
//! ชนิดที่ถือผล VL/สูตรยา (ข้อมูลสุขภาพ) เขียน `Debug` เองให้ redact กันหลุดลง log
//! การตรวจค่าจริงทำที่ service ที่เดียว เพราะเทสระดับ service เรียกตรงโดยไม่ผ่าน DTO

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// ใช้ทั้งสร้าง (POST) และแทนที่ (PUT) หัวนัดหมาย
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GroupRequest {
    pub budget_year_id: Uuid,
    pub patient_group_id: Uuid,
    pub cd4_first_date: NaiveDate,
    pub cd4_second_date: NaiveDate,
    pub vl_date: NaiveDate,
    /// สร้าง: ไม่ส่ง = ต่อท้ายลำดับสุดท้าย, แก้ไข: ไม่ส่ง = คงลำดับเดิม
    pub sort_order: Option<i32>,
}

/// ผลของ POST /lab-visit-groups — uuid ของนัดหมายใหม่
#[derive(Debug, Serialize)]
pub struct CreatedResponse {
    pub id: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupResponse {
    pub id: Uuid,
    pub budget_year_id: Uuid,
    /// ปีงบประมาณ (พ.ศ.) เช่น "2569"
    pub budget_year: String,
    pub patient_group_id: Uuid,
    pub patient_group_name: String,
    pub cd4_first_date: NaiveDate,
    pub cd4_second_date: NaiveDate,
    pub vl_date: NaiveDate,
    pub sort_order: i32,
    /// จำนวนคนไข้ในนัดหมาย (ไม่นับคนไข้ที่ถูกลบจากทะเบียน)
    pub patient_count: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisitResponse {
    /// uuid ของแถว lab_visit
    pub id: Uuid,
    pub patient_id: Uuid,
    pub take_med_year_range: Option<i32>,
    pub take_med_month_range: Option<i32>,
    pub vl_result: Option<String>,
    pub drug_recipe: Option<String>,
    pub sort_order: i32,
}

impl std::fmt::Debug for VisitResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VisitResponse")
            .field("id", &self.id)
            .field("fields", &"[redacted]")
            .finish()
    }
}

/// GET /lab-visit-groups/{id} — ฟิลด์ของหัวนัดหมาย (แบนอยู่ระดับเดียวกัน) + รายชื่อคนไข้
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDetailResponse {
    #[serde(flatten)]
    pub group: GroupResponse,
    pub visits: Vec<VisitResponse>,
}

/// POST /lab-visit-groups/{id}/visits — uuid ของคนไข้ตามลำดับที่ต้องการต่อท้าย
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AddVisitsRequest {
    pub patient_ids: Vec<Uuid>,
}

/// PUT /lab-visits/{id} — แทนที่ทั้งชุด (ฟิลด์ที่ไม่ส่ง/null = ล้างค่า ยกเว้น sortOrder = คงเดิม)
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct VisitRequest {
    pub take_med_year_range: Option<i32>,
    pub take_med_month_range: Option<i32>,
    pub vl_result: Option<String>,
    pub drug_recipe: Option<String>,
    pub sort_order: Option<i32>,
}

impl std::fmt::Debug for VisitRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VisitRequest")
            .field("fields", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetYearOption {
    pub id: Uuid,
    pub year: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientGroupOption {
    pub id: Uuid,
    pub name: String,
    pub is_active: bool,
}

/// ตัวเลือกของฟอร์มนัดหมาย — ต้องมีที่นี่เพราะ `/budget-years`, `/patient-groups` ต้องมี `user.manage`
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsResponse {
    pub budget_years: Vec<BudgetYearOption>,
    pub patient_groups: Vec<PatientGroupOption>,
}
