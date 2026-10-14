//! กฎทั้งหมดของ users module: สิทธิ์ราย action ของเพจผู้ใช้งาน/การตั้งค่าสิทธิ์/การตั้งค่าเพจ (`user.*`, `role.*`,
//! `page.*`), invariant (≥1 role เสมอ), กันแอดมินล็อกตัวเองออก, lockout ของ role/page (is_system, in-use)
//! ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 10) แยกไฟล์ตาม entity (users/roles/pages) แบบเดียวกับ
//! `repository/` เพราะรวมไฟล์เดียวยาวเกิน 300 บรรทัด (docs/rust-guidelines.md ข้อ 7)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็คสิทธิ์ของ action นั้น
//! ที่นี่ — "Service เป็นที่ตรวจ permission จริง" (docs/auth-design.md ข้อ 10)
//!
//! **id ขาเข้าจาก API เป็น uuid เสมอ** (`public_id`) — service แปลงเป็น pk int ภายในเองผ่าน
//! repository ถ้าไม่พบคืน `*NotFound` (id int ไม่ออกนอก backend)

mod pages;
mod roles;
mod users;

pub use pages::*;
pub use roles::*;
pub use users::*;

use crate::auth::CurrentUser;
use crate::domain::{Action, Permission, Resource};
use crate::password;
use crate::users::repository::RepositoryError;

#[derive(Debug, thiserror::Error)]
pub enum UsersError {
    #[error("forbidden")]
    Forbidden,
    /// ห้ามให้สิทธิ์เกินตัวเอง (403 พร้อมเหตุผล — ข้อความเป็นค่าคงที่ข้างล่าง)
    #[error("{0}")]
    BeyondOwnPermissions(&'static str),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("user not found")]
    UserNotFound,
    #[error("role not found")]
    RoleNotFound,
    #[error("page not found")]
    PageNotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Password(#[from] password::PasswordError),
}

/// ตรวจสิทธิ์ของ action นี้กับเพจนี้ (`user.*` / `role.*` / `page.*`)
fn require(actor: &CurrentUser, resource: Resource, action: Action) -> Result<(), UsersError> {
    if actor.can(resource, action) {
        Ok(())
    } else {
        Err(UsersError::Forbidden)
    }
}

/// ข้อความ 403 ของกติกาห้ามให้สิทธิ์เกินตัวเอง (docs/auth-design.md ข้อ 10) — หน้าเว็บแปลเป็นไทยจากข้อความเหล่านี้
const GRANT_BEYOND_OWN: &str = "cannot grant permissions you do not have";
const STRONGER_USER: &str = "cannot manage a user with permissions you do not have";
const STRONGER_ROLE: &str = "cannot manage a role with permissions you do not have";

/// ห้ามให้สิทธิ์เกินตัวเอง: ทุกสิทธิ์ที่จะให้หรือจะแตะต้องเป็นสิทธิ์ที่ผู้ทำถืออยู่ (สิทธิ์รวมทุก role ของตัวเอง)
/// ไม่งั้นคนที่จัดการผู้ใช้/role ได้แต่ไม่ใช่ admin จะยกตัวเองหรือคนอื่นเป็น admin หรือยึดบัญชี admin ได้
/// (รีเซ็ตรหัสแล้วได้ลิงก์ตั้งรหัสใหม่มาเอง) — admin มีครบทุกสิทธิ์จึงผ่านเสมอ
fn require_within_own<'a>(
    actor: &CurrentUser,
    permissions: impl IntoIterator<Item = &'a Permission>,
    reason: &'static str,
) -> Result<(), UsersError> {
    if permissions
        .into_iter()
        .all(|permission| actor.has_permission(*permission))
    {
        Ok(())
    } else {
        Err(UsersError::BeyondOwnPermissions(reason))
    }
}

/// กฎกันแอดมินล็อกตัวเองออก: สิทธิ์รวมของตัวเองหลังแก้ (union ทุก role ที่ถือ) ต้องยังมี `role.view` + `role.update`
/// — มีสองตัวนี้ก็เข้าหน้าการตั้งค่าสิทธิ์แล้วแก้สิทธิ์คืนให้ตัวเองได้เสมอ
fn require_role_management_kept(
    permissions: impl IntoIterator<Item = Permission>,
) -> Result<(), UsersError> {
    let held: Vec<Permission> = permissions.into_iter().collect();
    let keeps = [Action::View, Action::Update]
        .into_iter()
        .all(|action| Permission::of(Resource::Role, action).is_some_and(|p| held.contains(&p)));
    if keeps {
        Ok(())
    } else {
        Err(UsersError::Validation(
            "cannot remove your own role management permission".to_string(),
        ))
    }
}

fn map_unique_violation(err: RepositoryError, message: &str) -> UsersError {
    if err.is_unique_violation() {
        UsersError::Conflict(message.to_string())
    } else {
        UsersError::from(err)
    }
}

/// parse permission string จาก DTO — ค่าที่ไม่รู้จัก = ผู้ใช้ส่งข้อมูลผิด ไม่ใช่ข้อมูล DB เสีย
/// (ต่างจาก RepositoryError::UnknownPermission ที่หมายถึงอ่านจาก DB แล้วเจอค่าที่ไม่รู้จัก)
fn parse_permissions(raw: &[String]) -> Result<Vec<Permission>, UsersError> {
    raw.iter()
        .map(|p| {
            Permission::parse(p)
                .ok_or_else(|| UsersError::Validation(format!("unknown permission: {p}")))
        })
        .collect()
}
