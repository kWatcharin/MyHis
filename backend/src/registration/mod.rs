//! หน้าร้านของโมดูล registration — ลงทะเบียนผู้ใช้งานผ่านลิงก์ที่แอดมินออกให้ (docs/auth-design.md ข้อ 5, 8)
//!
//! แอดมินสร้างบัญชี/รีเซ็ตรหัสที่หน้า "ผู้ใช้งาน" (โมดูล users ออกลิงก์ใช้ครั้งเดียว อายุ 3 วัน) → ผู้ใช้เปิด
//! `/register#<token>` แล้วเรียก endpoint สาธารณะของโมดูลนี้: ดูข้อมูลบัญชี (lookup) และลงทะเบียน/ตั้งรหัสใหม่
//! (complete) — ผู้ใช้กรอกคำนำหน้า ชื่อ สกุล ตำแหน่งเองตอนลงทะเบียนครั้งแรก และตั้งรหัสผ่านเอง
//!
//! `service` เปิดเป็น pub เพราะเทสใน backend/tests/ ต้อง match error (แบบเดียวกับ lab_visit/users)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::RegistrationError; // error.rs แปลงเป็น AppError ที่เดียว
