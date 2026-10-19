//! รูปแบบ JSON เข้า/ออกของหน้า SQL — แยกจากชนิดภายใน (docs/rust-guidelines.md ข้อ 4)
//!
//! `QueryRequest` (ตัว SQL อาจมี HN ในตัวช่วย :hn()) และ `QueryResponse` (ผลลัพธ์อาจมีข้อมูลผู้ป่วยที่ถอดรหัสแล้ว)
//! ไม่พิมพ์ค่าใน Debug กันหลุดลง log

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct QueryRequest {
    pub sql: String,
    /// ขอถอดรหัสคอลัมน์ผู้ป่วยในผลลัพธ์ (มีผลเมื่อมีสิทธิ์ patient.view เท่านั้น)
    #[serde(default)]
    pub decrypt: bool,
}

impl std::fmt::Debug for QueryRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryRequest")
            .field("sql", &"[redacted]")
            .field("decrypt", &self.decrypt)
            .finish()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnResponse {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub supported: bool,
    pub encrypted: bool,
    pub decrypted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResponse {
    pub columns: Vec<ColumnResponse>,
    pub rows: Vec<Vec<Value>>,
    pub row_count: usize,
    pub truncated: bool,
    pub duration_ms: u64,
    pub decrypt_errors: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaColumnResponse {
    pub name: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub encrypted: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaTableResponse {
    pub name: String,
    pub columns: Vec<SchemaColumnResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaResponse {
    pub tables: Vec<SchemaTableResponse>,
    /// ขีดจำกัดที่หน้าเว็บแสดงให้ผู้ใช้รู้ (ค่าจริงอยู่ฝั่ง backend)
    pub max_rows: usize,
    pub statement_timeout_ms: u64,
}
