//! ถอดรหัส ciphertext ของคอลัมน์ผู้ป่วยที่ "ฝัง" อยู่ในค่าผลลัพธ์ซึ่งไม่ได้มาจากคอลัมน์ตรง ๆ (ฟังก์ชันล้วน)
//! เช่น `concat(p.name, pt.first_name, ' ', pt.last_name)`, `pt.first_name::text`, `coalesce`/`case`, `union`,
//! `pt.first_name || ' ' || pt.last_name` (bytea), `string_agg`, `json_build_object`, `encode(pt.hn, 'hex')`
//!
//! Postgres แสดง bytea เป็นข้อความ `\x<hex>` (bytea_output = hex) เมื่อถูกแปลงเป็น text/json จึงหา hex ในข้อความ แปลงกลับ
//! เป็นไบต์ แล้วลองถอดทีละ blob (`version 01 || nonce || ciphertext+tag` ของ crypto.rs) ด้วย AAD ของทุกคอลัมน์ผู้ป่วย —
//! AES-GCM ตรวจ tag กับ AAD เสมอ hex ที่ไม่ใช่ ciphertext จริงของคอลัมน์ผู้ป่วย (blind index, hash, uuid) จึงถอดไม่ผ่าน
//! ไม่มีทางได้ข้อความผิดออกไป ค่าที่ถูกแปลงแบบอื่น (substring, md5, base64) ถอดไม่ได้ แสดงตามเดิม
//!
//! ถอดแค่ค่าในผลลัพธ์ (ไม่เกิน MAX_ROWS แถว) ไม่ได้ถอดทั้งตารางไปกรอง และไม่ส่ง plaintext กลับเข้า DB

use std::cell::Cell;

use serde_json::Value;

/// blob สั้นที่สุด: version (1) + nonce (12) + tag (16) — plaintext ว่าง
const MIN_BLOB: usize = 1 + 12 + 16;
/// blob ยาวที่สุดที่ลอง: ชื่อ ≤ 100 ตัวอักษร (≤ 400 ไบต์ UTF-8) + ส่วนเกิน จึง 512 ไบต์พอ — จำกัดจำนวนครั้งที่ลองถอดต่อตำแหน่ง
const MAX_BLOB: usize = MIN_BLOB + 512;
/// hex run ที่ยาวกว่านี้ (ตัวอักษร) ไม่สแกน: ciphertext ผู้ป่วยต่อกันได้ไม่กี่ก้อน ส่วน run ยาวคือข้อมูลอื่น
/// (เช่น `repeat('01', N)`) ที่ถ้าสแกนจะกินเวลาโตตามขนาดเซลล์
const MAX_HEX_RUN: usize = 4096;
/// งบจำนวนครั้งที่ลองถอด blob ต่อคำขอ (รวมทุกเซลล์) — เกินแล้วค่าที่เหลือแสดงเป็น hex ตามเดิม ผลปกติใช้หลักสิบถึงหลักพันครั้ง
/// (ทั้งค่าเป็น blob เดียวถอดได้ในครั้งแรก) ตัวนี้กันคำขอที่ตั้งใจสร้างงานถอดไม่รู้จบ — ขอบเขตงานต่อคำขอจึงกำหนดได้
pub const MAX_DECRYPT_ATTEMPTS: usize = 200_000;
/// ไบต์แรกของ blob (crypto.rs BLOB_VERSION)
const BLOB_VERSION: u8 = 1;

/// งบการลองถอดที่ใช้ร่วมกันทั้งคำขอ (ผู้เรียกสร้างหนึ่งอันต่อคำขอ)
pub struct Budget {
    left: Cell<usize>,
}

impl Budget {
    pub fn new(attempts: usize) -> Self {
        Self {
            left: Cell::new(attempts),
        }
    }

    fn is_exhausted(&self) -> bool {
        self.left.get() == 0
    }

    /// หักหนึ่งครั้ง — false = หมดงบแล้ว ห้ามลอง
    fn take(&self) -> bool {
        let left = self.left.get();
        if left == 0 {
            return false;
        }
        self.left.set(left - 1);
        true
    }
}

/// ลองถอดหนึ่งก้อนโดยหักงบ — หมดงบ = None
fn attempt(
    budget: &Budget,
    decrypt: &dyn Fn(&[u8]) -> Option<String>,
    blob: &[u8],
) -> Option<String> {
    budget.take().then(|| decrypt(blob)).flatten()
}

/// ถอด ciphertext ทุกก้อนที่พบในข้อความของค่านี้ (string ใน array/object ด้วย) — `decrypt` ลองถอด blob หนึ่งก้อน
/// ด้วย AAD ของทุกคอลัมน์ผู้ป่วย คืน true ถ้าถอดได้อย่างน้อยหนึ่งก้อน
pub fn decrypt_in_value(
    value: &mut Value,
    decrypt: &dyn Fn(&[u8]) -> Option<String>,
    budget: &Budget,
) -> bool {
    match value {
        Value::String(text) => match decrypt_in_text(text, decrypt, budget) {
            Some(plain) => {
                *text = plain;
                true
            }
            None => false,
        },
        Value::Array(items) => items.iter_mut().fold(false, |found, item| {
            decrypt_in_value(item, decrypt, budget) | found
        }),
        Value::Object(map) => map.values_mut().fold(false, |found, item| {
            decrypt_in_value(item, decrypt, budget) | found
        }),
        _ => false,
    }
}

/// แทน hex ที่ถอดได้ในข้อความด้วย plaintext — `\x<hex>` (bytea ที่ถูกแปลงเป็น text) หรือ hex เปล่าที่ขึ้นต้นด้วย
/// version 01 (`encode(..., 'hex')`) ไม่มีก้อนไหนถอดได้ = None
fn decrypt_in_text(
    text: &str,
    decrypt: &dyn Fn(&[u8]) -> Option<String>,
    budget: &Budget,
) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut copied = 0;
    let mut i = 0;
    while i < bytes.len() && !budget.is_exhausted() {
        if !bytes[i].is_ascii_hexdigit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_hexdigit() {
            i += 1;
        }
        let run = &text[start..i];
        let prefixed = text[..start].ends_with("\\x");
        if !run.len().is_multiple_of(2)
            || run.len() < MIN_BLOB * 2
            || run.len() > MAX_HEX_RUN
            || !(prefixed || run.starts_with("01"))
        {
            continue;
        }
        let Some(plain) = decode_hex(run).and_then(|raw| decrypt_bytes(&raw, decrypt, budget))
        else {
            continue;
        };
        let token_start = if prefixed { start - 2 } else { start };
        out.push_str(&text[copied..token_start]);
        out.push_str(&plain);
        copied = i;
    }
    (copied > 0).then(|| {
        out.push_str(&text[copied..]);
        out
    })
}

/// ไบต์ที่มี blob หนึ่งก้อนหรือหลายก้อนต่อกัน อาจมีไบต์อื่นคั่น (`first_name || ' ' || last_name` ที่เป็น bytea) —
/// ไบต์ที่ไม่ใช่ blob ต้องเป็น UTF-8 ถูกต้อง ไม่งั้นไม่ถอดทั้งค่า (แสดง hex ตามเดิม)
fn decrypt_bytes(
    raw: &[u8],
    decrypt: &dyn Fn(&[u8]) -> Option<String>,
    budget: &Budget,
) -> Option<String> {
    // กรณีส่วนใหญ่: ทั้งค่าคือ blob เดียว
    if let Some(plain) = attempt(budget, decrypt, raw) {
        return Some(plain);
    }
    let mut out = Vec::with_capacity(raw.len());
    let mut found = false;
    let mut i = 0;
    while i < raw.len() {
        if budget.is_exhausted() {
            // หมดงบ: ที่เหลือคัดลอกตามเดิม ไม่ลองอีก
            out.extend_from_slice(&raw[i..]);
            break;
        }
        if raw[i] == BLOB_VERSION {
            if let Some((plain, end)) = blob_at(raw, i, decrypt, budget) {
                out.extend_from_slice(plain.as_bytes());
                found = true;
                i = end;
                continue;
            }
        }
        out.push(raw[i]);
        i += 1;
    }
    if !found {
        return None;
    }
    String::from_utf8(out).ok()
}

/// blob ที่เริ่มตรง `start` (ลองทุกความยาว — tag ของ GCM ผ่านได้แค่ความยาวที่ถูกเท่านั้น) → (plaintext, ตำแหน่งจบ)
fn blob_at(
    raw: &[u8],
    start: usize,
    decrypt: &dyn Fn(&[u8]) -> Option<String>,
    budget: &Budget,
) -> Option<(String, usize)> {
    let last = raw.len().min(start + MAX_BLOB);
    (start + MIN_BLOB..=last)
        .take_while(|_| !budget.is_exhausted())
        .find_map(|end| attempt(budget, decrypt, &raw[start..end]).map(|plain| (plain, end)))
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::crypto::Crypto;

    const COLUMNS: [&str; 2] = ["patient.first_name", "patient.last_name"];

    fn crypto() -> Crypto {
        Crypto::from_hex_keys(&"a".repeat(64), &"b".repeat(64)).unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// ค่าที่ผ่าน decrypt_in_value แล้ว + ถอดได้ไหม
    fn run(value: Value) -> (Value, bool) {
        let crypto = crypto();
        let decrypt = |blob: &[u8]| {
            COLUMNS
                .iter()
                .find_map(|aad| crypto.decrypt(aad, blob).ok())
        };
        let mut value = value;
        let found = decrypt_in_value(&mut value, &decrypt, &Budget::new(MAX_DECRYPT_ATTEMPTS));
        (value, found)
    }

    /// จำนวนครั้งที่ตัวถอดถูกเรียก เมื่อรันกับค่านี้ด้วยงบ `max`
    fn calls(value: Value, max: usize) -> (usize, Value) {
        let counter = Cell::new(0usize);
        let decrypt = |_blob: &[u8]| {
            counter.set(counter.get() + 1);
            None
        };
        let mut value = value;
        decrypt_in_value(&mut value, &decrypt, &Budget::new(max));
        (counter.get(), value)
    }

    fn names() -> (Vec<u8>, Vec<u8>) {
        let crypto = crypto();
        (
            crypto.encrypt("patient.first_name", "สมชาย").unwrap(),
            crypto.encrypt("patient.last_name", "ใจดี").unwrap(),
        )
    }

    #[test]
    fn decrypts_bytea_text_inside_concat() {
        let (first, last) = names();
        // concat(p.name, pt.first_name, ' ', pt.last_name)
        let text = format!("นาย\\x{} \\x{}", hex(&first), hex(&last));
        assert_eq!(run(json!(text)), (json!("นายสมชาย ใจดี"), true));
        // concat ไม่มีตัวคั่น: \x ของก้อนถัดไปแบ่งก้อนให้เอง
        let text = format!("\\x{}\\x{}", hex(&first), hex(&last));
        assert_eq!(run(json!(text)), (json!("สมชายใจดี"), true));
    }

    #[test]
    fn decrypts_concatenated_bytea_with_separators() {
        let (first, last) = names();
        // pt.first_name || ' ' || pt.last_name เป็น bytea ก้อนเดียว
        let raw = [first.as_slice(), b" ", last.as_slice()].concat();
        assert_eq!(
            run(json!(format!("\\x{}", hex(&raw)))),
            (json!("สมชาย ใจดี"), true)
        );
    }

    #[test]
    fn decrypts_bare_hex_json_and_arrays() {
        let (first, last) = names();
        // encode(pt.first_name, 'hex')
        assert_eq!(run(json!(hex(&first))), (json!("สมชาย"), true));
        // json_build_object / array_agg(...::text)
        let value = json!({ "name": format!("\\x{}", hex(&first)), "list": [format!("\\x{}", hex(&last)), 1] });
        assert_eq!(
            run(value),
            (json!({ "name": "สมชาย", "list": ["ใจดี", 1] }), true)
        );
    }

    #[test]
    fn leaves_other_hex_untouched() {
        let crypto = crypto();
        let index = format!("\\x{}", hex(&crypto.blind_index("patient.hn", "66471")));
        let sha = "01".to_string() + &"ab".repeat(31);
        let other_key = Crypto::from_hex_keys(&"c".repeat(64), &"d".repeat(64))
            .unwrap()
            .encrypt("patient.first_name", "สมชาย")
            .unwrap();
        for text in [
            index,
            sha,
            format!("\\x{}", hex(&other_key)),
            "\\x01".to_string(),
            "no hex here".to_string(),
        ] {
            assert_eq!(run(json!(text.clone())), (json!(text), false));
        }
        // ไบต์คั่นที่ไม่ใช่ UTF-8 → ไม่ถอดทั้งค่า
        let (first, _) = names();
        let raw = [first.as_slice(), &[0xff, 0xfe]].concat();
        let text = format!("\\x{}", hex(&raw));
        assert_eq!(run(json!(text.clone())), (json!(text), false));
    }

    /// ค่าที่สร้างงานถอดให้ไม่รู้จบต้องถูกจำกัด: hex run ยาวเกินเพดานไม่ถูกสแกนเลย และ run ที่สั้นกว่าเพดานก็ลองได้ไม่เกินงบ
    #[test]
    fn work_per_value_is_bounded_whatever_the_input() {
        // 6,000 ตัวอักษร hex > MAX_HEX_RUN → ข้ามทั้งก้อน ไม่ลองถอดสักครั้ง
        let long = format!("\\x{}", "01".repeat(3000));
        let (count, value) = calls(json!(long.clone()), MAX_DECRYPT_ATTEMPTS);
        assert_eq!((count, value), (0, json!(long)));

        // 4,000 ตัวอักษร hex (≤ เพดาน) ที่ทุกไบต์เป็น 0x01 → 2,000 ตำแหน่งเริ่ม × ~500 ความยาว: ต้องหยุดที่งบ
        let dense = format!("\\x{}", "01".repeat(2000));
        let (count, value) = calls(json!(dense.clone()), 1000);
        assert_eq!((count, value), (1000, json!(dense)));
        let (count, _) = calls(json!(dense), MAX_DECRYPT_ATTEMPTS);
        assert_eq!(count, MAX_DECRYPT_ATTEMPTS);
    }

    /// งบใช้ร่วมกันทั้งคำขอ: เซลล์ถัดไปไม่ได้งบใหม่
    #[test]
    fn the_budget_is_shared_across_cells() {
        let dense = format!("\\x{}", "01".repeat(1000));
        let counter = Cell::new(0usize);
        let decrypt = |_blob: &[u8]| {
            counter.set(counter.get() + 1);
            None
        };
        let budget = Budget::new(300);
        for _ in 0..4 {
            decrypt_in_value(&mut json!(dense.clone()), &decrypt, &budget);
        }
        assert_eq!(counter.get(), 300);
    }
}
