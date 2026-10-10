//! หน้าร้านของโมดูล lab_visit — นัดหมายแล็ป: หัวนัดหมายต่อปีงบประมาณ + กลุ่มคนไข้ (`lab_visit_group`
//! วันนัด CD4 ครั้งที่ 1/2 และวันนัด VL) และรายชื่อคนไข้ในนัดหมาย (`lab_visit` ระยะเวลากินยา ผล VL สูตรยา)
//!
//! ใช้สิทธิ์ผู้ป่วยเดิม (`patient.read` อ่าน / `patient.write` เขียน) และ **ไม่แตะข้อมูลระบุตัวคนไข้** —
//! API คืนแค่ uuid ของคนไข้ หน้าเว็บ join กับ `GET /patients` (ถอดรหัสที่โมดูล patient ที่เดียว) ข้อมูล master
//! ของฟอร์ม (ปีงบประมาณ, กลุ่มคนไข้) อ่านผ่าน query ของโมดูลนี้เอง (`GET /lab-visit-groups/options`)
//! เพราะ `/budget-years`, `/patient-groups` ต้อง `user.manage`
//!
//! `service` เปิดเป็น pub เพราะเทสระดับ service ใน backend/tests/ เรียกตรง ๆ (ไม่มี HTTP ในทางผ่าน)

mod dto;
mod handlers;
mod repository;
mod routes;
pub mod service;

pub(crate) use repository::RepositoryError; // error.rs แปลงเป็น AppError ที่เดียว
pub use repository::{GroupRow, MasterOptions, VisitRow};
pub(crate) use routes::router; // จุดเดียวที่ app.rs รู้จักโมดูลนี้
pub use service::LabVisitError; // error.rs แปลงเป็น AppError ที่เดียว (pub เพราะเทสใน backend/tests/ ต้อง match)
