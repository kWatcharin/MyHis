//! กฎของการลงทะเบียนผ่านลิงก์ (ไม่ต้อง login) — ห้าม import ชนิดข้อมูลของ Axum
//!
//! - ลิงก์ใช้ไม่ได้ทุกกรณี (ไม่มี/หมดอายุ/ใช้แล้ว/ถูกแทน/บัญชีถูกปิด) → `InvalidLink` ข้อความกลางเดียว ไม่บอกว่าเพราะอะไร
//! - ตรวจลิงก์ (SHA-256 + select ราคาถูก) **ก่อน** คำนวณ argon2 เสมอ — token สุ่มไม่ทำให้ต้อง hash จึงไม่เปิดช่อง DoS
//!   และ token 256 บิตเดาไม่ได้ จึงไม่ต้องมี rate limit เพิ่ม
//! - โหมดมาจากบัญชี: ยังไม่เคยลงทะเบียน → กรอกข้อมูลตัว (คำนำหน้า ชื่อ สกุล บังคับ ตำแหน่งไม่บังคับ) + ตั้งรหัส,
//!   ลงทะเบียนแล้ว (ลิงก์จากการรีเซ็ต) → ตั้งรหัสใหม่อย่างเดียว ไม่แตะข้อมูลตัว
//! - เวลา `now` รับเป็นพารามิเตอร์เสมอ (docs/rust-guidelines.md ข้อ 14)

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::registration::repository::{self, ProfileWrite, RepositoryError};
use crate::{password, token};

const PROFILE_TEXT_MAX_CHARS: usize = 100;

#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    #[error("invalid or expired registration link")]
    InvalidLink,
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Password(#[from] password::PasswordError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationMode {
    /// ลงทะเบียนครั้งแรก: กรอกข้อมูลตัว + ตั้งรหัสผ่าน
    Register,
    /// ลิงก์จากการรีเซ็ต: ตั้งรหัสผ่านใหม่อย่างเดียว
    Reset,
}

impl RegistrationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RegistrationMode::Register => "register",
            RegistrationMode::Reset => "reset",
        }
    }
}

pub struct PrenameOption {
    pub public_id: Uuid,
    pub name: String,
}

/// ข้อมูลที่หน้าลงทะเบียนแสดง (เฉพาะของบัญชีเจ้าของลิงก์)
pub struct RegistrationInfo {
    pub username: String,
    pub display_name: String,
    pub mode: RegistrationMode,
    pub expires_at: DateTime<Utc>,
    pub prename_public_id: Option<Uuid>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub position: Option<String>,
    pub prenames: Vec<PrenameOption>,
}

pub struct CompleteInput<'a> {
    pub token: &'a str,
    pub new_password: &'a str,
    pub prename_public_id: Option<Uuid>,
    pub first_name: Option<&'a str>,
    pub last_name: Option<&'a str>,
    pub position: Option<&'a str>,
}

fn mode_of(link: &repository::UsableLink) -> RegistrationMode {
    if link.registered_at.is_none() {
        RegistrationMode::Register
    } else {
        RegistrationMode::Reset
    }
}

pub async fn lookup(
    pool: &PgPool,
    raw_token: &str,
    now: DateTime<Utc>,
) -> Result<RegistrationInfo, RegistrationError> {
    let link = repository::find_usable_link(pool, &token::hash_token(raw_token), now)
        .await?
        .ok_or(RegistrationError::InvalidLink)?;
    let prenames = repository::list_prenames(pool)
        .await?
        .into_iter()
        .map(|row| PrenameOption {
            public_id: row.public_id,
            name: row.name,
        })
        .collect();

    Ok(RegistrationInfo {
        mode: mode_of(&link),
        username: link.username,
        display_name: link.display_name,
        expires_at: link.expires_at,
        prename_public_id: link.prename_public_id,
        first_name: link.first_name,
        last_name: link.last_name,
        position: link.position,
        prenames,
    })
}

/// ช่องบังคับ: trim แล้วต้องไม่ว่างและยาวไม่เกินกำหนด
fn required_text(raw: Option<&str>, field: &str) -> Result<String, RegistrationError> {
    optional_text(raw, field)?
        .ok_or_else(|| RegistrationError::Validation(format!("{field} is required")))
}

/// ช่องไม่บังคับ: trim แล้วว่าง = None
fn optional_text(raw: Option<&str>, field: &str) -> Result<Option<String>, RegistrationError> {
    let Some(value) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.chars().count() > PROFILE_TEXT_MAX_CHARS {
        return Err(RegistrationError::Validation(format!(
            "{field} must be at most {PROFILE_TEXT_MAX_CHARS} characters"
        )));
    }
    Ok(Some(value.to_string()))
}

async fn hash_password(plain: String) -> Result<String, RegistrationError> {
    tokio::task::spawn_blocking(move || password::hash(&plain))
        .await
        .map_err(|_| password::PasswordError::Hash)?
        .map_err(RegistrationError::from)
}

/// ลงทะเบียน/ตั้งรหัสใหม่ผ่านลิงก์ — ความยาวรหัส (≥12) ตรวจที่ handler (validator ของ DTO) แล้ว
pub async fn complete(
    pool: &PgPool,
    input: CompleteInput<'_>,
    now: DateTime<Utc>,
) -> Result<(), RegistrationError> {
    let token_hash = token::hash_token(input.token);
    let link = repository::find_usable_link(pool, &token_hash, now)
        .await?
        .ok_or(RegistrationError::InvalidLink)?;

    let profile = match mode_of(&link) {
        RegistrationMode::Register => {
            let prename_public_id = input
                .prename_public_id
                .ok_or_else(|| RegistrationError::Validation("prename is required".to_string()))?;
            let first_name = required_text(input.first_name, "first name")?;
            let last_name = required_text(input.last_name, "last name")?;
            let position = optional_text(input.position, "position")?;
            let prename_id = repository::find_prename_id(pool, prename_public_id)
                .await?
                .ok_or_else(|| RegistrationError::Validation("unknown prename".to_string()))?;
            Some((prename_id, first_name, last_name, position))
        }
        RegistrationMode::Reset => None,
    };

    // argon2 หลังตรวจลิงก์และข้อมูลผ่านแล้วเท่านั้น และไม่ถือทรานแซกชันระหว่าง hash
    let password_hash = hash_password(input.new_password.to_string()).await?;

    let profile_write =
        profile.as_ref().map(
            |(prename_id, first_name, last_name, position)| ProfileWrite {
                prename_id: *prename_id,
                first_name,
                last_name,
                position: position.as_deref(),
            },
        );
    let completed = repository::complete_registration(
        pool,
        &token_hash,
        now,
        &password_hash,
        profile_write.as_ref(),
    )
    .await?;
    if !completed {
        return Err(RegistrationError::InvalidLink);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_text_trims_and_rejects_blank() {
        assert_eq!(
            required_text(Some("  สมศรี "), "first name").unwrap(),
            "สมศรี"
        );
        let err = required_text(Some("   "), "first name").unwrap_err();
        assert!(matches!(err, RegistrationError::Validation(m) if m == "first name is required"));
        assert!(required_text(None, "last name").is_err());
    }

    #[test]
    fn profile_text_is_bounded() {
        assert!(optional_text(Some(&"ก".repeat(100)), "position").is_ok());
        let err = optional_text(Some(&"ก".repeat(101)), "position").unwrap_err();
        assert!(
            matches!(err, RegistrationError::Validation(m) if m == "position must be at most 100 characters")
        );
        assert_eq!(optional_text(Some("  "), "position").unwrap(), None);
    }

    #[test]
    fn invalid_link_message_is_generic() {
        assert_eq!(
            RegistrationError::InvalidLink.to_string(),
            "invalid or expired registration link"
        );
    }
}
