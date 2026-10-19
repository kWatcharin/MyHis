//! อ่าน environment variable ที่เดียวของทั้งแอป (docs/rust-guidelines.md ข้อ 15)
//! คืน `Result` เสมอ ไม่ unwrap/expect ที่นี่ — อนุญาตเฉพาะใน main.rs ตอนเริ่มระบบ
//! (docs/rust-guidelines.md ข้อ 2) ถ้าค่าที่จำเป็นหายหรือผิดรูปแบบ ให้ main.rs ล้มตั้งแต่ตอนเริ่ม

use std::net::SocketAddr;

use chrono::Duration;
use thiserror::Error;

use crate::crypto::Crypto;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing environment variable: {0}")]
    Missing(&'static str),
    #[error("invalid value for {name}: {value}")]
    Invalid { name: &'static str, value: String },
    /// ค่าลับที่ผิดรูปแบบ — ไม่แนบค่าลงใน error เพราะ error ถูก log ได้ (ห้าม log key)
    #[error("invalid secret in {0}")]
    InvalidSecret(&'static str),
}

pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub session_idle_ttl: Duration,
    pub session_absolute_ttl: Duration,
    pub initial_admin_username: String,
    pub initial_admin_password: String,
    pub crypto: Crypto,
    /// Cloudflare Turnstile ที่หน้า login — None = ปิด (ไม่ได้ตั้ง key ทั้งคู่)
    pub turnstile: Option<TurnstileConfig>,
    /// รหัสของ role ฐานข้อมูลแยกของหน้า SQL (`SQL_CONSOLE_DB_PASSWORD`) — None = ปิดหน้า SQL
    /// role/host/ฐานข้อมูลมาจาก sql_console::READER_ROLE + DATABASE_URL (ดู sql_console/setup.rs)
    pub sql_console_db_password: Option<String>,
}

/// รหัสของ role หน้า SQL ต้องยาวอย่างน้อยเท่านี้ (เช่น `openssl rand -hex 24`) — role นี้ login ได้จาก network ของ compose
const MIN_SQL_CONSOLE_PASSWORD_CHARS: usize = 16;

/// key ของ widget Turnstile (Cloudflare dashboard) — site key เป็นค่าสาธารณะ (อยู่ในหน้าเว็บ) secret ห้ามหลุด
pub struct TurnstileConfig {
    pub site_key: String,
    pub secret_key: String,
    /// `TURNSTILE_HOSTNAME` (ไม่บังคับ) — ถ้าตั้ง ผลตรวจจาก key จริงต้องมาจากโดเมนนี้เท่านั้น
    pub hostname: Option<String>,
}

impl std::fmt::Debug for TurnstileConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TurnstileConfig")
            .field("site_key", &self.site_key)
            .field("secret_key", &"[redacted]")
            .field("hostname", &self.hostname)
            .finish()
    }
}

/// เขียน Debug เอง (AppState ถือ Config และ derive Debug) — DATABASE_URL มีรหัส DB, รหัส admin เริ่มต้นเป็นข้อความธรรมดา
/// ห้ามหลุดลง log แม้มีคนเผลอ `{:?}` state ทั้งก้อน (เหมือน `Crypto` ที่ redact key ไว้แล้ว)
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("database_url", &"[redacted]")
            .field("bind_addr", &self.bind_addr)
            .field("session_idle_ttl", &self.session_idle_ttl)
            .field("session_absolute_ttl", &self.session_absolute_ttl)
            .field("initial_admin_username", &self.initial_admin_username)
            .field("initial_admin_password", &"[redacted]")
            .field("crypto", &self.crypto)
            .field("turnstile", &self.turnstile)
            .field(
                "sql_console_db_password",
                &self.sql_console_db_password.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = require("DATABASE_URL")?;

        let bind_addr_raw = require("BIND_ADDR")?;
        let bind_addr = bind_addr_raw.parse().map_err(|_| ConfigError::Invalid {
            name: "BIND_ADDR",
            value: bind_addr_raw,
        })?;

        let idle_hours_raw = require("SESSION_IDLE_TTL_HOURS")?;
        let idle_hours: i64 = idle_hours_raw.parse().map_err(|_| ConfigError::Invalid {
            name: "SESSION_IDLE_TTL_HOURS",
            value: idle_hours_raw.clone(),
        })?;
        let session_idle_ttl = Duration::try_hours(idle_hours).ok_or(ConfigError::Invalid {
            name: "SESSION_IDLE_TTL_HOURS",
            value: idle_hours_raw,
        })?;

        let absolute_days_raw = require("SESSION_ABSOLUTE_TTL_DAYS")?;
        let absolute_days: i64 = absolute_days_raw
            .parse()
            .map_err(|_| ConfigError::Invalid {
                name: "SESSION_ABSOLUTE_TTL_DAYS",
                value: absolute_days_raw.clone(),
            })?;
        let session_absolute_ttl =
            Duration::try_days(absolute_days).ok_or(ConfigError::Invalid {
                name: "SESSION_ABSOLUTE_TTL_DAYS",
                value: absolute_days_raw,
            })?;

        let initial_admin_username = require("INITIAL_ADMIN_USERNAME")?;
        let initial_admin_password = require("INITIAL_ADMIN_PASSWORD")?;

        let crypto = Crypto::from_hex_keys(
            &require("DATA_ENCRYPTION_KEY")?,
            &require("BLIND_INDEX_KEY")?,
        )
        .map_err(|_| ConfigError::InvalidSecret("DATA_ENCRYPTION_KEY/BLIND_INDEX_KEY"))?;

        let turnstile = turnstile_config(
            optional("TURNSTILE_SITE_KEY"),
            optional("TURNSTILE_SECRET_KEY"),
            optional("TURNSTILE_HOSTNAME"),
        )?;

        let sql_console_db_password = sql_console_password(optional("SQL_CONSOLE_DB_PASSWORD"))?;

        Ok(Config {
            database_url,
            bind_addr,
            session_idle_ttl,
            session_absolute_ttl,
            initial_admin_username,
            initial_admin_password,
            crypto,
            turnstile,
            sql_console_db_password,
        })
    }
}

/// ไม่ตั้ง = ปิดหน้า SQL, สั้นเกิน = ล้มตอนเริ่ม (ไม่แนบค่าลงใน error)
fn sql_console_password(value: Option<String>) -> Result<Option<String>, ConfigError> {
    match value {
        Some(password) if password.chars().count() < MIN_SQL_CONSOLE_PASSWORD_CHARS => {
            Err(ConfigError::InvalidSecret("SQL_CONSOLE_DB_PASSWORD"))
        }
        other => Ok(other),
    }
}

fn require(name: &'static str) -> Result<String, ConfigError> {
    std::env::var(name).map_err(|_| ConfigError::Missing(name))
}

/// ตัวแปรที่ไม่บังคับ — ค่าว่าง (`NAME=` ใน .env) ถือว่าไม่ได้ตั้ง
fn optional(name: &'static str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// ตั้ง key ครบคู่ = เปิด, ไม่ตั้งทั้งคู่ = ปิด, ตั้งตัวเดียว = ล้มตอนเริ่ม (กันคิดว่าเปิดแล้วแต่จริง ๆ ปิดอยู่)
fn turnstile_config(
    site_key: Option<String>,
    secret_key: Option<String>,
    hostname: Option<String>,
) -> Result<Option<TurnstileConfig>, ConfigError> {
    match (site_key, secret_key) {
        (Some(site_key), Some(secret_key)) => Ok(Some(TurnstileConfig {
            site_key,
            secret_key,
            hostname,
        })),
        (None, None) => Ok(None),
        (Some(_), None) => Err(ConfigError::Missing("TURNSTILE_SECRET_KEY")),
        (None, Some(_)) => Err(ConfigError::Missing("TURNSTILE_SITE_KEY")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_hides_secrets() {
        let config = Config {
            database_url: "postgres://my_his:db-secret-123@db:5432/my_his".to_string(),
            bind_addr: "127.0.0.1:8080".parse().unwrap(),
            session_idle_ttl: Duration::hours(8),
            session_absolute_ttl: Duration::days(30),
            initial_admin_username: "admin".to_string(),
            initial_admin_password: "admin-secret-456".to_string(),
            crypto: Crypto::from_hex_keys(&"a".repeat(64), &"b".repeat(64)).unwrap(),
            turnstile: Some(TurnstileConfig {
                site_key: "public-site-key".to_string(),
                secret_key: "turnstile-secret-789".to_string(),
                hostname: None,
            }),
            sql_console_db_password: Some("sql-console-secret-012".to_string()),
        };
        let printed = format!("{config:?}");
        assert!(!printed.contains("db-secret-123"), "{printed}");
        assert!(!printed.contains("admin-secret-456"), "{printed}");
        assert!(!printed.contains("turnstile-secret-789"), "{printed}");
        assert!(!printed.contains("sql-console-secret-012"), "{printed}");
        assert!(printed.contains("[redacted]"));
    }

    #[test]
    fn sql_console_password_is_optional_but_must_be_long() {
        assert_eq!(sql_console_password(None).unwrap(), None);
        let long = "0123456789abcdef".to_string();
        assert_eq!(
            sql_console_password(Some(long.clone())).unwrap(),
            Some(long)
        );
        assert!(matches!(
            sql_console_password(Some("short".to_string())),
            Err(ConfigError::InvalidSecret("SQL_CONSOLE_DB_PASSWORD"))
        ));
    }

    #[test]
    fn turnstile_needs_both_keys_or_neither() {
        let key = |value: &str| Some(value.to_string());
        let enabled = turnstile_config(key("site"), key("secret"), key("my-his.example"))
            .unwrap()
            .unwrap();
        assert_eq!(enabled.site_key, "site");
        assert_eq!(enabled.hostname.as_deref(), Some("my-his.example"));
        assert!(turnstile_config(None, None, None).unwrap().is_none());
        assert!(matches!(
            turnstile_config(key("site"), None, None),
            Err(ConfigError::Missing("TURNSTILE_SECRET_KEY"))
        ));
        assert!(matches!(
            turnstile_config(None, key("secret"), None),
            Err(ConfigError::Missing("TURNSTILE_SITE_KEY"))
        ));
    }
}
