//! หน้าร้านของโมดูล patient — ลงทะเบียนผู้ป่วย (ตาราง `patient`) + ผูกโรคประจำตัว (`medical_condition`)
//! นี่คือโมดูลแรกที่เขียน/อ่านข้อมูลระบุตัวคนไข้ (HN, เลขประจำตัวคนไข้, ชื่อ, สกุล) จึงต้องทำตามกติกาใน
//! CLAUDE.md "ขอบเขตข้อมูล": เข้ารหัสก่อนลง DB (`crate::crypto`), ค้นด้วย blind index เท่านั้น,
//! ห้าม log ค่าเหล่านี้ และไม่ส่งเข้า audit metadata
//!
//! ใช้สิทธิ์ `patient.read` (อ่าน) / `patient.write` (สร้าง-แก้-ลบ) ไม่ใช่ `user.manage` — ผู้ป่วยเป็นงานของ
//! พยาบาล ไม่ใช่งานแอดมิน ข้อมูล master (คำนำหน้า, สัญชาติ, โรค) อ่านผ่าน query ของโมดูลนี้เอง
//! (`GET /patients/options`) เพราะ `/diseases` ฯลฯ ต้อง `user.manage`
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (ไม่มี HTTP ในทางผ่าน)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub use repository::{ConditionRow, MasterOptions};
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::{PatientError, PatientInput, PatientView}; // error.rs แปลงเป็น AppError ที่เดียว (+ ให้เทส match/สร้าง input)
