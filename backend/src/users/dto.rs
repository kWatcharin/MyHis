//! รูปแบบ JSON เข้า/ออกของโมดูล users (บัญชีผู้ใช้ + role + page/menu)
//! `Permission` ไม่มี serde derive โดยตั้งใจ (ปิดตายใน Rust) — DTO รับเป็น `Vec<String>` แล้วให้
//! handler แปลงผ่าน `Permission::parse` เอง ค่าที่แปลงไม่ได้ = `AppError::Validation`
//!
//! ชนิดที่ถือรหัสผ่านชั่วคราวเขียน Debug เองให้พิมพ์เป็น [redacted] (docs/rust-guidelines.md ข้อ 9)

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
    pub roles: Vec<RoleRef>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateUserResponse {
    pub user: UserSummary,
    pub temporary_password: String,
}

impl std::fmt::Debug for CreateUserResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreateUserResponse")
            .field("user", &self.user)
            .field("temporary_password", &"[redacted]")
            .finish()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetPasswordResponse {
    pub temporary_password: String,
}

impl std::fmt::Debug for ResetPasswordResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResetPasswordResponse")
            .field("temporary_password", &"[redacted]")
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
    #[validate(length(min = 1, message = "route is required"))]
    pub route: String,
    pub sort_order: Option<i32>,
    pub permissions: Vec<String>,
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResponse {
    pub id: Uuid,
    pub label: String,
    pub icon: Option<String>,
    pub route: String,
    pub sort_order: i32,
    pub permissions: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuItemResponse {
    pub id: Uuid,
    pub label: String,
    pub icon: Option<String>,
    pub route: String,
}
