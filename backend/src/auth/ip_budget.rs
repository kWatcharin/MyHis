//! เพดานจำนวนครั้ง login ที่ไม่สำเร็จต่อ IP (docs/auth-design.md ข้อ 9.2)
//!
//! ตัวนับต่อชื่อผู้ใช้ (ข้อ 9) กันการเดารหัสของบัญชีหนึ่ง แต่ผู้โจมตีเปลี่ยนชื่อทุกครั้งได้ — แต่ละครั้งยังเสียคิว argon2 และ
//! เรียก Turnstile เพดานนี้นับตามที่อยู่ที่ proxy เห็น ไม่ขึ้นกับว่าชื่อมีในระบบ จึงไม่เป็น oracle ของชื่อผู้ใช้
//!
//! **จองก่อนทำงาน** (`reserve`) ไม่ใช่เช็กแล้วค่อยนับ: คำขอพร้อมกันจาก IP เดียวผ่านด่านได้ไม่เกินเพดาน ไม่ว่าจะค้างอยู่ในคิวนานแค่ไหน
//! คำขอที่สำเร็จหรือล้มเหลวเพราะฝั่งเราเอง (503) คืนโควตา (`refund`) ส่วนคำขอที่ถูกยกเลิกกลางทาง (client ตัด connection) **ไม่คืน**
//! — ไม่งั้นส่งรหัสผิดแล้วตัดทิ้งเพื่อให้ argon2 ทำงานต่อโดยไม่ถูกนับได้
//!
//! เก็บในหน่วยความจำของโปรเซส (หน้าต่างคงที่ต่อ IP) ไม่แตะฐานข้อมูล — backend รันตัวเดียวตามดีไซน์ รีสตาร์ทแล้วเริ่มใหม่ได้
//!
//! **คีย์ต้องแพงกว่าตาราง:** IPv6 นับทั้ง /56 (ไม่ใช่ /64 — ผู้ถือ prefix /48 มี /64 ถึง 65,536 ก้อน แต่มี /56 แค่ 256 ก้อน) และเมื่อตาราง
//! เต็มด้วยแถวที่ยังไม่หมดอายุ ที่อยู่ใหม่ใช้ **ถังรวม** ถังเดียว (`OVERFLOW_PER_WINDOW`) แทนการปล่อยไม่จำกัด — ผู้โจมตีที่ดันตารางให้เต็ม
//! จึงไม่ได้ใบอนุญาตไม่จำกัด (ราคาที่จ่าย: ระหว่างถูกโจมตี ที่อยู่ใหม่ของคนจริงอาจชนถังรวมได้ ซึ่งยังเข้าใหม่ได้เมื่อหน้าต่างหมด)

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, PoisonError};

use chrono::{DateTime, Duration, Utc};

/// จำนวนครั้งที่ไม่สำเร็จต่อ IP ต่อหน้าต่าง
pub const LOGIN_FAILURES_PER_IP: u32 = 20;

/// จำนวน IP ที่ติดตามพร้อมกันสูงสุด (หน่วยความจำ ~ไม่กี่ร้อย KB) เต็มด้วยแถวที่ยังไม่หมดอายุ → ที่อยู่ใหม่ใช้ถังรวม (`OVERFLOW_PER_WINDOW`)
const MAX_TRACKED_IPS: usize = 10_000;

/// ที่อยู่ใหม่ทั้งหมดรวมกันได้กี่ครั้งต่อหน้าต่างขณะตารางเต็ม (~0.5 ครั้ง/วินาที — ต่ำกว่าที่ argon2 ของ login รับได้)
const OVERFLOW_PER_WINDOW: u32 = 300;

fn default_window() -> Duration {
    Duration::minutes(10)
}

/// IPv4 ตรง ๆ, IPv6 นับทั้ง /56 (ผู้ใช้ IPv6 เปลี่ยนที่อยู่ภายใน /64 ได้เอง และ ISP ปล่อยให้ลูกค้า /56–/48 — ถ้านับแค่ /64
/// ผู้ถือ /48 มี key 65,536 ก้อน), `::ffff:a.b.c.d` นับเป็น IPv4
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum IpKey {
    V4(u32),
    /// 56 บิตบนของ IPv6
    V6(u64),
}

impl IpKey {
    fn from_ip(ip: IpAddr) -> Self {
        match ip {
            IpAddr::V4(v4) => IpKey::V4(u32::from(v4)),
            IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
                Some(v4) => IpKey::V4(u32::from(v4)),
                None => IpKey::V6((u128::from(v6) >> 72) as u64),
            },
        }
    }
}

/// ที่อยู่ที่ไม่ใช่ที่อยู่สาธารณะ (loopback / private / link-local / unspecified) — ถ้า budget เห็นแบบนี้ในเครื่องจริง แปลว่า
/// proxy chain ส่งที่อยู่ของ container/gateway มาแทนผู้ใช้ (ทุกคนจะใช้ key เดียวกัน) ผู้เรียกควรเตือนใน log หนึ่งครั้ง
fn is_non_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
        }
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_non_public(IpAddr::V4(v4)),
            None => {
                let first = v6.segments()[0];
                v6.is_loopback()
                    || v6.is_unspecified()
                    // fc00::/7 (unique local) และ fe80::/10 (link-local)
                    || (first & 0xfe00) == 0xfc00
                    || (first & 0xffc0) == 0xfe80
            }
        },
    }
}

#[derive(Debug)]
struct Entry {
    started: DateTime<Utc>,
    used: u32,
    /// แจ้งใน log แล้วว่า IP นี้ชนเพดานในหน้าต่างนี้ (ครั้งเดียวต่อหน้าต่าง)
    warned: bool,
}

#[derive(Debug, Default)]
struct State {
    entries: HashMap<IpKey, Entry>,
    /// ถังรวมของที่อยู่ใหม่ขณะตารางเต็ม (หน้าต่างเดียวกับ key ปกติ แต่เพดานคือ `overflow_max`)
    overflow: Option<Entry>,
    /// แจ้งใน log แล้วว่าตารางเต็ม (เคลียร์เมื่อกวาดแถวหมดอายุได้)
    full_warned: bool,
    /// แจ้งใน log แล้วว่าเห็นที่อยู่ที่ไม่ใช่ public (ครั้งเดียวต่อโปรเซส — สัญญาณว่า proxy chain ผิด)
    non_public_warned: bool,
}

/// IP ชนเพดาน — ปฏิเสธก่อนเริ่มงาน
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocked {
    /// ลองใหม่ได้เมื่อหน้าต่างของ IP นี้หมด
    pub until: DateTime<Utc>,
    /// ครั้งแรกของหน้าต่างที่ถูกปฏิเสธ — ผู้เรียกควรแจ้งใน log หนึ่งครั้ง
    pub first: bool,
}

/// ใบจองเป็นของถังไหน
#[derive(Debug, Clone, Copy)]
enum TicketKey {
    /// ไม่มีที่อยู่ — ไม่นับ
    None,
    Ip(IpKey),
    /// ถังรวมของที่อยู่ใหม่ขณะตารางเต็ม
    Overflow,
}

/// ใบจองหนึ่งครั้ง — คืนโควตาด้วย [`IpBudget::refund`] เท่านั้น (ทิ้งเฉย ๆ = นับเป็นความพยายามที่ไม่สำเร็จ)
#[derive(Debug)]
#[must_use = "a dropped ticket counts as a failed attempt; call IpBudget::refund when the attempt did not fail"]
pub struct Ticket {
    key: TicketKey,
    started: DateTime<Utc>,
}

#[derive(Debug)]
pub struct IpBudget {
    max: u32,
    window: Duration,
    max_tracked: usize,
    overflow_max: u32,
    state: Mutex<State>,
}

impl IpBudget {
    pub fn new(max: u32, window: Duration, max_tracked: usize) -> Self {
        Self::with_overflow(max, window, max_tracked, OVERFLOW_PER_WINDOW)
    }

    /// เหมือน [`IpBudget::new`] แต่กำหนดเพดานของถังรวมเอง (เทสที่ต้องการตารางเล็กกับถังรวมเล็ก)
    pub fn with_overflow(
        max: u32,
        window: Duration,
        max_tracked: usize,
        overflow_max: u32,
    ) -> Self {
        Self {
            max,
            window,
            max_tracked,
            overflow_max,
            state: Mutex::new(State::default()),
        }
    }

    fn expired(&self, started: DateTime<Utc>, now: DateTime<Utc>) -> bool {
        // นาฬิกาถอยหลังถือว่าหมดอายุ (เริ่มหน้าต่างใหม่) แทนที่จะล็อก IP ค้าง
        now < started || now - started >= self.window
    }

    /// จองหนึ่งครั้งก่อนเริ่ม login ณ เวลา `now` (เวลาเป็นพารามิเตอร์ — docs/rust-guidelines.md ข้อ 14)
    /// ไม่มี IP (`None` — ไม่ได้มาทาง proxy) = ไม่นับ
    pub fn reserve(&self, ip: Option<IpAddr>, now: DateTime<Utc>) -> Result<Ticket, Blocked> {
        let Some(ip) = ip else {
            return Ok(Ticket::untracked(now));
        };
        let key = IpKey::from_ip(ip);

        // lock ถูกถือสั้น ๆ โดยไม่มี await และไม่ panic ในช่วงที่ถือ — ถ้า poisoned ก็ใช้ค่าเดิมต่อได้
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);

        if !state.entries.contains_key(&key) {
            if is_non_public(ip) && !state.non_public_warned {
                state.non_public_warned = true;
                tracing::warn!(
                    %ip,
                    "login IP budget sees a non-public client address: check the proxy chain (docs/infrastructure.md) — every client behind it would share this key"
                );
            }
            if state.entries.len() >= self.max_tracked {
                state
                    .entries
                    .retain(|_, entry| !self.expired(entry.started, now));
                if state.entries.len() >= self.max_tracked {
                    if !state.full_warned {
                        state.full_warned = true;
                        tracing::warn!(
                            tracked = state.entries.len(),
                            overflow_limit = self.overflow_max,
                            "login IP budget table is full: new addresses share one overflow budget until entries expire"
                        );
                    }
                    return self.reserve_overflow(&mut state, now);
                }
                state.full_warned = false;
            }
        }

        let entry = state.entries.entry(key).or_insert(Entry {
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
        Ok(Ticket {
            key: TicketKey::Ip(key),
            started: entry.started,
        })
    }

    /// ตารางเต็ม: ที่อยู่ใหม่ทั้งหมดใช้ถังรวมถังเดียว (หน้าต่างเริ่มเมื่อมีคนใช้ครั้งแรก และเริ่มใหม่เมื่อหมด)
    fn reserve_overflow(&self, state: &mut State, now: DateTime<Utc>) -> Result<Ticket, Blocked> {
        let entry = state.overflow.get_or_insert(Entry {
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
        if entry.used >= self.overflow_max {
            let first = !entry.warned;
            entry.warned = true;
            return Err(Blocked {
                until: entry.started + self.window,
                first,
            });
        }
        entry.used += 1;
        Ok(Ticket {
            key: TicketKey::Overflow,
            started: entry.started,
        })
    }

    /// คืนโควตาของใบจอง — เฉพาะเมื่อหน้าต่างของถังนี้ยังเป็นอันเดิมที่จองไว้ (หน้าต่างใหม่ไม่ถูกลดตามใบเก่า)
    pub fn refund(&self, ticket: Ticket) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let entry = match ticket.key {
            TicketKey::None => return,
            TicketKey::Ip(key) => state.entries.get_mut(&key),
            TicketKey::Overflow => state.overflow.as_mut(),
        };
        if let Some(entry) = entry {
            if entry.started == ticket.started {
                entry.used = entry.used.saturating_sub(1);
            }
        }
    }
}

impl Ticket {
    fn untracked(now: DateTime<Utc>) -> Self {
        Self {
            key: TicketKey::None,
            started: now,
        }
    }
}

impl Default for IpBudget {
    fn default() -> Self {
        Self::new(LOGIN_FAILURES_PER_IP, default_window(), MAX_TRACKED_IPS)
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

    fn ip(text: &str) -> Option<IpAddr> {
        Some(text.parse().unwrap())
    }

    fn budget(max: u32) -> IpBudget {
        IpBudget::new(max, Duration::minutes(10), 100)
    }

    #[test]
    fn allows_the_limit_then_blocks_until_the_window_ends() {
        let budget = budget(3);
        for second in 0..3 {
            assert!(budget.reserve(ip("203.0.113.5"), at(second)).is_ok());
        }
        let blocked = budget.reserve(ip("203.0.113.5"), at(10)).unwrap_err();
        assert_eq!(blocked.until, at(0) + Duration::minutes(10));
        assert!(blocked.first);
        // ครั้งถัดไปในหน้าต่างเดียวกันไม่ใช่ครั้งแรกอีกแล้ว (ผู้เรียกแจ้ง log แค่ครั้งเดียว)
        assert!(!budget.reserve(ip("203.0.113.5"), at(20)).unwrap_err().first);
    }

    #[test]
    fn a_new_window_starts_fresh() {
        let budget = budget(1);
        assert!(budget.reserve(ip("203.0.113.5"), at(0)).is_ok());
        assert!(budget.reserve(ip("203.0.113.5"), at(599)).is_err());
        assert!(budget.reserve(ip("203.0.113.5"), at(600)).is_ok());
        // หน้าต่างใหม่ แจ้งการปฏิเสธครั้งแรกอีกครั้ง
        assert!(
            budget
                .reserve(ip("203.0.113.5"), at(601))
                .unwrap_err()
                .first
        );
    }

    #[test]
    fn other_addresses_are_not_affected() {
        let budget = budget(1);
        assert!(budget.reserve(ip("203.0.113.5"), at(0)).is_ok());
        assert!(budget.reserve(ip("203.0.113.5"), at(1)).is_err());
        assert!(budget.reserve(ip("203.0.113.6"), at(1)).is_ok());
    }

    #[test]
    fn a_missing_address_is_never_limited() {
        let budget = budget(1);
        for second in 0..10 {
            assert!(budget.reserve(None, at(second)).is_ok());
        }
    }

    #[test]
    fn refund_returns_the_quota() {
        let budget = budget(1);
        let ticket = budget.reserve(ip("203.0.113.5"), at(0)).unwrap();
        assert!(budget.reserve(ip("203.0.113.5"), at(1)).is_err());
        budget.refund(ticket);
        assert!(budget.reserve(ip("203.0.113.5"), at(2)).is_ok());
    }

    #[test]
    fn a_dropped_ticket_still_counts() {
        let budget = budget(1);
        drop(budget.reserve(ip("203.0.113.5"), at(0)).unwrap());
        assert!(budget.reserve(ip("203.0.113.5"), at(1)).is_err());
    }

    #[test]
    fn a_stale_ticket_does_not_refund_a_newer_window() {
        let budget = budget(1);
        let old = budget.reserve(ip("203.0.113.5"), at(0)).unwrap();
        // หน้าต่างแรกหมด → หน้าต่างใหม่จองไว้หนึ่งครั้ง; ใบเก่าคืนมาแล้วต้องไม่ลดตัวนับของหน้าต่างใหม่
        let new = budget.reserve(ip("203.0.113.5"), at(700)).unwrap();
        budget.refund(old);
        assert!(budget.reserve(ip("203.0.113.5"), at(701)).is_err());
        budget.refund(new);
        assert!(budget.reserve(ip("203.0.113.5"), at(702)).is_ok());
    }

    #[test]
    fn refunds_restore_exactly_what_was_reserved() {
        let budget = budget(2);
        let ticket = budget.reserve(ip("203.0.113.5"), at(0)).unwrap();
        budget.refund(ticket);
        let a = budget.reserve(ip("203.0.113.5"), at(1)).unwrap();
        let b = budget.reserve(ip("203.0.113.5"), at(2)).unwrap();
        assert!(budget.reserve(ip("203.0.113.5"), at(3)).is_err());
        budget.refund(a);
        budget.refund(b);
        assert!(budget.reserve(ip("203.0.113.5"), at(4)).is_ok());
        assert!(budget.reserve(ip("203.0.113.5"), at(5)).is_ok());
        assert!(budget.reserve(ip("203.0.113.5"), at(6)).is_err());
    }

    #[test]
    fn ipv6_is_counted_per_slash_56_and_mapped_ipv4_as_ipv4() {
        let budget = budget(1);
        assert!(budget.reserve(ip("2001:db8:1:200::1"), at(0)).is_ok());
        // /64 อื่นใน /56 เดียวกัน (2001:db8:1:2xx::/56) ใช้โควตาเดียวกัน
        assert!(budget.reserve(ip("2001:db8:1:2ff::9"), at(1)).is_err());
        assert!(budget.reserve(ip("2001:db8:1:201::1"), at(1)).is_err());
        // ข้ามไป /56 ถัดไป (2001:db8:1:3xx) เป็นคนละ key
        assert!(budget.reserve(ip("2001:db8:1:300::1"), at(1)).is_ok());

        assert!(budget.reserve(ip("198.51.100.7"), at(2)).is_ok());
        assert!(budget.reserve(ip("::ffff:198.51.100.7"), at(3)).is_err());
    }

    #[test]
    fn a_routed_slash_48_has_at_most_256_keys() {
        // ผู้ถือ /48 มี /56 แค่ 256 ก้อน (เดิมนับ /64 = 65,536 ก้อน)
        let budget = IpBudget::new(1, Duration::minutes(10), 100_000);
        let granted = (0..65_536u32)
            .filter(|n| {
                // 2001:db8:4800:<n>::1 สำหรับ n = 0..=0xffff (16 บิตของ subnet id ใน /48)
                let text = format!("2001:db8:4800:{n:x}::1");
                budget.reserve(ip(&text), at(0)).is_ok()
            })
            .count();
        assert_eq!(granted, 256);
    }

    #[test]
    fn non_public_addresses_are_recognised() {
        for text in [
            "127.0.0.1",
            "10.1.2.3",
            "172.18.0.1",
            "192.168.1.9",
            "169.254.0.5",
            "0.0.0.0",
            "::1",
            "::",
            "fd00::1",
            "fe80::1",
            "::ffff:172.18.0.1",
        ] {
            assert!(is_non_public(text.parse().unwrap()), "{text}");
        }
        for text in [
            "203.0.113.5",
            "198.51.100.7",
            "2001:db8::1",
            "::ffff:8.8.8.8",
        ] {
            assert!(!is_non_public(text.parse().unwrap()), "{text}");
        }
    }

    #[test]
    fn a_clock_that_moves_backwards_does_not_lock_an_address() {
        let budget = budget(1);
        assert!(budget.reserve(ip("203.0.113.5"), at(100)).is_ok());
        assert!(budget.reserve(ip("203.0.113.5"), at(110)).is_err());
        assert!(budget.reserve(ip("203.0.113.5"), at(0)).is_ok());
    }

    #[test]
    fn a_full_table_sweeps_expired_entries_then_uses_the_shared_overflow_bucket() {
        // ตาราง 2 key, ถังรวม 3 ครั้ง
        let budget = IpBudget::with_overflow(1, Duration::minutes(10), 2, 3);
        assert!(budget.reserve(ip("203.0.113.1"), at(0)).is_ok());
        assert!(budget.reserve(ip("203.0.113.2"), at(1)).is_ok());
        // ตารางเต็มด้วยแถวที่ยังไม่หมดอายุ: ที่อยู่ใหม่ **ทุกตัว** แบ่งถังรวมร่วมกัน (ไม่ใช่ไม่จำกัด) ของเดิมยังถูกจำกัดต่อ
        assert!(budget.reserve(ip("203.0.113.3"), at(2)).is_ok());
        assert!(budget.reserve(ip("203.0.113.3"), at(3)).is_ok());
        assert!(budget.reserve(ip("203.0.113.4"), at(4)).is_ok());
        let blocked = budget.reserve(ip("203.0.113.5"), at(5)).unwrap_err();
        assert!(blocked.first);
        assert_eq!(blocked.until, at(2) + Duration::minutes(10));
        assert!(!budget.reserve(ip("203.0.113.6"), at(6)).unwrap_err().first);
        assert!(budget.reserve(ip("203.0.113.1"), at(7)).is_err());
        // แถวเก่าหมดอายุแล้ว → กวาดทิ้ง ที่อยู่ใหม่ถูกติดตามตามปกติ (ไม่ใช้ถังรวมอีก) และถังรวมเริ่มหน้าต่างใหม่ได้
        assert!(budget.reserve(ip("203.0.113.7"), at(700)).is_ok());
        assert!(budget.reserve(ip("203.0.113.7"), at(701)).is_err());
    }

    #[test]
    fn the_overflow_bucket_restarts_with_a_new_window_and_accepts_refunds() {
        // ตาราง 1 key, ถังรวม 1 ครั้ง
        let budget = IpBudget::with_overflow(1, Duration::minutes(10), 1, 1);
        assert!(budget.reserve(ip("203.0.113.1"), at(0)).is_ok());
        // ตารางเต็ม: ที่อยู่ใหม่ใช้ถังรวมได้ครั้งเดียว
        let first = budget.reserve(ip("203.0.113.2"), at(1)).unwrap();
        assert!(budget.reserve(ip("203.0.113.3"), at(2)).is_err());
        // คืนใบของถังรวม → ถังรวมว่างอีกครั้ง
        budget.refund(first);
        let old = budget.reserve(ip("203.0.113.3"), at(3)).unwrap();
        // .1 หมดอายุที่ at(600) และมีที่อยู่ใหม่เข้าแทนทันที → ตารางเต็มอีกรอบ แต่หน้าต่างของถังรวม (เริ่ม at(1)) หมดที่ at(601)
        assert!(budget.reserve(ip("203.0.113.8"), at(600)).is_ok());
        let fresh = budget.reserve(ip("203.0.113.9"), at(601)).unwrap();
        // ใบเก่าของหน้าต่างก่อนหน้าต้องไม่ลดตัวนับของหน้าต่างใหม่
        budget.refund(old);
        assert!(budget.reserve(ip("203.0.113.10"), at(602)).is_err());
        budget.refund(fresh);
        assert!(budget.reserve(ip("203.0.113.10"), at(603)).is_ok());
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
                            .filter(|_| budget.reserve(ip("203.0.113.5"), at(0)).is_ok())
                            .count()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).sum()
        });
        assert_eq!(granted, 20);
    }

    #[test]
    fn default_is_twenty_per_ten_minutes() {
        let budget = IpBudget::default();
        let granted = (0..50)
            .filter(|_| budget.reserve(ip("203.0.113.5"), at(0)).is_ok())
            .count();
        assert_eq!(granted, LOGIN_FAILURES_PER_IP as usize);
        let blocked = budget.reserve(ip("203.0.113.5"), at(1)).unwrap_err();
        assert_eq!(blocked.until, at(0) + Duration::minutes(10));
    }
}
