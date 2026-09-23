//! query SQLx ต่อ users, roles, role_permissions, user_roles, pages, page_permissions
//! (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! เพื่อตรวจตอน compile ห้ามประกอบ SQL ด้วย format!
//! (docs/rust-guidelines.md ข้อ 11) แยกไฟล์ตาม entity (users/roles/pages) เพราะรวมไฟล์เดียวยาว
//! เกิน 300 บรรทัด (docs/rust-guidelines.md ข้อ 7)
//!
//! หมายเหตุ: ฟังก์ชันสร้างผู้ใช้/ลบ session ในนี้คล้ายกับใน `auth::repository` โดยตั้งใจ — โมดูล
//! ฟีเจอร์ไม่เรียกกันเอง (docs/rust-guidelines.md) เลยต้องมี query ของตัวเองแยกจาก auth แม้จะซ้ำ
//! กันบางส่วน

mod pages;
mod roles;
mod users;

pub(crate) use pages::*;
pub(crate) use roles::*;
pub(crate) use users::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("unknown permission stored in database: {0}")]
    UnknownPermission(String),
}

impl RepositoryError {
    /// เช็คว่า error เกิดจาก unique constraint ชนไหม (username/role name/page route ซ้ำ) —
    /// ให้ service ชั้นบนแปลงเป็น Conflict แทนที่จะ pre-check แข่งกับ concurrent request เอง
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, RepositoryError::Database(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()))
    }
}
