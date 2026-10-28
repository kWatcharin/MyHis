//! Library root ของ backend — เปิดโมดูลให้ `backend/tests/` เรียกได้ตรง ๆ (integration test
//! เป็นคนละ crate จาก main.rs เรียกจากไฟล์นั้นไม่ได้) `main.rs` เป็นแค่ thin wrapper ที่เรียกจากที่นี่

pub mod app;
pub mod audit;
pub mod auth;
pub mod budget_year;
pub mod config;
pub mod conn_guard;
pub mod crypto;
pub mod db;
pub mod disease;
pub mod domain;
pub mod error;
pub mod item;
pub mod item_category;
pub mod item_unit;
pub mod lab;
pub mod lab_visit;
pub mod nationality;
pub mod password;
pub mod patient;
pub mod patient_group;
pub mod prename;
pub mod registration;
pub mod sql_console;
pub mod state;
pub mod stock;
pub mod token;
pub mod turnstile;
pub mod users;
