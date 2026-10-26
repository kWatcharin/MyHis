//! แปลงค่าหนึ่งช่องของผลลัพธ์ (sqlx รับค่าจาก Postgres เป็นรูปแบบ binary) เป็น JSON ตามชนิดคอลัมน์
//! ชนิดที่ไม่รองรับ คอลัมน์ทั้งคอลัมน์ถูกทำเครื่องหมาย `supported = false` (หน้าเว็บบอกให้ cast เป็น `::text`)
//!
//! numeric ถอดเองจากรูปแบบ binary (ไม่เพิ่ม crate ชนิด decimal) — ได้ข้อความตรงตามที่ Postgres แสดง ไม่ผ่าน float

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, SecondsFormat, TimeDelta};
use serde_json::Value;
use sqlx::error::BoxDynError;
use sqlx::postgres::types::{Oid, PgInterval};
use sqlx::postgres::{PgTypeInfo, PgTypeKind, PgValueFormat, PgValueRef, Postgres};
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

/// ผลลัพธ์ที่แปลงเป็น JSON แล้วใหญ่เกินงบ (`Budget`) — ผู้เรียกแยกจาก error ถอดค่าธรรมดาด้วย `downcast_ref`
#[derive(Debug)]
pub struct ResultTooLarge;

impl std::fmt::Display for ResultTooLarge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("result too large")
    }
}

impl std::error::Error for ResultTooLarge {}

/// งบหน่วยความจำของ JSON ที่สร้างจากผลลัพธ์ทั้งคำขอ (ไบต์โดยประมาณ) — แต่ละช่อง "จ่าย" **ก่อน** จองหน่วยความจำ
/// เพราะบางชนิดขยายใหญ่กว่าที่ส่งมามาก: numeric 10 ไบต์ → ข้อความ 131,001 ตัวอักษร, bytea ×2 (hex), array ทุก element กลายเป็น `Value`
/// (หลายสิบไบต์ต่อ element แม้ element ละ 1 ไบต์)
#[derive(Debug)]
pub struct Budget {
    remaining: usize,
}

impl Budget {
    pub fn new(limit: usize) -> Self {
        Self { remaining: limit }
    }

    fn charge(&mut self, bytes: usize) -> Result<(), BoxDynError> {
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or_else(|| BoxDynError::from(ResultTooLarge))?;
        Ok(())
    }
}

/// ค่าใช้จ่ายคงที่ต่อช่อง (ตัวเลข/บูล/วันที่ ฯลฯ ที่เป็น `Value` เล็ก ๆ) และต่อ `String`
const CELL_OVERHEAD: usize = 32;
/// array ข้อความ: ทุก element เป็น `String` + `Value` (~68 ไบต์ต่อ element วัดแล้ว แม้ส่งมา ~5 ไบต์) — คิดเป็นตัวคูณของไบต์บนสาย
const TEXT_ARRAY_FACTOR: usize = 16;
/// array ชนิดอื่น: ทุก element เป็น `Value` (32 ไบต์) ต่อ element ที่ส่งมา ≥ 5 ไบต์
const ARRAY_FACTOR: usize = 8;
/// json/jsonb: โครงสร้าง `Value` ใหญ่กว่าข้อความต้นทางหลายเท่า (node เล็ก ๆ จำนวนมาก)
const JSON_FACTOR: usize = 4;

/// ค่าหนึ่งช่องเป็น JSON — ชนิดที่ไม่รองรับคืน null (คอลัมน์ถูกทำเครื่องหมายไว้แล้ว) จ่ายงบ `budget` ก่อนจองหน่วยความจำ
/// เกินงบ = error ชนิด `ResultTooLarge`
pub fn to_json(
    value: PgValueRef<'_>,
    kind: ValueKind,
    budget: &mut Budget,
) -> Result<Value, BoxDynError> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    let raw_len = value.as_bytes().map(<[u8]>::len).unwrap_or(0);
    budget.charge(match kind {
        ValueKind::Text => raw_len.saturating_add(CELL_OVERHEAD),
        ValueKind::Bytea => raw_len.saturating_mul(2).saturating_add(CELL_OVERHEAD),
        ValueKind::Json => raw_len.saturating_mul(JSON_FACTOR),
        ValueKind::Numeric => numeric_text_len(value.as_bytes()?).saturating_add(CELL_OVERHEAD),
        ValueKind::TextArray => raw_len.saturating_mul(TEXT_ARRAY_FACTOR),
        ValueKind::Int16Array
        | ValueKind::Int32Array
        | ValueKind::Int64Array
        | ValueKind::BoolArray
        | ValueKind::UuidArray => raw_len.saturating_mul(ARRAY_FACTOR),
        _ => CELL_OVERHEAD,
    })?;
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
        ValueKind::Date => Value::String(date_to_string(value)?),
        ValueKind::Time => Value::String(decode::<NaiveTime>(value)?.to_string()),
        ValueKind::Timestamp => Value::String(timestamp_to_string(value, false)?),
        ValueKind::Timestamptz => Value::String(timestamp_to_string(value, true)?),
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

/// ตัวถอด chrono ของ sqlx บวกค่าออฟเซ็ตของ Postgres เข้ากับ 2000-01-01 ด้วยตัวดำเนินการ + ที่ panic เมื่อล้นช่วง
/// (`infinity`, `-infinity`, ปีเกิน ±262,142) — panic ในนี้ข้ามขั้นบันทึก audit ของหน้า SQL จึงอ่านเลขดิบเองแล้วใช้ checked_*
/// (ล้นช่วง = error ธรรมดา "cast it to ::text" ที่ถูกบันทึก audit) `infinity` แสดงแบบที่ Postgres แสดง
const INFINITY: &str = "infinity";
const NEG_INFINITY: &str = "-infinity";

fn pg_epoch() -> Result<NaiveDateTime, BoxDynError> {
    NaiveDate::from_ymd_opt(2000, 1, 1)
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .ok_or_else(|| BoxDynError::from("invalid postgres epoch"))
}

fn date_text(days: i32) -> Result<String, BoxDynError> {
    match days {
        i32::MAX => Ok(INFINITY.to_string()),
        i32::MIN => Ok(NEG_INFINITY.to_string()),
        days => pg_epoch()?
            .date()
            .checked_add_signed(TimeDelta::try_days(i64::from(days)).ok_or("date out of range")?)
            .map(|date| date.to_string())
            .ok_or_else(|| BoxDynError::from("date out of range")),
    }
}

fn timestamp_text(micros: i64, utc: bool) -> Result<String, BoxDynError> {
    match micros {
        i64::MAX => Ok(INFINITY.to_string()),
        i64::MIN => Ok(NEG_INFINITY.to_string()),
        micros => {
            let timestamp = pg_epoch()?
                .checked_add_signed(TimeDelta::microseconds(micros))
                .ok_or("timestamp out of range")?;
            Ok(if utc {
                timestamp
                    .and_utc()
                    .to_rfc3339_opts(SecondsFormat::AutoSi, true)
            } else {
                timestamp.to_string()
            })
        }
    }
}

fn date_to_string(value: PgValueRef<'_>) -> Result<String, BoxDynError> {
    if value.format() == PgValueFormat::Text {
        return Ok(decode::<NaiveDate>(value)?.to_string());
    }
    let bytes: [u8; 4] = value
        .as_bytes()?
        .try_into()
        .map_err(|_| "invalid date value")?;
    date_text(i32::from_be_bytes(bytes))
}

fn timestamp_to_string(value: PgValueRef<'_>, utc: bool) -> Result<String, BoxDynError> {
    if value.format() == PgValueFormat::Text {
        let text = decode::<&str>(value)?;
        return Ok(text.to_string());
    }
    let bytes: [u8; 8] = value
        .as_bytes()?
        .try_into()
        .map_err(|_| "invalid timestamp value")?;
    timestamp_text(i64::from_be_bytes(bytes), utc)
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

/// ความยาวข้อความ (ตัวอักษร) ที่ `numeric_to_string` จะสร้าง คำนวณจาก header 8 ไบต์ **ก่อน** จองหน่วยความจำ: เลขที่เก็บ 10 ไบต์
/// (ndigits = 1, weight = 32767) ขยายเป็น ~131,000 ตัวอักษร header ที่อ่านไม่ได้คืน 0 (ตัวถอดจะ error เอง)
fn numeric_text_len(bytes: &[u8]) -> usize {
    let word = |at: usize| bytes.get(at..at + 2).map(|b| [b[0], b[1]]);
    let (Some(weight), Some(dscale)) = (word(2), word(6)) else {
        return 0;
    };
    let weight = i16::from_be_bytes(weight);
    let dscale = usize::from(u16::from_be_bytes(dscale));
    let integer = usize::try_from(weight).map_or(1, |w| (w + 1) * 4);
    // เครื่องหมาย + ส่วนจำนวนเต็ม + (จุด + ส่วนทศนิยมปัดขึ้นเป็นกลุ่มละ 4 หลัก)
    1 + integer
        + if dscale > 0 {
            1 + dscale.div_ceil(4) * 4
        } else {
            0
        }
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
    fn numeric_text_length_is_known_before_decoding() {
        // header 8 ไบต์ + 1 หลัก แต่ weight 32767 ขยายเป็นข้อความ ~131,000 ตัวอักษร (วัดจริงใน audit รอบ 2)
        let huge = numeric(1, 32767, 0, 0, &[1]);
        let expected = numeric_to_string(&huge).unwrap().len();
        let estimate = numeric_text_len(&huge);
        assert!(estimate >= expected, "{estimate} < {expected}");
        assert!(estimate < expected + 16, "{estimate} vs {expected}");

        // ทุกกรณีของ decoder ต้องประมาณไม่ต่ำกว่าความยาวจริง
        for bytes in [
            numeric(2, 0, 0, 2, &[123, 4500]),
            numeric(1, -1, 0x4000, 3, &[10]),
            numeric(1, 5, 0, 0, &[1]),
            numeric(0, 0, 0, 2, &[]),
            numeric(3, 1, 0, 1, &[12, 3456, 7000]),
        ] {
            let actual = numeric_to_string(&bytes).unwrap().len();
            assert!(numeric_text_len(&bytes) >= actual, "{bytes:?}");
        }
        assert_eq!(numeric_text_len(&[0, 1]), 0);
    }

    #[test]
    fn the_budget_refuses_what_it_cannot_afford() {
        let mut budget = Budget::new(100);
        assert!(budget.charge(60).is_ok());
        assert!(budget.charge(40).is_ok());
        let err = budget.charge(1).unwrap_err();
        assert!(err.downcast_ref::<ResultTooLarge>().is_some());
        // งบที่หมดแล้วไม่ล้นกลับ
        assert!(budget.charge(usize::MAX).is_err());
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

    /// infinity / ช่วงที่ chrono แทนไม่ได้ต้องไม่ panic (panic ข้ามการบันทึก audit ของหน้า SQL) — infinity แสดงแบบ Postgres
    /// ส่วนนอกช่วงเป็น error ธรรมดา
    #[test]
    fn temporal_values_never_panic_on_infinity_or_out_of_range() {
        assert_eq!(date_text(0).unwrap(), "2000-01-01");
        assert_eq!(date_text(-1).unwrap(), "1999-12-31");
        assert_eq!(date_text(i32::MAX).unwrap(), "infinity");
        assert_eq!(date_text(i32::MIN).unwrap(), "-infinity");
        // 262143-01-01 เป็นวันที่ที่ Postgres รับแต่เกินช่วงของ chrono (ปีสูงสุด 262142)
        assert!(date_text(i32::MAX - 1).is_err());
        assert!(date_text(i32::MIN + 1).is_err());

        assert_eq!(timestamp_text(0, false).unwrap(), "2000-01-01 00:00:00");
        assert_eq!(timestamp_text(0, true).unwrap(), "2000-01-01T00:00:00Z");
        assert_eq!(
            timestamp_text(1_500_000, true).unwrap(),
            "2000-01-01T00:00:01.500Z"
        );
        assert_eq!(timestamp_text(i64::MAX, true).unwrap(), "infinity");
        assert_eq!(timestamp_text(i64::MIN, false).unwrap(), "-infinity");
        assert!(timestamp_text(i64::MAX - 1, true).is_err());
        assert!(timestamp_text(i64::MIN + 1, false).is_err());
    }

    #[test]
    fn kinds_by_type_name() {
        assert_eq!(ValueKind::from_name("INT4"), ValueKind::Int32);
        assert_eq!(ValueKind::from_name("TEXT[]"), ValueKind::TextArray);
        assert!(!ValueKind::from_name("INET").is_supported());
    }
}
