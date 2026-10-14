//! query SQLx ต่อ users, roles, role_permissions, user_roles, pages, page_permissions, registration_links
//! (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! เพื่อตรวจตอน compile ห้ามประกอบ SQL ด้วย format!
//! (docs/rust-guidelines.md ข้อ 11) แยกไฟล์ตาม entity (users/roles/pages/registration_links) เพราะรวมไฟล์เดียว
//! ยาวเกิน 300 บรรทัด (docs/rust-guidelines.md ข้อ 7)
//!
//! หมายเหตุ: ฟังก์ชันสร้างผู้ใช้/ลบ session ในนี้คล้ายกับใน `auth::repository` โดยตั้งใจ — โมดูล
//! ฟีเจอร์ไม่เรียกกันเอง (docs/rust-guidelines.md) เลยต้องมี query ของตัวเองแยกจาก auth แม้จะซ้ำ
//! กันบางส่วน
//!
//! การเขียนบัญชีผู้ใช้ทุกครั้ง (`users.rs`) เขียน `audit_log` ในทรานแซกชันเดียวกัน (ตรงจากที่นี่ เหมือน
//! lab_visit/patient) target_id เป็น uuid ของผู้ใช้ ไม่มี metadata — role/page ก็เขียนเหมือนกันแต่เก็บชื่อไว้ใน
//! `metadata.label` ด้วย (`insert_labeled_audit`) เพราะสองตารางนี้ลบจริง

mod pages;
mod registration_links;
mod roles;
mod users;

pub(crate) use pages::*;
pub(crate) use registration_links::NewRegistrationLink;
pub(crate) use roles::*;
pub(crate) use users::*;

use serde_json::Value;
use sqlx::{Postgres, Transaction};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::UserId;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("unknown permission stored in database: {0}")]
    UnknownPermission(String),
}

impl RepositoryError {
    /// เช็คว่า error เกิดจาก unique constraint ชนไหม (username/role name/page route ซ้ำ) —
    /// ให้ service ชั้นบนแปลงเป็น Conflict แทนที่จะ pre-check แข่งกับ concurrent request เอง
    pub fn is_unique_violation(&self) -> bool {
        matches!(self, RepositoryError::Database(e) if e.as_database_error().is_some_and(|d| d.is_unique_violation()))
    }
}

/// audit ของ role/เพจ (หน้า "ประวัติการใช้งาน") — สองตารางนี้ลบจริง จึงเก็บชื่อ ณ ตอนนั้นไว้ใน
/// `metadata = {"label": ...}` ให้ประวัติยังแสดงชื่อได้หลังแถวหายไป (ชื่อ role/เพจ ไม่ใช่ความลับหรือข้อมูลผู้ป่วย)
/// `details` (ค่าที่เพิ่ม / ค่าเดิม→ใหม่ จาก `domain::changes`) รวมเข้า metadata เดียวกัน
pub(super) struct LabeledAudit<'a> {
    pub action: &'a str,
    pub target_type: &'a str,
    pub target_public_id: Uuid,
    pub label: &'a str,
    pub details: Option<Value>,
}

async fn insert_labeled_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    entry: LabeledAudit<'_>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id, metadata)
         values ($1, $2, $3, $4, jsonb_build_object('label', $5::text) || coalesce($6::jsonb, '{}'::jsonb))",
        actor.0,
        entry.action,
        entry.target_type,
        entry.target_public_id.to_string(),
        entry.label,
        entry.details
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
