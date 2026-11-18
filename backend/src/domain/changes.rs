//! รายละเอียดการเปลี่ยนแปลงสำหรับ `audit_log.metadata` (หน้า "ประวัติการใช้งาน" → ปุ่ม "ดู") — ฟังก์ชันล้วน ใช้ร่วมกันทุกโมดูล
//! (โมดูลฟีเจอร์ไม่เรียกกันเอง จึงอยู่ใน domain)
//!
//! input คือ snapshot ของแถว (JSON object ของค่าสำหรับแสดงผล — FK แปลงเป็นชื่อแล้ว ไม่มี id int ภายใน ไม่มีความลับ)
//! ที่แต่ละ repository สร้างเองในทรานแซกชันเดียวกับการเขียนข้อมูล:
//! - เพิ่ม → `{"after": {...ทุกช่อง}}`
//! - แก้ไข → `{"before": {...}, "after": {...}}` เฉพาะช่องที่ค่าต่างกัน
//! - ข้อมูลผู้ป่วย → `{"changed": [ชื่อช่อง]}` เท่านั้น **ห้ามเก็บค่า** (CLAUDE.md — ไม่มีข้อมูลผู้ป่วยใน audit_log)

use serde_json::{json, Map, Value};

/// ยืม object ใน snapshot ไม่ clone ทั้งก้อน — ไม่ใช่ object = map ว่าง (`empty` ของผู้เรียก)
fn object<'a>(value: &'a Value, empty: &'a Map<String, Value>) -> &'a Map<String, Value> {
    value.as_object().unwrap_or(empty)
}

/// ค่าของแถวที่เพิ่ม
pub fn created(after: Value) -> Value {
    json!({ "after": after })
}

/// ค่าเดิม/ค่าใหม่ของช่องที่เปลี่ยน (ช่องที่หายไปฝั่งหนึ่ง = null) — ไม่มีอะไรเปลี่ยน = before/after ว่าง
pub fn updated(before: &Value, after: &Value) -> Value {
    let empty = Map::new();
    let (before, after) = (object(before, &empty), object(after, &empty));
    let mut old = Map::new();
    let mut new = Map::new();
    for key in before.keys().chain(after.keys()) {
        let (b, a) = (
            before.get(key).unwrap_or(&Value::Null),
            after.get(key).unwrap_or(&Value::Null),
        );
        if b != a && !old.contains_key(key) {
            old.insert(key.clone(), b.clone());
            new.insert(key.clone(), a.clone());
        }
    }
    json!({ "before": old, "after": new })
}

/// ชื่อช่องที่เปลี่ยน (เรียงตามตัวอักษร) + ช่องที่ผู้เรียกเทียบเองแล้วว่าเปลี่ยน (`extra` — เช่นชื่อผู้ป่วยที่เข้ารหัส
/// แบบสุ่ม nonce จึงเทียบใน DB ไม่ได้) ไม่มีค่าใด ๆ ใน output — ใช้กับข้อมูลผู้ป่วย
pub fn changed_fields(before: &Value, after: &Value, extra: &[&str]) -> Value {
    let diff = updated(before, after);
    let empty = Map::new();
    let mut fields: Vec<String> = object(&diff["before"], &empty).keys().cloned().collect();
    fields.extend(extra.iter().map(|field| field.to_string()));
    fields.sort();
    fields.dedup();
    json!({ "changed": fields })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updated_keeps_only_changed_fields() {
        let before =
            json!({ "name": "DM", "abbreviation": "DM", "sort_order": 1, "is_active": true });
        let after =
            json!({ "name": "DM", "abbreviation": "T2DM", "sort_order": 2, "is_active": true });
        assert_eq!(
            updated(&before, &after),
            json!({
                "before": { "abbreviation": "DM", "sort_order": 1 },
                "after": { "abbreviation": "T2DM", "sort_order": 2 },
            })
        );
    }

    #[test]
    fn updated_with_no_change_is_empty_and_handles_missing_keys() {
        let row = json!({ "name": "x" });
        assert_eq!(updated(&row, &row), json!({ "before": {}, "after": {} }));
        assert_eq!(
            updated(&json!({ "icon": "home" }), &json!({})),
            json!({ "before": { "icon": "home" }, "after": { "icon": null } })
        );
    }

    #[test]
    fn changed_fields_has_names_only() {
        let before = json!({ "weight": 67.01, "age": 30, "hn": "aa" });
        let after = json!({ "weight": 70.5, "age": 30, "hn": "aa" });
        let out = changed_fields(&before, &after, &["first_name", "weight"]);
        assert_eq!(out, json!({ "changed": ["first_name", "weight"] }));
        let printed = out.to_string();
        assert!(!printed.contains("67.01") && !printed.contains("70.5"));
    }

    #[test]
    fn created_wraps_after() {
        assert_eq!(
            created(json!({ "name": "x" })),
            json!({ "after": { "name": "x" } })
        );
    }
}
