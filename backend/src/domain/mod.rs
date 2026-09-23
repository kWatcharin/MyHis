//! ชนิดข้อมูลกลาง: UserId, RoleId, PageId, Permission, User (docs/rust-guidelines.md ข้อ 4)
//! newtype แทนชนิดพื้นฐานสำหรับ id, enum แทน string ที่ส่งไปมา
//! ชนิดในไฟล์นี้ไม่ผูกกับรูปแบบ JSON ของ API โดยตรง — auth::dto เป็นคนแปลงเป็น wire format
//! (แยก DTO ออกจากชนิดโดเมน เหมือนที่แยกจาก struct ของตาราง)

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct UserId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct RoleId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(transparent)]
pub struct PageId(pub Uuid);

/// ชุด permission ที่มีอยู่จริงในระบบ — ปิดตาย ผูกกับ authorization check จริงในโค้ดเท่านั้น
/// สร้างผ่าน UI ไม่ได้ role ไหนมี permission ไหนเป็นข้อมูลใน DB (role_permissions) ไม่ใช่
/// compile-time อีกต่อไป (docs/auth-design.md ข้อ 10)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    UserManage,
    ShiftRead,
    ShiftWrite,
    AuditRead,
}

impl Permission {
    /// ค่าที่เก็บใน role_permissions.permission และใน seed migration — ต้องตรงกันเป๊ะ
    pub fn as_str(self) -> &'static str {
        match self {
            Permission::UserManage => "user.manage",
            Permission::ShiftRead => "shift.read",
            Permission::ShiftWrite => "shift.write",
            Permission::AuditRead => "audit.read",
        }
    }

    /// parse ค่าที่มาจาก DB — ถ้าไม่รู้จักถือว่าข้อมูลเสีย ให้ error ไม่ใช่ข้าม
    /// (docs/auth-design.md ข้อ 4)
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "user.manage" => Some(Permission::UserManage),
            "shift.read" => Some(Permission::ShiftRead),
            "shift.write" => Some(Permission::ShiftWrite),
            "audit.read" => Some(Permission::AuditRead),
            _ => None,
        }
    }
}

/// ผู้ใช้ระดับโดเมน — ไม่มี password_hash ไม่ผูกกับรูปแบบ JSON โดยตรง
#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub display_name: String,
    pub must_change_password: bool,
}
