//! รูปแบบ JSON เข้า/ออกของโมดูล users (บัญชีผู้ใช้ + role + page/menu)
//! `Permission` ไม่มี serde derive โดยตั้งใจ (ปิดตายใน Rust) — DTO รับเป็น `Vec<String>` แล้วให้
//! handler แปลงผ่าน `Permission::parse` เอง ค่าที่แปลงไม่ได้ = `AppError::Validation`
//!
//! ชนิดที่ถือ token ของลิงก์ลงทะเบียนเขียน Debug เองให้พิมพ์เป็น [redacted] (docs/rust-guidelines.md ข้อ 9)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

// --- users ---

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateUserRequest {
    #[validate(length(min = 1, message = "username is required"))]
    pub username: String,
    #[validate(length(min = 1, message = "display name is required"))]
    pub display_name: String,
    pub role_id: Uuid,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateUserRequest {
    #[validate(length(min = 1, message = "display name cannot be empty"))]
    pub display_name: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReplaceUserRolesRequest {
    pub role_ids: Vec<Uuid>,
}

/// `PUT /users/{id}` — แทนที่ทั้งชุด (ช่องที่ไม่ส่ง/null = ล้างค่า) ตรวจค่าจริงที่ service
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReplaceUserRequest {
    pub display_name: String,
    pub prename_id: Option<Uuid>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub position: Option<String>,
    pub role_ids: Vec<Uuid>,
}

// ชื่อ-สกุลเจ้าหน้าที่เป็นข้อมูลส่วนบุคคล — Debug ไม่พิมพ์ค่า
impl std::fmt::Debug for ReplaceUserRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReplaceUserRequest")
            .field("role_ids", &self.role_ids)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleRef {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSummary {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub is_active: bool,
    pub must_change_password: bool,
    /// ล็อกชั่วคราวจากการใส่รหัสผิดหลายครั้งถึงเวลานี้ (null หรือเวลาในอดีต = ไม่ล็อก) — รีเซ็ตรหัสปลดล็อกให้
    pub locked_until: Option<DateTime<Utc>>,
    pub prename_id: Option<Uuid>,
    pub prename_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub position: Option<String>,
    /// ลงทะเบียน (ตั้งรหัสผ่านเองผ่านลิงก์) สำเร็จเมื่อ — null = ยังไม่ลงทะเบียน login ไม่ได้
    pub registered_at: Option<DateTime<Utc>>,
    /// หมดอายุของลิงก์ลงทะเบียน/ตั้งรหัสที่ค้างอยู่ (อาจหมดอายุแล้ว) — null = ไม่มีลิงก์ค้าง
    pub registration_expires_at: Option<DateTime<Utc>>,
    pub roles: Vec<RoleRef>,
    pub created_at: DateTime<Utc>,
}

/// token ของลิงก์ลงทะเบียน/ตั้งรหัสใหม่ — อยู่ใน response นี้ครั้งเดียว (DB เก็บแค่ hash) หน้าเว็บประกอบเป็นลิงก์เอง
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationLinkResponse {
    pub registration_token: String,
    pub registration_expires_at: DateTime<Utc>,
}

impl std::fmt::Debug for RegistrationLinkResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistrationLinkResponse")
            .field("registration_token", &"[redacted]")
            .field("registration_expires_at", &self.registration_expires_at)
            .finish()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserResponse {
    pub user: UserSummary,
    #[serde(flatten)]
    pub link: RegistrationLinkResponse,
}

impl std::fmt::Debug for CreateUserResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateUserResponse")
            .field("user", &self.user.id)
            .field("link", &self.link)
            .finish()
    }
}

// --- roles ---

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreateRoleRequest {
    #[validate(length(min = 1, message = "role name is required"))]
    pub name: String,
    pub description: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdateRoleRequest {
    #[validate(length(min = 1, message = "role name cannot be empty"))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub permissions: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoleResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub permissions: Vec<&'static str>,
}

// --- pages ---

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CreatePageRequest {
    #[validate(length(min = 1, message = "label is required"))]
    pub label: String,
    pub icon: Option<String>,
    // ไม่ส่ง/null = เพจหลักที่เป็นหัวข้อกลุ่มเมนู (ไม่มีหน้าของตัวเอง) — เพจย่อยต้องมี route (service ตรวจ)
    #[validate(length(min = 1, message = "route cannot be empty"))]
    pub route: Option<String>,
    pub sort_order: Option<i32>,
    pub permissions: Vec<String>,
    /// uuid ของเพจหลัก (ไม่ส่ง/null = เป็นเพจหลักเอง)
    pub ref_id: Option<Uuid>,
}

/// แยก "ไม่ส่ง field" (None — ใช้คู่กับ `#[serde(default)]`) ออกจาก "ส่ง null" (Some(None)) — PATCH ต้องรู้ว่า
/// ผู้ใช้ตั้งใจล้างค่าหรือแค่ไม่แตะ (Option ชั้นเดียวของ serde รวมสองกรณีนี้เป็น None)
fn double_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UpdatePageRequest {
    #[validate(length(min = 1, message = "label cannot be empty"))]
    pub label: Option<String>,
    pub icon: Option<String>,
    #[validate(length(min = 1, message = "route cannot be empty"))]
    pub route: Option<String>,
    pub sort_order: Option<i32>,
    pub permissions: Option<Vec<String>>,
    /// ไม่ส่ง = ไม่แตะ, null = ย้ายขึ้นเป็นเพจหลัก, uuid = ย้ายไปอยู่ใต้เพจหลักนั้น
    #[serde(default, deserialize_with = "double_option")]
    pub ref_id: Option<Option<Uuid>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResponse {
    pub id: Uuid,
    pub label: String,
    pub icon: Option<String>,
    /// null = เพจหลักที่เป็นหัวข้อกลุ่มเมนู
    pub route: Option<String>,
    pub sort_order: i32,
    pub permissions: Vec<&'static str>,
    /// uuid ของเพจหลัก (null = เป็นเพจหลักเอง)
    pub ref_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuItemResponse {
    pub id: Uuid,
    pub label: String,
    pub icon: Option<String>,
    pub route: Option<String>,
    pub ref_id: Option<Uuid>,
}
