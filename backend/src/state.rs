//! AppState (pool, config) — Clone ที่ภายในเป็น Arc สำหรับข้อมูลใหญ่ (docs/rust-guidelines.md ข้อ 5)
//! PgPool โคลนได้ราคาถูกโดยออกแบบอยู่แล้ว ไม่ต้องห่อ Arc เพิ่ม

use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::{AuditBudget, IpBudget, UserBudget};
use crate::config::Config;
use crate::sql_console::SqlConsole;
use crate::turnstile::Turnstile;

#[derive(Debug, Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    /// PHC hash คงที่ที่คำนวณไว้ตอน startup ใช้ verify ตอน login ไม่พบ username เพื่อให้เวลาตอบ
    /// กลับใกล้เคียงกับกรณีรหัสผ่านผิดจริง (timing-safe — docs/auth-design.md ข้อ 6)
    pub dummy_password_hash: Arc<str>,
    /// ตัวตรวจ Cloudflare Turnstile ตอน login — None = ปิด (ไม่ได้ตั้ง key, เทสส่วนใหญ่)
    pub turnstile: Option<Arc<Turnstile>>,
    /// หน้า SQL (connection ของ role อ่านอย่างเดียวที่แยกจาก pool หลัก) — None = ปิด (ไม่ได้ตั้งรหัสของ role)
    pub sql_console: Option<Arc<SqlConsole>>,
    /// เพดานแถว audit ของ login ที่ล้มเหลวด้วยชื่อที่ไม่มีในระบบ (กัน audit_log โตจากชื่อสุ่ม — auth/audit_budget.rs)
    pub unknown_login_audit: Arc<AuditBudget>,
    /// เพดาน login ที่ไม่สำเร็จต่อ IP (กันเปลี่ยนชื่อรัวเพื่อเลี่ยงตัวนับต่อชื่อ — auth/ip_budget.rs)
    pub login_ip_budget: Arc<IpBudget>,
    /// เพดาน login ที่ **สำเร็จ** ต่อผู้ใช้ (กัน session/audit_log สะสมจากการวนล็อกอิน — auth/user_budget.rs)
    pub login_success_budget: Arc<UserBudget>,
}
