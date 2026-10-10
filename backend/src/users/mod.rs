//! หน้าร้านของโมดูล users — ระบุชัดว่าเปิดอะไรออกไป (docs/rust-guidelines.md ข้อ 7)
//! จัดการบัญชีผู้ใช้ (เฉพาะ admin — หน้า "ผู้ใช้งาน" /users กลุ่มเมนู "ลงทะเบียน") + role + page/menu
//! (docs/auth-design.md หัวข้อ 10, 11) — ทั้งสามเรื่องนี้อยู่โมดูลเดียวกันเพราะเป็นกลุ่ม "การจัดการสิทธิ์
//! ระดับแอดมิน" เดียวกัน ไม่ใช่ auth/session — ดูเหตุผลในหัวข้อ 11
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (แบบเดียวกับ lab_visit)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::UsersError; // error.rs แปลงเป็น AppError ที่เดียว (pub เพราะเทสใน backend/tests/ ต้อง match)
