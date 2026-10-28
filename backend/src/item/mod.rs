//! หน้าร้านของโมดูล item — ตั้งค่ารายการคลัง + หน่วยแปลงของรายการ (ตาราง `item`, `item_unit_conversion`
//! ใน docs/my-his-data-dict.xlsx) หน้า master ในกลุ่มเมนู "การตั้งค่า" ใช้สิทธิ์ `item.view/create/update/delete`
//! (หน่วยแปลงแก้ในฟอร์มของรายการ ใช้สิทธิ์เดียวกัน) ภาพรวมระบบคลังอยู่ใน docs/inventory-design.md
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (ไม่มี HTTP ในทางผ่าน)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub use repository::{CategoryOptionRow, ConversionRow, ItemOptions, ItemRow, UnitOptionRow};
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::ItemError; // error.rs แปลงเป็น AppError ที่เดียว (pub เพราะเทสใน backend/tests/ ต้อง match)
