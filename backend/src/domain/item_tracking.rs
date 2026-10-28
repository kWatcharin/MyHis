//! การติดตามล็อต/วันหมดอายุของรายการคลัง — ใช้ร่วมกันระหว่าง item_category (ค่าตั้งต้นของหมวด) กับ item
//! (โมดูลฟีเจอร์ไม่เรียกกันเอง จึงอยู่ใน domain)
//!
//! วันหมดอายุเก็บที่ล็อต และรายการที่ไม่ติดตามล็อตมีล็อตไม่ระบุล็อตเดียว — "ติดตามวันหมดอายุแต่ไม่ติดตามล็อต"
//! จึงแยกวันหมดอายุของของคนละรอบไม่ได้ (FEFO ใช้ไม่ได้) ธงสองตัวที่ต้องสอดคล้องกันจึงเป็น enum ที่สร้างสถานะนั้น
//! ไม่ได้ตั้งแต่ขอบระบบ (docs/rust-guidelines.md ข้อ 4) ตรงกับ CHECK `*_expiry_requires_lot` ใน DB

/// ติดตามอะไรบ้างของรายการ — DB/API เก็บเป็นธงสองตัว (`track_lot`, `track_expiry`)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemTracking {
    /// ไม่ติดตามทั้งล็อตและวันหมดอายุ (ของทั้งหมดอยู่ในล็อตไม่ระบุ)
    Untracked,
    /// ติดตามล็อต ไม่ติดตามวันหมดอายุ
    Lot,
    /// ติดตามล็อตและวันหมดอายุ
    LotAndExpiry,
}

impl ItemTracking {
    /// จากธงสองตัว — None = ติดตามวันหมดอายุโดยไม่ติดตามล็อต (สถานะที่ไม่มีจริง)
    pub fn from_flags(track_lot: bool, track_expiry: bool) -> Option<Self> {
        match (track_lot, track_expiry) {
            (false, false) => Some(Self::Untracked),
            (true, false) => Some(Self::Lot),
            (true, true) => Some(Self::LotAndExpiry),
            (false, true) => None,
        }
    }

    pub fn track_lot(self) -> bool {
        !matches!(self, Self::Untracked)
    }

    pub fn track_expiry(self) -> bool {
        matches!(self, Self::LotAndExpiry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiry_without_lot_is_rejected() {
        assert_eq!(ItemTracking::from_flags(false, true), None);
    }

    #[test]
    fn valid_flags_round_trip() {
        for (lot, expiry) in [(false, false), (true, false), (true, true)] {
            let tracking = ItemTracking::from_flags(lot, expiry).unwrap();
            assert_eq!(
                (tracking.track_lot(), tracking.track_expiry()),
                (lot, expiry)
            );
        }
    }
}
