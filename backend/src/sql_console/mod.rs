//! หน้า SQL (`/data/script`, สิทธิ์ `sql.view`): รัน SQL อ่านอย่างเดียว แล้วถอดรหัสคอลัมน์ผู้ป่วยในผลลัพธ์ให้
//! (Postgres ถอด AES-GCM ของ crypto.rs เองไม่ได้ เพราะ key อยู่ที่ backend) — ดู docs/infrastructure.md หัวข้อหน้า SQL
//!
//! แนวป้องกัน (backend ต่อ DB ด้วย superuser — SQL ดิบผ่าน pool หลักจะ COPY TO PROGRAM / pg_read_file /
//! RESET ROLE ได้แม้อยู่ใน transaction READ ONLY):
//! - รันด้วย role แยก (`READER_ROLE`) ที่ไม่ใช่ superuser มีแค่ SELECT ระดับคอลัมน์ตาม allowlist (setup.rs)
//! - connection ใหม่ทุกครั้ง, READ ONLY, คำสั่งเดียว, timeout, จำกัดจำนวนแถว (repository.rs, sql_text.rs)
//! - ถอดรหัสเมื่อมี `patient.view` เท่านั้น: คอลัมน์จริงของ patient (จับคู่ด้วย OID/attnum ไม่ใช่ชื่อ, service.rs) และ
//!   ciphertext ที่ฝังใน expression เช่น concat/cast (embedded.rs — GCM ตรวจ tag + AAD จึงถอดได้แค่ ciphertext จริง)
//! - ทุกการรันเขียน audit_log โดยไม่เก็บตัว SQL — ไม่ log SQL, ข้อความ error ของ DB หรือค่าในผลลัพธ์

mod dto;
mod embedded;
mod handlers;
mod repository;
mod routes;
mod service;
mod setup;
mod sql_text;
mod value;

pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::{Blocked, QueryFailure, SqlConsoleError, MAX_ROWS};
pub use setup::{setup, SetupError, SqlConsole, READER_ROLE};
