//! กฎของโมดูล stock (ธุรกรรมคลัง — docs/inventory-design.md ข้อ 5): สิทธิ์ราย action, ตรวจค่าเอกสาร, แปลงผลของ
//! repository เป็น error เชิงธุรกิจ ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 3)
//! แยกไฟล์ตามเอกสาร (`receipts` = ใบรับเข้า + ยอดยกมา) แบบ `users/service/` — ขั้นเบิก/นับ/ปรับยอดเพิ่มไฟล์ของตัวเอง
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็คสิทธิ์ของ action นั้นที่นี่
//! — id ขาเข้าเป็น uuid (`public_id`) แล้ว resolve เป็น id ภายในเอง

mod receipts;

pub use receipts::*;

use crate::auth::CurrentUser;
use crate::domain::{Action, Resource};
use crate::stock::repository::RepositoryError;

#[derive(Debug, thiserror::Error)]
pub enum StockError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

fn require(actor: &CurrentUser, resource: Resource, action: Action) -> Result<(), StockError> {
    if actor.can(resource, action) {
        Ok(())
    } else {
        Err(StockError::Forbidden)
    }
}

fn validation(message: &str) -> StockError {
    StockError::Validation(message.to_string())
}

fn conflict(message: &str) -> StockError {
    StockError::Conflict(message.to_string())
}

/// trim แล้วค่าว่าง = None, ยาวเกิน `max_chars` ตัวอักษร (ไม่ใช่ byte — ข้อความไทย) = validation
fn clean_optional(
    value: Option<&str>,
    field: &str,
    max_chars: usize,
) -> Result<Option<String>, StockError> {
    let Some(trimmed) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    if trimmed.chars().count() > max_chars {
        return Err(StockError::Validation(format!(
            "{field} must be at most {max_chars} characters"
        )));
    }
    Ok(Some(trimmed.to_string()))
}

/// เหมือน `clean_optional` แต่ต้องไม่ว่าง
fn clean_required(value: &str, field: &str, max_chars: usize) -> Result<String, StockError> {
    clean_optional(Some(value), field, max_chars)?
        .ok_or_else(|| StockError::Validation(format!("{field} is required")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_text_is_trimmed_and_blank_becomes_none() {
        assert_eq!(clean_optional(None, "note", 5).unwrap(), None);
        assert_eq!(clean_optional(Some("   "), "note", 5).unwrap(), None);
        assert_eq!(
            clean_optional(Some(" ab "), "note", 5).unwrap(),
            Some("ab".to_string())
        );
        // นับตัวอักษร ไม่ใช่ byte
        assert!(clean_optional(Some("กขคงจ"), "note", 5).is_ok());
        assert!(clean_optional(Some("กขคงจฉ"), "note", 5).is_err());
        assert!(clean_required(" ", "reason", 5).is_err());
    }
}
