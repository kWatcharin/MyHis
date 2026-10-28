//! หน้าร้านของโมดูล stock — ธุรกรรมคลัง: ล็อต, คงเหลือต่อล็อต, ledger (`stock_movement`) และเอกสารที่ลง ledger
//! ตอนนี้มีใบรับเข้า + ยอดยกมา (หน้า "รับเข้า" กลุ่มเมนู "คลัง" สิทธิ์ `stock_receipt.*`) — เบิก/นับ/ปรับยอดจะเพิ่ม
//! ในโมดูลนี้ (ใช้ ledger ร่วมกัน — docs/inventory-design.md ข้อ 7) master คลังอยู่ที่ item_category/ item_unit/ item/
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (ไม่มี HTTP ในทางผ่าน)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub use repository::{
    BudgetYearOptionRow, ConversionOptionRow, ItemOptionRow, LotRow, ReceiptLineRow, ReceiptRow,
    ReceiptType,
};
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::StockError; // error.rs แปลงเป็น AppError ที่เดียว (pub เพราะเทสใน backend/tests/ ต้อง match)
