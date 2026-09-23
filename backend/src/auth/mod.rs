//! หน้าร้านของโมดูล auth — ระบุชัดว่าเปิดอะไรออกไป
//! ดีไซน์เต็ม: docs/auth-design.md
//!
//! ชั้นในโมดูลนี้ (routes -> handlers -> service -> repository ทางเดียว):
mod routes;
mod extractor;
mod handlers;
mod service;
mod repository;
mod dto;
mod password;
mod session;

// pub(crate) use extractor::CurrentUser; // โครงสร้างพื้นฐานเดียวที่โมดูลอื่นเรียกใช้ได้
