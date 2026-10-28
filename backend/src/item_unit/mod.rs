//! หน้าร้านของโมดูล item_unit — ตั้งค่าหน่วยนับ (ตาราง `item_unit` ใน docs/my-his-data-dict.xlsx)
//! หน้า master ในกลุ่มเมนู "การตั้งค่า" ใช้สิทธิ์ `item_unit.view/create/update/delete` (ตั้งรายเพจที่หน้าการตั้งค่าสิทธิ์)
//! ชื่อหน่วยใช้ร่วมกันทุกรายการ ส่วนอัตราแปลงเป็นหน่วยฐานเป็นของแต่ละรายการ (โมดูล item)
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (ไม่มี HTTP ในทางผ่าน)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub use repository::ItemUnitRow;
pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::ItemUnitError; // error.rs แปลงเป็น AppError ที่เดียว (pub เพราะเทสใน backend/tests/ ต้อง match)
