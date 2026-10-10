//! กฎทั้งหมดของ users module: สิทธิ์ (ต้องมี user.manage), invariant (≥1 role เสมอ), กันแอดมินล็อก
//! ตัวเองออก, lockout ของ role/page (is_system, in-use) ห้าม import ชนิดข้อมูลของ Axum
//! (docs/auth-design.md ข้อ 10) แยกไฟล์ตาม entity (users/roles/pages) แบบเดียวกับ `repository/`
//! เพราะรวมไฟล์เดียวยาวเกิน 300 บรรทัด (docs/rust-guidelines.md ข้อ 7)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็ค `user.manage`
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
use crate::domain::Permission;
use crate::password;
use crate::users::repository::RepositoryError;

#[derive(Debug, thiserror::Error)]
pub enum UsersError {
    #[error("forbidden")]
    Forbidden,
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

fn require_user_manage(actor: &CurrentUser) -> Result<(), UsersError> {
    if actor.has_permission(Permission::UserManage) {
        Ok(())
    } else {
        Err(UsersError::Forbidden)
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
