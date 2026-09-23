//! รูปแบบ JSON เข้า/ออกของโมดูล auth — แยกจากชนิดโดเมนและ struct ของตาราง
//! (docs/rust-guidelines.md ข้อ 4, 12) DTO ขาเข้าทุกตัว deny_unknown_fields + validate ที่
//! handler ก่อนส่งต่อ service (parse, don't validate)
//!
//! ชนิดที่ถือรหัสผ่านเขียน Debug เองให้พิมพ์เป็น [redacted] (docs/rust-guidelines.md ข้อ 9)
//! หมายเหตุ: handler ที่รับพารามิเตอร์เหล่านี้ยังต้องใส่ #[instrument(skip(...))] เองด้วย
//! การมี Debug ที่ปลอดภัยไม่ได้แปลว่า instrument จะ skip ให้อัตโนมัติ

use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::domain::{Permission, User};

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    #[validate(length(min = 1, message = "username is required"))]
    pub username: String,
    #[validate(length(min = 1, message = "password is required"))]
    pub password: String,
}

impl std::fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginRequest")
            .field("username", &self.username)
            .field("password", &"[redacted]")
            .finish()
    }
}

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    #[validate(length(min = 1, message = "current password is required"))]
    pub current_password: String,
    #[validate(length(min = 12, message = "new password must be at least 12 characters"))]
    pub new_password: String,
}

impl std::fmt::Debug for ChangePasswordRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChangePasswordRequest")
            .field("current_password", &"[redacted]")
            .field("new_password", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: uuid::Uuid,
    pub username: String,
    pub display_name: String,
    pub must_change_password: bool,
    pub permissions: Vec<&'static str>,
}

impl UserResponse {
    pub fn from_domain(user: &User, permissions: &[Permission]) -> Self {
        UserResponse {
            id: user.id.0,
            username: user.username.clone(),
            display_name: user.display_name.clone(),
            must_change_password: user.must_change_password,
            permissions: permissions.iter().map(|p| p.as_str()).collect(),
        }
    }
}
