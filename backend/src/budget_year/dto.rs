//! รูปแบบ JSON เข้า/ออกของโมดูล budget_year — แยกจาก struct ของตาราง (docs/rust-guidelines.md ข้อ 4)
//! `id` ที่ออก API คือ uuid เสมอ (id int ภายในไม่ออกนอก backend) วันที่เป็น "YYYY-MM-DD"

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateBudgetYearRequest {
    /// ปีงบประมาณ พ.ศ. 4 หลัก (service ตรวจว่าเป็นตัวเลขล้วนอีกชั้น)
    #[validate(length(equal = 4, message = "year must be 4 digits"))]
    pub year: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateBudgetYearRequest {
    #[validate(length(equal = 4, message = "year must be 4 digits"))]
    pub year: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub sort_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BudgetYearResponse {
    pub id: Uuid,
    pub year: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub sort_order: i32,
    pub is_active: bool,
}
