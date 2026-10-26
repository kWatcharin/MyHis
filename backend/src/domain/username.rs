//! รูปแบบ username ที่เก็บจริง — ใช้ร่วมกันระหว่างตอนสร้างบัญชี (users) กับตอน login (auth) จึงอยู่ใน `domain/`
//! (โมดูลฟีเจอร์ห้ามเรียกกันเอง; docs/rust-guidelines.md ข้อ 4)
//!
//! username ที่เก็บ = trim + ตัวพิมพ์เล็กแล้ว ยาว 3–32 ตัว เฉพาะ a-z 0-9 . _ - (พิมพ์บนมือถือได้ง่าย ไม่มีช่องว่าง)
//! ชื่อที่ไม่ตรงรูปแบบนี้ **ไม่มีทางมีอยู่ในฐานข้อมูล** — login จึงข้ามการค้นหาและปฏิบัติเหมือน ‘ชื่อที่ไม่มีในระบบ’ ได้เลย
//! (และไม่ส่งอักขระอย่าง NUL เข้า Postgres ที่ปฏิเสธมันด้วย error ซึ่งกลายเป็น 500)

pub const MIN_CHARS: usize = 3;
pub const MAX_CHARS: usize = 32;

/// `username` (หลัง trim + ตัวพิมพ์เล็ก) ตรงรูปแบบที่เก็บได้หรือไม่
pub fn is_well_formed(username: &str) -> bool {
    let length = username.chars().count();
    (MIN_CHARS..=MAX_CHARS).contains(&length)
        && username
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_stored_shape() {
        for ok in ["abc", "nurse.som_1-a", "a.b", &"a".repeat(MAX_CHARS)] {
            assert!(is_well_formed(ok), "{ok:?}");
        }
    }

    #[test]
    fn rejects_everything_else() {
        let too_long = "a".repeat(MAX_CHARS + 1);
        for bad in [
            "",
            "ab",
            too_long.as_str(),
            "Nurse",
            "has space",
            "tab\tname",
            "nul\0byte",
            "ชื่อ",
            "a@b.c",
            "a/b",
        ] {
            assert!(!is_well_formed(bad), "{bad:?}");
        }
    }
}
