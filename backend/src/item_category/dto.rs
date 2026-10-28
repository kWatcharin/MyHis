//! รูปแบบ JSON เข้า/ออกของโมดูล item_category — แยกจาก struct ของตาราง (docs/rust-guidelines.md ข้อ 4)
//! `id` ที่ออก API คือ uuid เสมอ (id int ภายในไม่ออกนอก backend) การตรวจค่า (trim, ไม่ว่าง, ยาวไม่เกิน
//! 100 ตัวอักษร, ติดตามวันหมดอายุต้องติดตามล็อต) ทำที่ service ที่เดียว เพราะเทสระดับ service เรียกตรงโดยไม่ผ่าน DTO

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateItemCategoryRequest {
    pub name: String,
    pub default_track_lot: Option<bool>,
    pub default_track_expiry: Option<bool>,
    pub default_patient_linkable: Option<bool>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateItemCategoryRequest {
    pub name: Option<String>,
    pub default_track_lot: Option<bool>,
    pub default_track_expiry: Option<bool>,
    pub default_patient_linkable: Option<bool>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemCategoryResponse {
    pub id: Uuid,
    pub name: String,
    pub default_track_lot: bool,
    pub default_track_expiry: bool,
    pub default_patient_linkable: bool,
    pub sort_order: i32,
    pub is_active: bool,
}
