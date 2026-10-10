//! หน้าร้านของโมดูล lab — ตั้งค่าแลป (ตาราง `lab` ใน docs/my-his-data-dict.xlsx)
//! หน้า master ในกลุ่มเมนู "การตั้งค่า" ใช้สิทธิ์ `user.manage` เหมือนหน้าโรค/ปีงบประมาณ
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (ไม่มี HTTP ในทางผ่าน)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub use repository::LabRow;
pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::LabError; // error.rs แปลงเป็น AppError ที่เดียว (pub เพราะเทสใน backend/tests/ ต้อง match)
