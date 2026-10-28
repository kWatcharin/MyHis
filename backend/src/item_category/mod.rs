//! หน้าร้านของโมดูล item_category — ตั้งค่าหมวดคลัง (ตาราง `item_category` ใน docs/my-his-data-dict.xlsx)
//! หน้า master ในกลุ่มเมนู "การตั้งค่า" ใช้สิทธิ์ `item_category.view/create/update/delete` (ตั้งรายเพจที่หน้าการตั้งค่าสิทธิ์)
//! ภาพรวมระบบคลังอยู่ใน docs/inventory-design.md
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (ไม่มี HTTP ในทางผ่าน)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub use repository::ItemCategoryRow;
pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::ItemCategoryError; // error.rs แปลงเป็น AppError ที่เดียว (pub เพราะเทสใน backend/tests/ ต้อง match)
