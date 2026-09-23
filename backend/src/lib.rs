//! Library root ของ backend — เปิดโมดูลให้ `backend/tests/` เรียกได้ตรง ๆ (integration test
//! เป็นคนละ crate จาก main.rs เรียกจากไฟล์นั้นไม่ได้) `main.rs` เป็นแค่ thin wrapper ที่เรียกจากที่นี่
//!
//! `audit/` ยังเป็น stub เฉย ๆ (ยังไม่ implement รอบนี้) เลยยังไม่ mod เข้ามาที่นี่ จะเปิดตอนถึง
//! รอบของมัน

pub mod app;
pub mod auth;
pub mod config;
pub mod db;
pub mod domain;
pub mod error;
pub mod password;
pub mod state;
pub mod users;
