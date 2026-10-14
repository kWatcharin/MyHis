//! หน้าร้านของโมดูล patient_group — ตั้งค่ากลุ่มคนไข้ (ตาราง `patient_group` ใน docs/my-his-data-dict.xlsx)
//! หน้า master ในกลุ่มเมนู "การตั้งค่า" ใช้สิทธิ์ `patient_group.view/create/update/delete` (ตั้งรายเพจที่หน้าการตั้งค่าสิทธิ์)
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (ไม่มี HTTP ในทางผ่าน)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub use repository::PatientGroupRow;
pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::PatientGroupError; // error.rs แปลงเป็น AppError ที่เดียว (pub เพราะเทสใน backend/tests/ ต้อง match)
