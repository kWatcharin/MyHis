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

#[derive(Debug)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: SocketAddr,
    pub session_idle_ttl: Duration,
    pub session_absolute_ttl: Duration,
    pub initial_admin_username: String,
    pub initial_admin_password: String,
    pub crypto: Crypto,
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

        Ok(Config {
            database_url,
            bind_addr,
            session_idle_ttl,
            session_absolute_ttl,
            initial_admin_username,
            initial_admin_password,
            crypto,
        })
    }
}

fn require(name: &'static str) -> Result<String, ConfigError> {
    std::env::var(name).map_err(|_| ConfigError::Missing(name))
}
