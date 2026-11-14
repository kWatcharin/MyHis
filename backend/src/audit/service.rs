//! กฎของหน้าประวัติ (การเข้าออก + การใช้งาน): สิทธิ์ (`audit.view`), ช่วงวันที่ (ตามเวลาไทย ไม่เกิน 366 วัน),
//! จำนวนแถวสูงสุด
//! ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 3)

use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};
use sqlx::PgPool;

use crate::audit::repository::{self, ActivityRow, AuthEventRow, RepositoryError};
use crate::auth::CurrentUser;
use crate::domain::{Action, Resource};

/// แถวสูงสุดต่อครั้ง — เกินแล้วตอบ truncated ให้หน้าเว็บบอกผู้ใช้เลือกช่วงวันให้แคบลง
pub const MAX_EVENTS: usize = 5000;
const MAX_RANGE_DAYS: i64 = 366;
/// เวลาไทย UTC+7 ตลอดปี (ไม่มี daylight saving) — วันที่ในตัวกรองหมายถึงวันตามเวลาไทย
const BANGKOK_OFFSET_HOURS: i64 = 7;

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

pub struct AuthEvents {
    pub events: Vec<AuthEventRow>,
    /// มีเหตุการณ์ในช่วงนี้มากกว่า MAX_EVENTS (ส่งแค่ใหม่สุด MAX_EVENTS แถว)
    pub truncated: bool,
}

pub struct Activities {
    pub events: Vec<ActivityRow>,
    /// มีรายการในช่วงนี้มากกว่า MAX_EVENTS (ส่งแค่ใหม่สุด MAX_EVENTS แถว)
    pub truncated: bool,
}

/// เที่ยงคืนตามเวลาไทยของวันนั้น เป็นเวลา UTC
fn bangkok_midnight_utc(date: NaiveDate) -> DateTime<Utc> {
    date.and_time(NaiveTime::MIN).and_utc() - Duration::hours(BANGKOK_OFFSET_HOURS)
}

/// ตรวจสิทธิ์ + ช่วงวัน แล้วคืนช่วงเวลา UTC [เที่ยงคืนไทยของ `from`, เที่ยงคืนไทยของวันถัดจาก `to`)
/// ใช้ร่วมกันทุกหน้าใต้ "ประวัติ" (สิทธิ์ `audit.view` ตัวเดียว)
fn checked_range(
    actor: &CurrentUser,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<(DateTime<Utc>, DateTime<Utc>), AuditError> {
    if !actor.can(Resource::Audit, Action::View) {
        return Err(AuditError::Forbidden);
    }
    if from > to {
        return Err(AuditError::Validation(
            "from must not be after to".to_string(),
        ));
    }
    if (to - from).num_days() + 1 > MAX_RANGE_DAYS {
        return Err(AuditError::Validation(
            "date range must not exceed 366 days".to_string(),
        ));
    }
    let day_after = to
        .checked_add_signed(Duration::days(1))
        .ok_or_else(|| AuditError::Validation("invalid date".to_string()))?;
    Ok((bangkok_midnight_utc(from), bangkok_midnight_utc(day_after)))
}

/// ขอเกิน MAX_EVENTS หนึ่งแถวเพื่อรู้ว่ามีมากกว่านั้นไหม
fn fetch_limit() -> i64 {
    i64::try_from(MAX_EVENTS + 1).unwrap_or(i64::MAX)
}

/// ตัดเหลือ MAX_EVENTS แถว — คืน true ถ้ามีมากกว่านั้น
fn cap<T>(rows: &mut Vec<T>) -> bool {
    let truncated = rows.len() > MAX_EVENTS;
    rows.truncate(MAX_EVENTS);
    truncated
}

/// ประวัติการเข้าออกของผู้ใช้ทุกคนตั้งแต่ `from` ถึง `to` (รวมทั้งสองวัน ตามเวลาไทย)
pub async fn list_auth_events(
    pool: &PgPool,
    actor: &CurrentUser,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<AuthEvents, AuditError> {
    let (since, until) = checked_range(actor, from, to)?;
    let mut events = repository::list_auth_events(pool, since, until, fetch_limit()).await?;
    let truncated = cap(&mut events);
    Ok(AuthEvents { events, truncated })
}

/// ประวัติการใช้งาน (เพิ่ม/แก้ไข/ลบ/ส่งออก ฯลฯ ทุกหน้า ยกเว้นเข้า/ออกระบบ) ตั้งแต่ `from` ถึง `to` ตามเวลาไทย
pub async fn list_activities(
    pool: &PgPool,
    actor: &CurrentUser,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Activities, AuditError> {
    let (since, until) = checked_range(actor, from, to)?;
    let mut events = repository::list_activities(pool, since, until, fetch_limit()).await?;
    let truncated = cap(&mut events);
    Ok(Activities { events, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bangkok_day_starts_at_17_utc_the_day_before() {
        let date = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
        assert_eq!(
            bangkok_midnight_utc(date).to_rfc3339(),
            "2026-10-11T17:00:00+00:00"
        );
    }
}
