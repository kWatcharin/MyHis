//! รูปแบบ JSON เข้า/ออกของโมดูล audit — ไม่ส่ง id int ของ audit_log ออกไป (แถวประวัติไม่มี uuid และหน้าเว็บไม่ต้องใช้)

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// `?from=YYYY-MM-DD&to=YYYY-MM-DD` วันตามเวลาไทย รวมทั้งสองวัน (ทุกหน้าใต้ "ประวัติ")
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DateRangeQuery {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventUserResponse {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthEventResponse {
    pub occurred_at: DateTime<Utc>,
    /// login_succeeded | login_failed | logout | logout_all
    pub event: String,
    /// null = ไม่พบชื่อผู้ใช้ (login ด้วยชื่อที่ไม่มีในระบบ — ไม่ส่งชื่อที่พิมพ์มา)
    pub user: Option<EventUserResponse>,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthEventsResponse {
    pub events: Vec<AuthEventResponse>,
    /// มีเกินจำนวนสูงสุด — ส่งแค่ใหม่สุด ให้หน้าเว็บบอกให้เลือกช่วงวันให้แคบลง
    pub truncated: bool,
}

/// การใช้งานหนึ่งแถว — ไม่มี target_id/metadata ดิบ มีแค่ชื่อรายการที่ backend เลือกให้ (แถวผู้ป่วย = null เสมอ)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityResponse {
    pub occurred_at: DateTime<Utc>,
    /// เช่น disease_created, patient_updated, lab_visit_group_exported (ดู docs/auth-design.md)
    pub action: String,
    /// null = ผู้ใช้ที่ทำถูกลบไปแล้ว (ปกติไม่เกิดเพราะผู้ใช้ไม่มีการลบจริง)
    pub user: Option<EventUserResponse>,
    pub target_type: Option<String>,
    pub target_label: Option<String>,
    /// เฉพาะการส่งออก: cd4 | roster
    pub export_document: Option<String>,
    /// เฉพาะการส่งออก: excel | pdf
    pub export_format: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitiesResponse {
    pub events: Vec<ActivityResponse>,
    /// มีเกินจำนวนสูงสุด — ส่งแค่ใหม่สุด ให้หน้าเว็บบอกให้เลือกช่วงวันให้แคบลง
    pub truncated: bool,
}
