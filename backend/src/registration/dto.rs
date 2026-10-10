//! รูปแบบ JSON เข้า/ออกของการลงทะเบียนผ่านลิงก์ — token อยู่ใน body ของ POST เสมอ (ไม่อยู่ใน path/query ที่ไปติด
//! log ของ server) ชนิดที่ถือ token/รหัสผ่าน/ชื่อ-สกุลเขียน Debug เองให้ไม่พิมพ์ค่า (docs/rust-guidelines.md ข้อ 9)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LookupRequest {
    pub token: String,
}

impl std::fmt::Debug for LookupRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LookupRequest")
            .field("token", &"[redacted]")
            .finish()
    }
}

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CompleteRequest {
    pub token: String,
    // เกณฑ์เดียวกับ POST /auth/password (docs/auth-design.md ข้อ 8)
    #[validate(length(min = 12, message = "new password must be at least 12 characters"))]
    pub new_password: String,
    pub prename_id: Option<Uuid>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub position: Option<String>,
}

impl std::fmt::Debug for CompleteRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompleteRequest")
            .field("token", &"[redacted]")
            .field("new_password", &"[redacted]")
            .finish_non_exhaustive()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrenameOption {
    pub id: Uuid,
    pub name: String,
}

/// ข้อมูลของบัญชีเจ้าของลิงก์ให้หน้าลงทะเบียนแสดง + ค่าเดิมของข้อมูลตัว (ถ้าแอดมินกรอกไว้ก่อน) + ตัวเลือกคำนำหน้า
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LookupResponse {
    pub username: String,
    pub display_name: String,
    /// "register" = ลงทะเบียนครั้งแรก (กรอกข้อมูลตัว + ตั้งรหัส), "reset" = ตั้งรหัสใหม่อย่างเดียว
    pub mode: &'static str,
    pub expires_at: DateTime<Utc>,
    pub prename_id: Option<Uuid>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub position: Option<String>,
    pub prenames: Vec<PrenameOption>,
}
