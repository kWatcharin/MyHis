//! แปลงค่าหนึ่งช่องของผลลัพธ์ (sqlx รับค่าจาก Postgres เป็นรูปแบบ binary) เป็น JSON ตามชนิดคอลัมน์
//! ชนิดที่ไม่รองรับ คอลัมน์ทั้งคอลัมน์ถูกทำเครื่องหมาย `supported = false` (หน้าเว็บบอกให้ cast เป็น `::text`)
//!
//! numeric ถอดเองจากรูปแบบ binary (ไม่เพิ่ม crate ชนิด decimal) — ได้ข้อความตรงตามที่ Postgres แสดง ไม่ผ่าน float

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, SecondsFormat, Utc};
use serde_json::Value;
use sqlx::error::BoxDynError;
use sqlx::postgres::types::{Oid, PgInterval};
use sqlx::postgres::{PgTypeInfo, PgTypeKind, PgValueRef, Postgres};
use sqlx::{Decode, TypeInfo, ValueRef};
use uuid::Uuid;

/// เลขจำนวนเต็มที่ JavaScript แทนได้ตรง (2^53 − 1) — เกินนี้ส่งเป็นข้อความ กันตัวเลขเพี้ยนในหน้าเว็บ
const MAX_SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// วิธีอ่านค่าของคอลัมน์ — ตัดสินครั้งเดียวต่อคอลัมน์จากชนิด
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Bool,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
    Numeric,
    Text,
    /// ชนิดภายใน `"char"` (หนึ่งไบต์)
    Char,
    Uuid,
    Date,
    Time,
    Timestamp,
    Timestamptz,
    Interval,
    Json,
    Bytea,
    Oid,
    TextArray,
    Int16Array,
    Int32Array,
    Int64Array,
    BoolArray,
    UuidArray,
    /// ผลของฟังก์ชันที่ไม่คืนค่า (เช่น pg_sleep) — แสดงเป็นค่าว่าง
    Void,
    Unsupported,
}

impl ValueKind {
    /// enum อ่านเป็นข้อความ (รูปแบบ binary ของ enum คือชื่อ label) — domain ใช้ชนิดฐานของมัน
    /// (เช่น `information_schema.sql_identifier` = name)
    pub fn of(type_info: &PgTypeInfo) -> ValueKind {
        match type_info.kind() {
            PgTypeKind::Enum(_) => ValueKind::Text,
            PgTypeKind::Domain(base) => ValueKind::of(base),
            _ => ValueKind::from_name(type_info.name()),
        }
    }

    fn from_name(name: &str) -> ValueKind {
        match name {
            "BOOL" => ValueKind::Bool,
            "INT2" => ValueKind::Int16,
            "INT4" => ValueKind::Int32,
            "INT8" => ValueKind::Int64,
            "FLOAT4" => ValueKind::Float32,
            "FLOAT8" => ValueKind::Float64,
            "NUMERIC" => ValueKind::Numeric,
            "TEXT" | "VARCHAR" | "BPCHAR" | "NAME" | "CITEXT" | "UNKNOWN" => ValueKind::Text,
            "\"CHAR\"" | "CHAR" => ValueKind::Char,
            "UUID" => ValueKind::Uuid,
            "DATE" => ValueKind::Date,
            "TIME" => ValueKind::Time,
            "TIMESTAMP" => ValueKind::Timestamp,
            "TIMESTAMPTZ" => ValueKind::Timestamptz,
            "INTERVAL" => ValueKind::Interval,
            "JSON" | "JSONB" => ValueKind::Json,
            "BYTEA" => ValueKind::Bytea,
            "OID" => ValueKind::Oid,
            "TEXT[]" | "VARCHAR[]" | "BPCHAR[]" | "NAME[]" => ValueKind::TextArray,
            "INT2[]" => ValueKind::Int16Array,
            "INT4[]" => ValueKind::Int32Array,
            "INT8[]" => ValueKind::Int64Array,
            "BOOL[]" => ValueKind::BoolArray,
            "UUID[]" => ValueKind::UuidArray,
            "VOID" => ValueKind::Void,
            _ => ValueKind::Unsupported,
        }
    }

    pub fn is_supported(self) -> bool {
        self != ValueKind::Unsupported
    }
}

fn decode<'r, T: Decode<'r, Postgres>>(value: PgValueRef<'r>) -> Result<T, BoxDynError> {
    T::decode(value)
}

/// ค่าหนึ่งช่องเป็น JSON — ชนิดที่ไม่รองรับคืน null (คอลัมน์ถูกทำเครื่องหมายไว้แล้ว)
pub fn to_json(value: PgValueRef<'_>, kind: ValueKind) -> Result<Value, BoxDynError> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    Ok(match kind {
        ValueKind::Bool => Value::Bool(decode(value)?),
        ValueKind::Int16 => Value::from(decode::<i16>(value)?),
        ValueKind::Int32 => Value::from(decode::<i32>(value)?),
        ValueKind::Int64 => integer(decode::<i64>(value)?),
        ValueKind::Float32 => float(f64::from(decode::<f32>(value)?)),
        ValueKind::Float64 => float(decode::<f64>(value)?),
        ValueKind::Numeric => {
            Value::String(numeric_to_string(value.as_bytes()?).ok_or("invalid numeric value")?)
        }
        ValueKind::Text => Value::String(decode(value)?),
        ValueKind::Char => {
            let byte = decode::<i8>(value)?.to_be_bytes()[0];
            Value::String(char::from(byte).to_string())
        }
        ValueKind::Uuid => Value::String(decode::<Uuid>(value)?.to_string()),
        ValueKind::Date => Value::String(decode::<NaiveDate>(value)?.to_string()),
        ValueKind::Time => Value::String(decode::<NaiveTime>(value)?.to_string()),
        ValueKind::Timestamp => Value::String(decode::<NaiveDateTime>(value)?.to_string()),
        ValueKind::Timestamptz => Value::String(
            decode::<DateTime<Utc>>(value)?.to_rfc3339_opts(SecondsFormat::AutoSi, true),
        ),
        ValueKind::Interval => Value::String(interval_to_string(&decode::<PgInterval>(value)?)),
        ValueKind::Json => decode::<Value>(value)?,
        ValueKind::Bytea => Value::String(bytea_to_string(decode::<&[u8]>(value)?)),
        ValueKind::Oid => Value::from(decode::<Oid>(value)?.0),
        ValueKind::TextArray => Value::from(decode::<Vec<String>>(value)?),
        ValueKind::Int16Array => Value::from(decode::<Vec<i16>>(value)?),
        ValueKind::Int32Array => Value::from(decode::<Vec<i32>>(value)?),
        ValueKind::Int64Array => Value::Array(
            decode::<Vec<i64>>(value)?
                .into_iter()
                .map(integer)
                .collect(),
        ),
        ValueKind::BoolArray => Value::from(decode::<Vec<bool>>(value)?),
        ValueKind::UuidArray => Value::from(
            decode::<Vec<Uuid>>(value)?
                .iter()
                .map(Uuid::to_string)
                .collect::<Vec<_>>(),
        ),
        ValueKind::Void | ValueKind::Unsupported => Value::Null,
    })
}

fn integer(n: i64) -> Value {
    if (-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&n) {
        Value::from(n)
    } else {
        Value::String(n.to_string())
    }
}

/// NaN/Infinity ไม่มีใน JSON — ส่งเป็นข้อความแบบที่ Postgres แสดง
fn float(x: f64) -> Value {
    if x.is_nan() {
        Value::String("NaN".to_string())
    } else if x.is_infinite() {
        Value::String(if x > 0.0 { "Infinity" } else { "-Infinity" }.to_string())
    } else {
        Value::from(x)
    }
}

/// bytea แบบที่ psql แสดง (`\x` ตามด้วย hex)
pub fn bytea_to_string(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("\\x");
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// numeric รูปแบบ binary: ndigits (i16), weight (i16), sign (u16), dscale (u16) แล้วตามด้วย ndigits หลักฐาน 10000
/// หลักที่ i มีค่าเท่ากับ digits[i] × 10000^(weight − i) — dscale = จำนวนหลักหลังจุดที่ต้องแสดง
fn numeric_to_string(bytes: &[u8]) -> Option<String> {
    let word = |at: usize| bytes.get(at..at + 2).map(|b| [b[0], b[1]]);
    let ndigits = usize::try_from(i16::from_be_bytes(word(0)?)).ok()?;
    let weight = i32::from(i16::from_be_bytes(word(2)?));
    let sign = u16::from_be_bytes(word(4)?);
    let dscale = usize::from(u16::from_be_bytes(word(6)?));
    let negative = match sign {
        0x0000 => false,
        0x4000 => true,
        0xC000 => return Some("NaN".to_string()),
        0xD000 => return Some("Infinity".to_string()),
        0xF000 => return Some("-Infinity".to_string()),
        _ => return None,
    };
    let digits: Vec<i16> = (0..ndigits)
        .map(|k| word(8 + k * 2).map(i16::from_be_bytes))
        .collect::<Option<_>>()?;
    // หลักที่อยู่นอกช่วงที่เก็บไว้ = 0
    let digit_at = |index: i32| {
        usize::try_from(index)
            .ok()
            .and_then(|i| digits.get(i))
            .copied()
            .unwrap_or(0)
    };

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if weight < 0 {
        out.push('0');
    } else {
        for index in 0..=weight {
            let group = digit_at(index);
            if index == 0 {
                out.push_str(&group.to_string());
            } else {
                out.push_str(&format!("{group:04}"));
            }
        }
    }
    if dscale > 0 {
        let groups = dscale.div_ceil(4);
        let mut fraction = String::with_capacity(groups * 4);
        for f in 1..=groups {
            let f = i32::try_from(f).ok()?;
            fraction.push_str(&format!("{:04}", digit_at(weight + f)));
        }
        fraction.truncate(dscale);
        out.push('.');
        out.push_str(&fraction);
    }
    Some(out)
}

/// interval แบบใกล้เคียงที่ psql แสดง เช่น `1 year 2 mons 3 days 04:05:06.5`
fn interval_to_string(interval: &PgInterval) -> String {
    let plural =
        |n: i32, one: &str, many: &str| format!("{n} {}", if n.abs() == 1 { one } else { many });
    let mut parts = Vec::new();
    let (years, months) = (interval.months / 12, interval.months % 12);
    if years != 0 {
        parts.push(plural(years, "year", "years"));
    }
    if months != 0 {
        parts.push(plural(months, "mon", "mons"));
    }
    if interval.days != 0 {
        parts.push(plural(interval.days, "day", "days"));
    }
    if interval.microseconds != 0 || parts.is_empty() {
        let micros = interval.microseconds.unsigned_abs();
        let sign = if interval.microseconds < 0 { "-" } else { "" };
        let (seconds, fraction) = (micros / 1_000_000, micros % 1_000_000);
        let mut time = format!(
            "{sign}{:02}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        );
        if fraction != 0 {
            let digits = format!("{fraction:06}");
            time.push('.');
            time.push_str(digits.trim_end_matches('0'));
        }
        parts.push(time);
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numeric(ndigits: i16, weight: i16, sign: u16, dscale: u16, digits: &[i16]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for word in [ndigits.to_be_bytes(), weight.to_be_bytes()] {
            bytes.extend(word);
        }
        bytes.extend(sign.to_be_bytes());
        bytes.extend(dscale.to_be_bytes());
        for digit in digits {
            bytes.extend(digit.to_be_bytes());
        }
        bytes
    }

    #[test]
    fn numeric_binary_decodes_to_exact_text() {
        let cases = [
            (numeric(2, 0, 0, 2, &[123, 4500]), "123.45"),
            (numeric(1, -1, 0x4000, 3, &[10]), "-0.001"),
            (numeric(1, 5, 0, 0, &[1]), "100000000000000000000"),
            (numeric(0, 0, 0, 0, &[]), "0"),
            (numeric(0, 0, 0, 2, &[]), "0.00"),
            (numeric(3, 1, 0, 1, &[12, 3456, 7000]), "123456.7"),
            (numeric(0, 0, 0xC000, 0, &[]), "NaN"),
        ];
        for (bytes, expected) in cases {
            assert_eq!(numeric_to_string(&bytes).as_deref(), Some(expected));
        }
        assert_eq!(numeric_to_string(&[0, 1]), None);
    }

    #[test]
    fn integers_beyond_javascript_precision_become_strings() {
        assert_eq!(integer(42), Value::from(42));
        assert_eq!(integer(i64::MAX), Value::String(i64::MAX.to_string()));
        assert_eq!(float(f64::NAN), Value::String("NaN".to_string()));
        assert_eq!(float(1.5), Value::from(1.5));
    }

    #[test]
    fn bytea_and_interval_text() {
        assert_eq!(bytea_to_string(&[0x01, 0xab]), "\\x01ab");
        let interval = PgInterval {
            months: 14,
            days: 3,
            microseconds: 14_706_500_000,
        };
        assert_eq!(
            interval_to_string(&interval),
            "1 year 2 mons 3 days 04:05:06.5"
        );
        let zero = PgInterval {
            months: 0,
            days: 0,
            microseconds: 0,
        };
        assert_eq!(interval_to_string(&zero), "00:00:00");
    }

    #[test]
    fn kinds_by_type_name() {
        assert_eq!(ValueKind::from_name("INT4"), ValueKind::Int32);
        assert_eq!(ValueKind::from_name("TEXT[]"), ValueKind::TextArray);
        assert!(!ValueKind::from_name("INET").is_supported());
    }
}
