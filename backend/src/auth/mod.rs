//! หน้าร้านของโมดูล auth — ระบุชัดว่าเปิดอะไรออกไป (docs/rust-guidelines.md ข้อ 7)
//! ดีไซน์เต็ม: docs/auth-design.md
//!
//! ชั้นในโมดูลนี้ (routes -> handlers -> service -> repository ทางเดียว):
mod dto;
mod extractor;
mod handlers;
mod repository;
mod routes;
mod service;
mod session;

// password.rs ย้ายไปเป็น crate::password (top-level) แล้ว เพราะ users module ก็ต้องใช้ด้วย —
// โมดูลฟีเจอร์ไม่เรียกกันเอง เลยต้องยกตรรกะร่วมขึ้นไปที่ crate root (docs/rust-guidelines.md)

// สิ่งที่เปิดออกให้โค้ดนอกโมดูล auth เรียกได้ — เท่าที่จำเป็นจริงเท่านั้น
pub use extractor::CurrentUser; // โครงสร้างพื้นฐานเดียวที่โมดูลอื่นข้ามฟีเจอร์เรียกใช้ได้ (pub เพราะเทสระดับ service ใน backend/tests/ ต้องสร้างเอง)
pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้ (app.rs อยู่ crate เดียวกัน)
pub(crate) use service::AuthError; // error.rs แปลงเป็น AppError ที่เดียว

// main.rs เป็นคนละ crate จาก lib (thin binary ที่ใช้ lib นี้เป็น dependency) — pub(crate) ไปไม่ถึง
// ต้อง pub จริง ๆ เท่านั้นสำหรับของที่ main.rs ต้องเรียกตอน startup
pub use service::bootstrap_first_admin;
