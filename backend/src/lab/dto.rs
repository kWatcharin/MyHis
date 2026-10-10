//! รูปแบบ JSON เข้า/ออกของโมดูล lab — แยกจาก struct ของตาราง (docs/rust-guidelines.md ข้อ 4)
//! `id` ที่ออก API คือ uuid เสมอ (id int ภายในไม่ออกนอก backend) การตรวจค่า (trim, ไม่ว่าง, ยาวไม่เกิน
//! 100 ตัวอักษร) ทำที่ service ที่เดียว เพราะเทสระดับ service เรียกตรงโดยไม่ผ่าน DTO

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateLabRequest {
    pub name: String,
    pub abbreviation: Option<String>,
    pub is_active: Option<bool>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateLabRequest {
    pub name: Option<String>,
    pub abbreviation: Option<String>,
    pub is_active: Option<bool>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LabResponse {
    pub id: Uuid,
    pub name: String,
    pub abbreviation: Option<String>,
    pub is_active: bool,
    pub sort_order: i32,
}
