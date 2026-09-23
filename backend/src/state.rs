//! AppState (pool, config) — Clone ที่ภายในเป็น Arc สำหรับข้อมูลใหญ่ (docs/rust-guidelines.md ข้อ 5)
//! PgPool โคลนได้ราคาถูกโดยออกแบบอยู่แล้ว ไม่ต้องห่อ Arc เพิ่ม

use std::sync::Arc;

use sqlx::PgPool;

use crate::config::Config;

#[derive(Debug, Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    /// PHC hash คงที่ที่คำนวณไว้ตอน startup ใช้ verify ตอน login ไม่พบ username เพื่อให้เวลาตอบ
    /// กลับใกล้เคียงกับกรณีรหัสผ่านผิดจริง (timing-safe — docs/auth-design.md ข้อ 6)
    pub dummy_password_hash: Arc<str>,
}
