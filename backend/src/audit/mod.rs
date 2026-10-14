//! บันทึกว่าใครทำอะไร — เขียนอย่างเดียว ไม่มีการแก้หรือลบจากในแอป (docs/auth-design.md ข้อ 4)
//!
//! แต่ละโมดูลเขียน `audit_log` ตรงจาก repository ของตัวเอง (โมดูลฟีเจอร์ไม่เรียกกันเอง) โมดูลนี้เป็นฝั่ง "อ่าน"
//! สำหรับหน้าประวัติ (สิทธิ์ `audit.view` ทั้งคู่):
//! - `GET /audit/auth-events` = ประวัติการเข้าออก (login/logout พร้อม IP + User-Agent)
//! - `GET /audit/activities` = ประวัติการใช้งาน (เพิ่ม/แก้ไข/ลบ/ส่งออก ของทุกหน้า — แถวผู้ป่วยไม่มีชื่อรายการ)
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (ไม่มี HTTP ในทางผ่าน)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub use repository::{ActivityRow, AuthEventRow};
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::AuditError; // error.rs แปลงเป็น AppError ที่เดียว (pub เพราะเทสใน backend/tests/ ต้อง match)
