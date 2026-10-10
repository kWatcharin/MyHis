//! หน้าร้านของโมดูล budget_year — ตั้งค่าปีงบประมาณ (ตาราง `budget_year` ใน docs/my-his-data-dict.xlsx)
//! เป็นหน้า master ที่อยู่ในกลุ่มเมนู "การตั้งค่า" ใช้สิทธิ์ `user.manage` เหมือนหน้าตั้งค่าเพจ/role
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (ไม่มี HTTP ในทางผ่าน)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub use repository::BudgetYearRow;
pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::BudgetYearError; // error.rs แปลงเป็น AppError ที่เดียว (pub เพราะเทสใน backend/tests/ ต้อง match)
