//! เพดานจำนวนครั้งที่ login **สำเร็จ** ต่อผู้ใช้ (docs/auth-design.md ข้อ 9.3)
//!
//! เพดานต่อ IP (ข้อ 9.2) นับเฉพาะความพยายามที่ไม่สำเร็จ — login สำเร็จคืนโควตา จึงไม่มีอะไรจำกัดผู้ถือรหัสผ่านที่ถูกต้อง (หรือรหัสที่ถูกขโมย)
//! ที่วนล็อกอินรัว: แต่ละครั้งสร้าง session ใหม่ (อยู่ตาม idle TTL), เขียนแถว audit `login_succeeded` ถาวร (ไม่มี retention) และสั่ง
//! sweep session หมดอายุ — เพดานนี้จำกัดทั้งสามอย่างพร้อมกันที่ต้นทาง ไม่ต้องจำกัดแยก
//!
//! **จองก่อนสร้าง session** (หลังรหัสผ่านถูก) **ไม่คืนโควตา**: ทุกการเข้าสำเร็จที่ผ่านด่านนี้นับ จองหนึ่งครั้งแล้วเกินเพดานได้ไม่ได้
//! แม้ขนานกัน (ตรวจ+นับใน lock เดียว) เกินเพดานแล้ว login ที่รหัสถูกตอบ 429 (ไม่ใช่ 401) — ดูข้อควรรู้ใน auth-design.md
//!
//! เก็บในหน่วยความจำของโปรเซส (หน้าต่างคงที่ต่อผู้ใช้) ไม่แตะฐานข้อมูล — backend รันตัวเดียวตามดีไซน์ รีสตาร์ทแล้วเริ่มใหม่ได้
//! ตารางมีขนาดไม่เกินจำนวนผู้ใช้ที่เข้าสำเร็จในหน้าต่างเดียว (ระบบครอบครัว/ทีมเล็ก) เต็ม `MAX_TRACKED_USERS` แล้วไม่ติดตามเพิ่ม (เตือนใน log)

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, Duration, Utc};

use super::ip_budget::Blocked;
use crate::domain::UserId;

/// login สำเร็จต่อผู้ใช้ต่อหน้าต่าง — ผู้ใช้จริงเข้าหลายเครื่อง/เปิดหลายแท็บไม่ถึงสิบครั้งใน 10 นาที
pub const LOGINS_PER_USER: u32 = 30;

/// จำนวนผู้ใช้ที่ติดตามพร้อมกันสูงสุด (เกินนี้ไม่ติดตามผู้ใช้ใหม่ — ระบบนี้มีผู้ใช้ไม่กี่คน จึงไม่เกิดจริง)
const MAX_TRACKED_USERS: usize = 10_000;

fn default_window() -> Duration {
    Duration::minutes(10)
}

#[derive(Debug)]
struct Entry {
    started: DateTime<Utc>,
    used: u32,
    /// แจ้งใน log แล้วว่าผู้ใช้นี้ชนเพดานในหน้าต่างนี้ (ครั้งเดียวต่อหน้าต่าง)
    warned: bool,
}

#[derive(Debug, Default)]
struct State {
    entries: HashMap<UserId, Entry>,
    full_warned: bool,
}

#[derive(Debug)]
pub struct UserBudget {
    max: u32,
    window: Duration,
    max_tracked: usize,
    state: Mutex<State>,
}

impl UserBudget {
    pub fn new(max: u32, window: Duration, max_tracked: usize) -> Self {
        Self {
            max,
            window,
            max_tracked,
            state: Mutex::new(State::default()),
        }
    }

    fn expired(&self, started: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        // นาฬิกาถอยหลังถือว่าหมดอายุ (เริ่มหน้าต่างใหม่) แทนที่จะล็อกผู้ใช้ค้าง
        now < started || now - started >= self.window
    }

    /// จองการเข้าสำเร็จหนึ่งครั้งของ `user` ณ เวลา `now` (เวลาเป็นพารามิเตอร์ — docs/rust-guidelines.md ข้อ 14)
    /// เกินเพดาน → `Blocked` (ลองใหม่ได้เมื่อหน้าต่างของผู้ใช้นี้หมด)
    pub fn reserve(&self, user: UserId, now: DateTime<Utc>) -> Result<(), Blocked> {
        // lock ถูกถือสั้น ๆ โดยไม่มี await และไม่ panic ในช่วงที่ถือ — ถ้า poisoned ก็ใช้ค่าเดิมต่อได้
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);

        if !state.entries.contains_key(&user) && state.entries.len() >= self.max_tracked {
            state
                .entries
                .retain(|_, entry| !self.expired(entry.started, now));
            if state.entries.len() >= self.max_tracked {
                if !state.full_warned {
                    state.full_warned = true;
                    tracing::warn!(
                        tracked = state.entries.len(),
                        "login success budget table is full: new accounts are not limited until entries expire"
                    );
                }
                return Ok(());
            }
            state.full_warned = false;
        }

        let entry = state.entries.entry(user).or_insert(Entry {
            started: now,
            used: 0,
            warned: false,
        });
        if self.expired(entry.started, now) {
            *entry = Entry {
                started: now,
                used: 0,
                warned: false,
            };
        }
        if entry.used >= self.max {
            let first = !entry.warned;
            entry.warned = true;
            return Err(Blocked {
                until: entry.started + self.window,
                first,
            });
        }
        entry.used += 1;
        Ok(())
    }
}

impl Default for UserBudget {
    fn default() -> Self {
        Self::new(LOGINS_PER_USER, default_window(), MAX_TRACKED_USERS)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use chrono::TimeZone;

    use super::*;

    fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_800_000_000 + seconds, 0).unwrap()
    }

    fn budget(max: u32) -> UserBudget {
        UserBudget::new(max, Duration::minutes(10), 100)
    }

    const NURSE: UserId = UserId(1);
    const ADMIN: UserId = UserId(2);

    #[test]
    fn allows_the_limit_then_blocks_until_the_window_ends() {
        let budget = budget(3);
        for second in 0..3 {
            assert!(budget.reserve(NURSE, at(second)).is_ok());
        }
        let blocked = budget.reserve(NURSE, at(10)).unwrap_err();
        assert_eq!(blocked.until, at(0) + Duration::minutes(10));
        assert!(blocked.first);
        // ครั้งถัดไปในหน้าต่างเดียวกันไม่ใช่ครั้งแรกอีกแล้ว (ผู้เรียกแจ้ง log แค่ครั้งเดียว)
        assert!(!budget.reserve(NURSE, at(20)).unwrap_err().first);
        // หน้าต่างใหม่เริ่มใหม่
        assert!(budget.reserve(NURSE, at(600)).is_ok());
    }

    #[test]
    fn other_users_are_not_affected() {
        let budget = budget(1);
        assert!(budget.reserve(NURSE, at(0)).is_ok());
        assert!(budget.reserve(NURSE, at(1)).is_err());
        assert!(budget.reserve(ADMIN, at(1)).is_ok());
    }

    #[test]
    fn a_clock_that_moves_backwards_does_not_lock_a_user() {
        let budget = budget(1);
        assert!(budget.reserve(NURSE, at(100)).is_ok());
        assert!(budget.reserve(NURSE, at(110)).is_err());
        assert!(budget.reserve(NURSE, at(0)).is_ok());
    }

    #[test]
    fn a_full_table_sweeps_expired_entries_then_stops_tracking_new_users() {
        let budget = UserBudget::new(1, Duration::minutes(10), 2);
        assert!(budget.reserve(UserId(1), at(0)).is_ok());
        assert!(budget.reserve(UserId(2), at(1)).is_ok());
        // ตารางเต็มด้วยแถวที่ยังไม่หมดอายุ: ผู้ใช้ใหม่ไม่ถูกติดตาม (ระบบนี้ไม่มีผู้ใช้ถึง 10,000 — กันตารางโตเท่านั้น)
        assert!(budget.reserve(UserId(3), at(2)).is_ok());
        assert!(budget.reserve(UserId(3), at(3)).is_ok());
        assert!(budget.reserve(UserId(1), at(4)).is_err());
        // แถวเก่าหมดอายุแล้ว → กวาดทิ้ง ผู้ใช้ใหม่ถูกติดตามตามปกติ
        assert!(budget.reserve(UserId(4), at(700)).is_ok());
        assert!(budget.reserve(UserId(4), at(701)).is_err());
    }

    #[test]
    fn concurrent_reservations_never_exceed_the_limit() {
        let budget = Arc::new(budget(20));
        let granted: usize = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..8)
                .map(|_| {
                    let budget = Arc::clone(&budget);
                    scope.spawn(move || {
                        (0..25)
                            .filter(|_| budget.reserve(NURSE, at(0)).is_ok())
                            .count()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).sum()
        });
        assert_eq!(granted, 20);
    }

    #[test]
    fn default_is_thirty_per_ten_minutes() {
        let budget = UserBudget::default();
        let granted = (0..50)
            .filter(|_| budget.reserve(NURSE, at(0)).is_ok())
            .count();
        assert_eq!(granted, LOGINS_PER_USER as usize);
        let blocked = budget.reserve(NURSE, at(1)).unwrap_err();
        assert_eq!(blocked.until, at(0) + Duration::minutes(10));
    }
}
