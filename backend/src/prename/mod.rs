//! หน้าร้านของโมดูล prename — ตั้งค่าคำนำหน้า (ตาราง `prename` ใน docs/my-his-data-dict.xlsx)
//! หน้า master ในกลุ่มเมนู "การตั้งค่า" ใช้สิทธิ์ `user.manage` เหมือนหน้าโรค/ปีงบประมาณ
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (ไม่มี HTTP ในทางผ่าน)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub use repository::PrenameRow;
pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::PrenameError; // error.rs แปลงเป็น AppError ที่เดียว (pub เพราะเทสใน backend/tests/ ต้อง match)
