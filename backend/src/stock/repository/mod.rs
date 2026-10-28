//! query SQLx ของโมดูล stock (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! ตรวจตอน compile ไม่ประกอบ SQL ด้วย format!
//! (docs/rust-guidelines.md ข้อ 11) แยกไฟล์: `ledger` = แกนที่ทุกเอกสารใช้ร่วม (ล็อต, movement, คงเหลือ) และ
//! `receipts` = ใบรับเข้า — ขั้นเบิก/นับ/ปรับยอดเพิ่มไฟล์ของเอกสารตัวเองแล้วเรียก `ledger` แบบเดียวกัน
//!
//! อ่านตาราง item / item_unit / item_unit_conversion / budget_year ตรง ๆ ไม่เรียก service ของโมดูลนั้น
//! (โมดูลฟีเจอร์ไม่เรียกกันเอง — แบบเดียวกับที่ item อ่านหมวด/หน่วย)
//!
//! ledger เพิ่มได้อย่างเดียว (DB มี trigger กันแก้/ลบ) และทุกการเปลี่ยนยอดเกิดในทรานแซกชันเดียวกับเอกสาร + `audit_log`

mod ledger;
mod receipts;

pub use receipts::*;

use chrono::NaiveDate;
use sqlx::PgPool;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    /// ค่าใน DB ที่ CHECK ควรกันไว้แล้ว (เช่น receipt_type ที่ไม่รู้จัก) — ข้อมูลเสีย ไม่ใช่ input ผู้ใช้
    #[error("unexpected value in {0}")]
    InvalidValue(&'static str),
}

/// วันนี้ตามเวลาไทย (คำนวณใน Postgres — ไม่เพิ่ม crate timezone) ใช้ตรวจว่าวันที่รับไม่เกินวันนี้
pub async fn thai_today(pool: &PgPool) -> Result<NaiveDate, RepositoryError> {
    let today =
        sqlx::query_scalar!(r#"select (now() at time zone 'Asia/Bangkok')::date as "today!""#)
            .fetch_one(pool)
            .await?;
    Ok(today)
}
