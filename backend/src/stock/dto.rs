//! รูปแบบ JSON เข้า/ออกของโมดูล stock — แยกจาก struct ของตาราง (docs/rust-guidelines.md ข้อ 4)
//! `id` ที่ออก API คือ uuid เสมอ (id int ภายในไม่ออกนอก backend) การตรวจค่าทำที่ service ที่เดียว
//! เพราะเทสระดับ service เรียกตรงโดยไม่ผ่าน DTO — จำนวนทุกช่องเป็นหน่วยฐาน ยกเว้น `quantity` ของบรรทัด (หน่วยที่กรอก)

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::stock::repository::ReceiptType;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReceiptLineRequest {
    pub item_id: Uuid,
    /// หน่วยฐานหรือหน่วยแปลงของรายการ
    pub unit_id: Uuid,
    /// จำนวนในหน่วย `unit_id`
    pub quantity: i32,
    /// รายการที่ไม่ติดตามล็อตต้องไม่ส่ง/ว่าง
    pub lot_no: Option<String>,
    /// เฉพาะรายการที่ติดตามวันหมดอายุ
    pub expiry_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateReceiptRequest {
    pub receipt_type: ReceiptType,
    pub budget_year_id: Uuid,
    pub received_on: NaiveDate,
    pub source: Option<String>,
    pub ref_no: Option<String>,
    pub note: Option<String>,
    pub lines: Vec<ReceiptLineRequest>,
}

/// แก้ข้อความหัวใบ — ส่งครบสามช่อง (null/ว่าง = ล้าง)
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateReceiptRequest {
    pub source: Option<String>,
    pub ref_no: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CancelReceiptRequest {
    pub reason: String,
}

/// `GET /stock/lots?item=<uuid ของรายการ>`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LotsQuery {
    pub item: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptResponse {
    pub id: Uuid,
    pub doc_no: String,
    pub budget_year_id: Uuid,
    pub budget_year: String,
    pub receipt_type: ReceiptType,
    pub received_on: NaiveDate,
    pub source: Option<String>,
    pub ref_no: Option<String>,
    pub note: Option<String>,
    pub line_count: i64,
    /// ผู้รับ (ผู้บันทึกใบ)
    pub created_by_name: String,
    pub created_at: DateTime<Utc>,
    /// null = ยังไม่ยกเลิก
    pub cancelled_by_name: Option<String>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancel_reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptLineResponse {
    pub item_id: Uuid,
    pub item_name: String,
    pub lot_no: Option<String>,
    pub expiry_date: Option<NaiveDate>,
    pub unit_name: String,
    /// หน่วยฐานต่อ 1 หน่วยที่กรอก ณ วันรับ
    pub unit_factor: i32,
    pub quantity: i32,
    /// quantity × unit_factor
    pub base_quantity: i32,
    pub base_unit_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptDetailResponse {
    #[serde(flatten)]
    pub receipt: ReceiptResponse,
    pub lines: Vec<ReceiptLineResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedReceiptResponse {
    pub id: Uuid,
    pub doc_no: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitOption {
    pub id: Uuid,
    pub name: String,
    /// หน่วยฐานต่อ 1 หน่วยนี้
    pub factor: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemOption {
    pub id: Uuid,
    pub name: String,
    pub category_name: String,
    pub base_unit_id: Uuid,
    pub base_unit_name: String,
    pub track_lot: bool,
    pub track_expiry: bool,
    /// หน่วยแปลง เรียงตาม factor
    pub conversions: Vec<UnitOption>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetYearOption {
    pub id: Uuid,
    pub year: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

/// ตัวเลือกของฟอร์มใบรับ — เฉพาะรายการ/ปีงบที่ใช้งานอยู่
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionsResponse {
    pub items: Vec<ItemOption>,
    pub budget_years: Vec<BudgetYearOption>,
    pub sources: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LotResponse {
    pub lot_no: String,
    pub expiry_date: Option<NaiveDate>,
    pub qty_on_hand: i32,
}
