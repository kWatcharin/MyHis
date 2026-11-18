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
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct LoginRequest {
    // เพดานบนกันชื่อ/รหัสยาวหลายร้อย KB (เดิมมีแค่เพดาน body 256 KiB): ชื่อที่เก็บจริงยาว ≤ 32 (domain::username) เผื่อช่องว่างหัวท้าย
    #[validate(length(min = 1, max = 64, message = "username is required"))]
    pub username: String,
    #[validate(length(min = 1, max = 1024, message = "password is required"))]
    pub password: String,
    /// token จาก widget Cloudflare Turnstile — ไม่มีได้เมื่อปิดฟีเจอร์ ใช้ได้ครั้งเดียว ห้าม log
    /// (Cloudflare กำหนดยาวสุด 2048 ตัวอักษร)
    #[validate(length(max = 2048, message = "turnstile token is too long"))]
    pub turnstile_token: Option<String>,
}

impl std::fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoginRequest")
            .field("username", &self.username)
            .field("password", &"[redacted]")
            .field(
                "turnstile_token",
                &self.turnstile_token.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

/// สิ่งที่หน้า login ต้องรู้ก่อนแสดงฟอร์ม (`GET /auth/login-config` สาธารณะ)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginConfigResponse {
    /// site key ของ widget Turnstile (ค่าสาธารณะ) — null = ปิด ไม่ต้องแสดง widget
    pub turnstile_site_key: Option<String>,
}

#[derive(Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ChangePasswordRequest {
    #[validate(length(min = 1, max = 1024, message = "current password is required"))]
    pub current_password: String,
    #[validate(length(
        min = 12,
        max = 1024,
        message = "new password must be at least 12 characters (at most 1024)"
    ))]
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
    pub fn from_domain(user: User, permissions: &[Permission]) -> Self {
        UserResponse {
            id: user.public_id,
            username: user.username,
            display_name: user.display_name,
            must_change_password: user.must_change_password,
            permissions: permissions.iter().map(|p| p.as_str()).collect(),
        }
    }
}
