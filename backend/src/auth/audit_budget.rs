//! เพดานจำนวนแถว audit ของ login ที่ล้มเหลวด้วย **ชื่อที่ไม่มีในระบบ** (docs/auth-design.md ข้อ 9)
//!
//! ชื่อที่ไม่มีในระบบสร้างได้ไม่จำกัด (เปลี่ยนชื่อทุกครั้งก็รีเซ็ตตัวนับของชื่อได้) แต่ละครั้งเขียน audit_log ถาวรราว 1 KB
//! เพดานนี้ทำให้ผู้ไม่ระบุตัวตนเติมตารางได้ไม่เกินอัตราที่ตั้ง — เกินแล้วไม่เขียนแถว audit (ตัวนับใน `login_throttle` ยังทำงานครบ
//! และ login ล้มเหลวของชื่อที่มีจริงบันทึกครบทุกครั้งเสมอ เพราะไม่ผ่านที่นี่)
//!
//! เก็บในหน่วยความจำของโปรเซส (หน้าต่างคงที่) ไม่แตะฐานข้อมูล — backend รันตัวเดียวตามดีไซน์ และรีสตาร์ทแล้วเริ่มหน้าต่างใหม่ได้

use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, Duration, Utc};

/// แถว audit ของชื่อที่ไม่มีในระบบที่บันทึกได้ต่อหน้าต่าง (1 นาที)
pub const UNKNOWN_NAME_AUDIT_PER_MINUTE: usize = 60;

/// ผลของการขอบันทึก audit หนึ่งแถว
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Take {
    /// บันทึกได้
    Recorded,
    /// เกินเพดานเป็นครั้งแรกของหน้าต่างนี้ — ไม่บันทึก และผู้เรียกควรแจ้งใน log หนึ่งครั้ง
    FirstSuppressed,
    /// เกินเพดาน (ไม่ใช่ครั้งแรก) — ไม่บันทึก
    Suppressed,
}

impl Take {
    pub fn is_recorded(self) -> bool {
        self == Take::Recorded
    }
}

#[derive(Debug)]
struct Window {
    started: Option<DateTime<Utc>>,
    used: usize,
    suppressed: usize,
}

#[derive(Debug)]
pub struct AuditBudget {
    per_window: usize,
    window: Duration,
    state: Mutex<Window>,
}

impl AuditBudget {
    pub fn new(per_window: usize, window: Duration) -> Self {
        Self {
            per_window,
            window,
            state: Mutex::new(Window {
                started: None,
                used: 0,
                suppressed: 0,
            }),
        }
    }

    /// ขอบันทึกหนึ่งแถว ณ เวลา `now` (เวลาเป็นพารามิเตอร์ — docs/rust-guidelines.md ข้อ 14)
    pub fn try_take(&self, now: DateTime<Utc>) -> Take {
        // lock ถูกถือสั้น ๆ โดยไม่มี await และไม่ panic ในช่วงที่ถือ — ถ้า poisoned ก็ใช้ค่าเดิมต่อได้
        let mut window = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let expired = match window.started {
            Some(started) => now < started || now - started >= self.window,
            None => true,
        };
        if expired {
            *window = Window {
                started: Some(now),
                used: 0,
                suppressed: 0,
            };
        }
        if window.used < self.per_window {
            window.used += 1;
            return Take::Recorded;
        }
        window.suppressed += 1;
        if window.suppressed == 1 {
            Take::FirstSuppressed
        } else {
            Take::Suppressed
        }
    }
}

impl Default for AuditBudget {
    fn default() -> Self {
        Self::new(UNKNOWN_NAME_AUDIT_PER_MINUTE, Duration::minutes(1))
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_800_000_000 + seconds, 0).unwrap()
    }

    #[test]
    fn allows_the_limit_then_suppresses_and_flags_the_first_suppression() {
        let budget = AuditBudget::new(3, Duration::minutes(1));
        let results: Vec<_> = (0..6).map(|i| budget.try_take(at(i))).collect();
        assert_eq!(
            results,
            vec![
                Take::Recorded,
                Take::Recorded,
                Take::Recorded,
                Take::FirstSuppressed,
                Take::Suppressed,
                Take::Suppressed,
            ]
        );
    }

    #[test]
    fn a_new_window_starts_fresh() {
        let budget = AuditBudget::new(1, Duration::minutes(1));
        assert!(budget.try_take(at(0)).is_recorded());
        assert!(!budget.try_take(at(30)).is_recorded());
        assert!(!budget.try_take(at(59)).is_recorded());
        assert!(budget.try_take(at(60)).is_recorded());
        // หน้าต่างใหม่ แจ้งการตัดครั้งแรกอีกครั้ง
        assert_eq!(budget.try_take(at(61)), Take::FirstSuppressed);
    }

    #[test]
    fn a_clock_that_moves_backwards_does_not_lock_the_budget() {
        let budget = AuditBudget::new(1, Duration::minutes(1));
        assert!(budget.try_take(at(100)).is_recorded());
        assert!(!budget.try_take(at(110)).is_recorded());
        assert!(budget.try_take(at(0)).is_recorded());
    }

    #[test]
    fn default_is_sixty_per_minute() {
        let budget = AuditBudget::default();
        let recorded = (0..100)
            .filter(|_| budget.try_take(at(0)).is_recorded())
            .count();
        assert_eq!(recorded, UNKNOWN_NAME_AUDIT_PER_MINUTE);
    }
}
