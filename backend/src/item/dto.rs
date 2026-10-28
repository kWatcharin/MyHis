//! รูปแบบ JSON เข้า/ออกของโมดูล item — แยกจาก struct ของตาราง (docs/rust-guidelines.md ข้อ 4)
//! `id` ที่ออก API คือ uuid เสมอ (id int ภายในไม่ออกนอก backend) การตรวจค่าทำที่ service ที่เดียว
//! เพราะเทสระดับ service เรียกตรงโดยไม่ผ่าน DTO

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConversionRequest {
    /// uuid ของ item_unit
    pub unit_id: Uuid,
    /// จำนวนหน่วยฐานต่อ 1 หน่วยนี้
    pub factor: i32,
}

/// ใช้ทั้งสร้าง (POST) และแทนที่ทั้งชุด (PUT) — ฟอร์มส่งครบทุกฟิลด์ และฟิลด์ nullable ต้องล้างเป็น null ได้
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ItemRequest {
    pub name: String,
    pub category_id: Uuid,
    pub base_unit_id: Uuid,
    /// สร้าง: ไม่ส่ง = ค่าตั้งต้นของหมวด, แก้ไข: ไม่ส่ง = คงค่าเดิม (สามธงเหมือนกัน)
    pub track_lot: Option<bool>,
    pub track_expiry: Option<bool>,
    pub patient_linkable: Option<bool>,
    /// จุดสั่งซื้อ (หน่วยฐาน) — ไม่ส่ง/null = ไม่ตั้ง
    pub reorder_point: Option<i32>,
    pub is_active: Option<bool>,
    /// หน่วยแปลงทั้งชุด — ชุดนี้แทนที่หน่วยแปลงเดิมทั้งหมด (ไม่ส่ง = ไม่มีหน่วยแปลง)
    #[serde(default)]
    pub conversions: Vec<ConversionRequest>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversionResponse {
    pub unit_id: Uuid,
    pub unit_name: String,
    pub factor: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResponse {
    pub id: Uuid,
    pub name: String,
    pub category_id: Uuid,
    pub category_name: String,
    pub base_unit_id: Uuid,
    pub base_unit_name: String,
    pub track_lot: bool,
    pub track_expiry: bool,
    pub patient_linkable: bool,
    pub reorder_point: Option<i32>,
    pub is_active: bool,
    /// เรียงตาม factor น้อยไปมาก
    pub conversions: Vec<ConversionResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryOption {
    pub id: Uuid,
    pub name: String,
    pub default_track_lot: bool,
    pub default_track_expiry: bool,
    pub default_patient_linkable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitOption {
    pub id: Uuid,
    pub name: String,
}

/// ตัวเลือกของฟอร์มรายการ — เฉพาะหมวด/หน่วยที่ใช้งานอยู่ (ค่าที่ผูกอยู่แล้วมากับแถวของรายการเอง)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsResponse {
    pub categories: Vec<CategoryOption>,
    pub units: Vec<UnitOption>,
}
