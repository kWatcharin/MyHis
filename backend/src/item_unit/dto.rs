//! รูปแบบ JSON เข้า/ออกของโมดูล item_unit — แยกจาก struct ของตาราง (docs/rust-guidelines.md ข้อ 4)
//! `id` ที่ออก API คือ uuid เสมอ (id int ภายในไม่ออกนอก backend) การตรวจค่า (trim, ไม่ว่าง, ยาวไม่เกิน
//! 100 ตัวอักษร) ทำที่ service ที่เดียว เพราะเทสระดับ service เรียกตรงโดยไม่ผ่าน DTO

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateItemUnitRequest {
    pub name: String,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateItemUnitRequest {
    pub name: Option<String>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemUnitResponse {
    pub id: Uuid,
    pub name: String,
    pub sort_order: i32,
    pub is_active: bool,
}
