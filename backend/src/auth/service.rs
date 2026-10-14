//! กฎทั้งหมด: lockout, ตรวจรหัสผ่าน, ออก/เพิกถอน session (docs/auth-design.md ข้อ 6-9)
//! ห้าม import ชนิดข้อมูลของ Axum — คืนชนิดโดเมนหรือ AuthError แล้วให้ handler แปลงเป็น HTTP
//! เวลาเป็นพารามิเตอร์เสมอ ไม่เรียก Utc::now() ในนี้ตรง ๆ (docs/rust-guidelines.md ข้อ 14)

use std::net::IpAddr;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::auth::repository::{self, AuditEntry, RepositoryError};
use crate::auth::session;
use crate::config::Config;
use crate::domain::{Permission, User, UserId};
use crate::password;
use crate::turnstile::{Turnstile, TurnstileError};

/// login ผิดติดกันได้กี่ครั้งก่อนล็อก (หน้า login แสดง "เหลือ X/5 ครั้ง")
pub const MAX_FAILED_ATTEMPTS: i32 = 5;

/// action ที่หน้า login ตั้งให้ widget Turnstile — token ของ widget อื่น (ถ้ามีในอนาคต) ใช้ login ไม่ได้
pub const TURNSTILE_ACTION: &str = "login";

fn lockout_duration() -> chrono::Duration {
    chrono::Duration::seconds(60)
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    /// login ไม่ผ่าน (ชื่อไม่มี/รหัสผิด/บัญชีปิด — ตอบเหมือนกันหมด) ยังเหลือโอกาสอีก `remaining_attempts` ครั้งก่อนล็อก
    #[error("invalid credentials")]
    LoginRejected { remaining_attempts: i32 },
    #[error("account temporarily locked")]
    AccountLocked { until: DateTime<Utc> },
    /// ไม่มี token Turnstile หรือ Cloudflare บอกว่าไม่ผ่าน — ยังไม่ได้ดูชื่อผู้ใช้ ไม่นับเป็นครั้งที่ผิด
    #[error("captcha failed")]
    CaptchaFailed,
    /// ติดต่อ Cloudflare ไม่ได้ — ปฏิเสธ login (fail-closed) ไม่นับเป็นครั้งที่ผิด
    #[error("captcha unavailable")]
    CaptchaUnavailable,
    /// เปลี่ยนรหัสผ่านเป็นค่าเดิม — เช่นใส่รหัสชั่วคราวที่แอดมินเห็นแล้วซ้ำ ซึ่งทำให้การเปลี่ยนรหัสไม่มีความหมาย
    #[error("new password must be different from current password")]
    PasswordUnchanged,
    #[error("required seed data missing: {0}")]
    MissingSeedRole(&'static str),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Password(#[from] password::PasswordError),
}

pub struct LoginSuccess {
    pub user: User,
    pub permissions: Vec<Permission>,
    pub session_token: String,
}

/// ข้อมูลผู้ใช้จาก request header สำหรับประวัติการเข้าออก (IP จาก X-Forwarded-For ของ proxy, User-Agent) —
/// handler อ่าน header แล้วส่งมา service ไม่รู้จัก HTTP ไม่เก็บ header อื่น (cookie/token ห้ามเด็ดขาด)
#[derive(Debug, Clone, Copy, Default)]
pub struct ClientInfo<'a> {
    pub ip: Option<IpAddr>,
    pub user_agent: Option<&'a str>,
}

impl ClientInfo<'_> {
    /// User-Agent เก็บใน audit_log.metadata (ไม่ใช่ความลับ/ข้อมูลผู้ป่วย) IP เก็บคอลัมน์ ip
    fn audit_metadata(&self) -> Option<serde_json::Value> {
        self.user_agent
            .map(|user_agent| serde_json::json!({ "userAgent": user_agent }))
    }
}

/// ขั้นตอน login แบบ timing-safe ตาม docs/auth-design.md ข้อ 6 — ไม่พบผู้ใช้ก็ยังคำนวณ argon2
/// กับ dummy hash เพื่อให้เวลาตอบกลับใกล้เคียงกับกรณีรหัสผ่านผิดจริง
pub struct LoginParams<'a> {
    pub username: &'a str,
    pub password: &'a str,
    /// token จาก widget Turnstile (ห้าม log) — ใช้เมื่อเปิด Turnstile เท่านั้น
    pub turnstile_token: Option<&'a str>,
    pub client: ClientInfo<'a>,
    pub now: DateTime<Utc>,
}

/// ไม่บันทึก audit_log (บอทยิงรัวจะถมประวัติ) — log แค่ error-codes ของ Cloudflare ห้าม log token
async fn verify_turnstile(
    turnstile: &Turnstile,
    token: Option<&str>,
    ip: Option<IpAddr>,
) -> Result<(), AuthError> {
    let Some(token) = token.filter(|token| !token.is_empty()) else {
        tracing::warn!("login without turnstile token");
        return Err(AuthError::CaptchaFailed);
    };
    match turnstile.verify(token, ip, TURNSTILE_ACTION).await {
        Ok(()) => Ok(()),
        Err(TurnstileError::Rejected(codes)) => {
            tracing::warn!(?codes, "turnstile rejected login");
            Err(AuthError::CaptchaFailed)
        }
        Err(err @ TurnstileError::Unavailable(_)) => {
            tracing::warn!(error = %err, "turnstile check failed");
            Err(AuthError::CaptchaUnavailable)
        }
    }
}

/// ตัวนับ login ผิดของ username หนึ่ง — ชื่อที่มีในระบบนับใน `users`, ชื่อที่ไม่มีนับใน `login_throttle` ด้วย
/// SHA-256 ของชื่อ กติกาเดียวกันทุกอย่าง (docs/auth-design.md ข้อ 9) คนลองจึงแยกไม่ออกว่าชื่อไหนมีจริง
enum ThrottleKey {
    User(UserId),
    Unknown(String),
}

impl ThrottleKey {
    async fn locked_until(
        &self,
        pool: &PgPool,
        user_lock: Option<DateTime<Utc>>,
    ) -> Result<Option<DateTime<Utc>>, RepositoryError> {
        match self {
            ThrottleKey::User(_) => Ok(user_lock),
            ThrottleKey::Unknown(hash) => repository::find_unknown_lock(pool, hash).await,
        }
    }

    /// บันทึกความผิดหนึ่งครั้ง — ครบ MAX แล้วล็อก คืน error ที่ต้องตอบ (401 พร้อมจำนวนที่เหลือ หรือ 429)
    async fn reject(
        &self,
        pool: &PgPool,
        now: DateTime<Utc>,
    ) -> Result<AuthError, RepositoryError> {
        let count = match self {
            ThrottleKey::User(id) => repository::register_user_failure(pool, *id, now).await?,
            ThrottleKey::Unknown(hash) => {
                repository::register_unknown_failure(pool, hash, now).await?
            }
        };
        if count < MAX_FAILED_ATTEMPTS {
            return Ok(AuthError::LoginRejected {
                remaining_attempts: MAX_FAILED_ATTEMPTS - count,
            });
        }
        let until = now + lockout_duration();
        match self {
            ThrottleKey::User(id) => repository::lock_account(pool, *id, until).await?,
            ThrottleKey::Unknown(hash) => repository::lock_unknown(pool, hash, until).await?,
        }
        Ok(AuthError::AccountLocked { until })
    }
}

pub async fn login(
    pool: &PgPool,
    dummy_password_hash: &str,
    config: &Config,
    turnstile: Option<&Turnstile>,
    params: LoginParams<'_>,
) -> Result<LoginSuccess, AuthError> {
    // ขั้นแรกสุด ก่อนค้นผู้ใช้/เช็กล็อก: ไม่ผ่านแล้วตอบเหมือนกันทุกชื่อ ไม่แตะตัวนับ และไม่เปลือง argon2 กับบอท
    if let Some(turnstile) = turnstile {
        verify_turnstile(turnstile, params.turnstile_token, params.client.ip).await?;
    }

    let normalized_username = params.username.trim().to_lowercase();
    let existing = repository::find_user_by_username(pool, &normalized_username).await?;
    // ชื่อที่ไม่มีในระบบใช้แค่ hash (ตัวนับ + ประวัติ) — ชื่อที่พิมพ์มาอาจเป็นรหัสผ่านที่พิมพ์ผิดช่อง
    let unknown_name_hash = session::hash_token(&normalized_username);
    let throttle = match &existing {
        Some(user) => ThrottleKey::User(user.id),
        None => ThrottleKey::Unknown(unknown_name_hash.clone()),
    };

    // ถูกล็อกอยู่ → ตอบ 429 ทันที ไม่เสียเวลาคำนวณ argon2 (ยังไม่ถือเป็นความพยายามผิดครั้งใหม่)
    // ชื่อที่ไม่มีในระบบก็ถูกล็อกได้เหมือนกัน
    let user_lock = existing.as_ref().and_then(|user| user.locked_until);
    if let Some(until) = throttle.locked_until(pool, user_lock).await? {
        if until > params.now {
            return Err(AuthError::AccountLocked { until });
        }
    }

    let hash_to_check = existing
        .as_ref()
        .map(|u| u.password_hash.clone())
        .unwrap_or_else(|| dummy_password_hash.to_string());
    let password_ok = password::verify_async(params.password.to_string(), hash_to_check).await?;

    let Some(user) = existing else {
        record_login_failure(pool, None, &unknown_name_hash, params.client).await?;
        return Err(throttle.reject(pool, params.now).await?);
    };

    if !password_ok || !user.is_active {
        record_login_failure(pool, Some(user.id), &user.username, params.client).await?;
        return Err(throttle.reject(pool, params.now).await?);
    }

    // จุดที่เว้นไว้สำหรับ MFA ในอนาคต (docs/auth-design.md ข้อ 6 ข้อ 8): ระหว่างตรงนี้กับก่อน
    // สร้าง session จริงด้านล่าง

    repository::reset_failed_attempts(pool, user.id).await?;

    let token = session::generate_token();
    let token_hash = session::hash_token(&token);
    let idle_expires_at = params.now + config.session_idle_ttl;
    let absolute_expires_at = params.now + config.session_absolute_ttl;

    repository::insert_session(
        pool,
        &token_hash,
        user.id,
        idle_expires_at,
        absolute_expires_at,
        params.client.user_agent,
    )
    .await?;
    repository::delete_expired_sessions(pool, params.now).await?;

    repository::insert_audit_log(
        pool,
        AuditEntry {
            actor_user_id: Some(user.id),
            action: "login_succeeded",
            target_type: Some("user"),
            target_id: Some(&user.username),
            metadata: params.client.audit_metadata(),
            ip: params.client.ip,
        },
    )
    .await?;

    let permissions = repository::fetch_permissions_for_user(pool, user.id).await?;

    Ok(LoginSuccess {
        user: User {
            id: user.id,
            public_id: user.public_id,
            username: user.username,
            display_name: user.display_name,
            must_change_password: user.must_change_password,
        },
        permissions,
        session_token: token,
    })
}

/// target_id = username ของบัญชีที่มีอยู่ หรือ SHA-256 ของชื่อที่พิมพ์มาถ้าไม่มีในระบบ (อาจเป็นรหัสผ่านที่พิมพ์ผิดช่อง
/// จึงไม่เก็บดิบ — แถวก่อน 2026-10-12 ยังเป็นชื่อดิบ) API ประวัติการเข้าออกไม่อ่านคอลัมน์นี้
async fn record_login_failure(
    pool: &PgPool,
    actor_user_id: Option<UserId>,
    target_username: &str,
    client: ClientInfo<'_>,
) -> Result<(), RepositoryError> {
    repository::insert_audit_log(
        pool,
        AuditEntry {
            actor_user_id,
            action: "login_failed",
            target_type: Some("user"),
            target_id: Some(target_username),
            metadata: client.audit_metadata(),
            ip: client.ip,
        },
    )
    .await
}

/// ลบ session ของ cookie นี้ แล้วบันทึก `logout` ของเจ้าของ — session หมดอายุ/ไม่พบแล้วไม่บันทึก (ไม่มีใครให้ผูก)
pub async fn logout(
    pool: &PgPool,
    token_hash: &str,
    client: ClientInfo<'_>,
) -> Result<(), AuthError> {
    if let Some(user_id) = repository::delete_session(pool, token_hash).await? {
        repository::insert_audit_log(
            pool,
            AuditEntry {
                actor_user_id: Some(user_id),
                action: "logout",
                target_type: Some("user"),
                target_id: None,
                metadata: client.audit_metadata(),
                ip: client.ip,
            },
        )
        .await?;
    }
    Ok(())
}

pub async fn logout_all(
    pool: &PgPool,
    user_id: UserId,
    client: ClientInfo<'_>,
) -> Result<u64, AuthError> {
    let count = repository::delete_all_sessions_for_user(pool, user_id).await?;
    repository::insert_audit_log(
        pool,
        AuditEntry {
            actor_user_id: Some(user_id),
            action: "logout_all",
            target_type: Some("user"),
            target_id: None,
            metadata: client.audit_metadata(),
            ip: client.ip,
        },
    )
    .await?;
    Ok(count)
}

pub async fn change_password(
    pool: &PgPool,
    user_id: UserId,
    current_password: &str,
    new_password: &str,
    current_token_hash: &str,
) -> Result<(), AuthError> {
    let user = repository::find_user_by_id(pool, user_id)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;

    let ok =
        password::verify_async(current_password.to_string(), user.password_hash.clone()).await?;

    if !ok {
        return Err(AuthError::InvalidCredentials);
    }
    // ตรวจหลัง verify รหัสเดิมผ่านแล้วเท่านั้น (ผู้เรียกรู้รหัสเดิมอยู่แล้ว เทียบตรง ๆ ไม่รั่วอะไร)
    if new_password == current_password {
        return Err(AuthError::PasswordUnchanged);
    }

    let new_hash = password::hash_async(new_password.to_string()).await?;

    repository::update_password_hash(pool, user_id, &new_hash).await?;

    // เพิกถอน session อื่นทั้งหมดยกเว้นอันปัจจุบัน (docs/auth-design.md ข้อ 8)
    repository::delete_all_sessions_for_user_except(pool, user_id, current_token_hash).await?;

    repository::insert_audit_log(
        pool,
        AuditEntry {
            actor_user_id: Some(user_id),
            action: "password_changed",
            target_type: Some("user"),
            target_id: None,
            metadata: None,
            ip: None,
        },
    )
    .await?;

    Ok(())
}

/// ตอนเริ่มระบบ ถ้ายังไม่มีผู้ใช้เลย สร้างบัญชี admin จาก config (docs/auth-design.md ข้อ 13)
/// role `admin` ต้องมีอยู่แล้วจาก seed migration — ถ้าไม่มีคือ migration ไม่ครบ ให้ main.rs ล้มระบบ
pub async fn bootstrap_first_admin(pool: &PgPool, config: &Config) -> Result<(), AuthError> {
    let count = repository::count_users(pool).await?;
    if count > 0 {
        return Ok(());
    }

    let role_id = repository::find_role_id_by_name(pool, "admin")
        .await?
        .ok_or(AuthError::MissingSeedRole("admin"))?;

    let hash = password::hash_async(config.initial_admin_password.clone()).await?;

    let user_id = repository::insert_user_with_role(
        pool,
        &config.initial_admin_username,
        &config.initial_admin_username,
        &hash,
        true,
        role_id,
    )
    .await?;

    repository::insert_audit_log(
        pool,
        AuditEntry {
            actor_user_id: Some(user_id),
            action: "user_created",
            target_type: Some("user"),
            target_id: Some(&config.initial_admin_username),
            metadata: None,
            ip: None,
        },
    )
    .await?;

    Ok(())
}

/// รวม permission จากทุก role ที่ผู้ใช้ถืออยู่ (docs/auth-design.md ข้อ 10) — ใช้ทั้งจาก extractor
/// (สร้าง CurrentUser) และ handler /auth/me
///
/// คืน None ถ้าไม่พบผู้ใช้ **หรือบัญชีถูกปิดใช้งาน** — extractor ตอบ 401 ให้ session ของบัญชีที่ปิดแล้ว
/// ใช้ต่อไม่ได้ทันที (docs/auth-design.md ข้อ 2, 7) แม้แถว session ยังค้างอยู่ใน DB
pub async fn current_user_with_permissions(
    pool: &PgPool,
    user_id: UserId,
) -> Result<Option<(User, Vec<Permission>)>, AuthError> {
    let Some(row) = repository::find_user_by_id(pool, user_id).await? else {
        return Ok(None);
    };
    if !row.is_active {
        return Ok(None);
    }
    let permissions = repository::fetch_permissions_for_user(pool, user_id).await?;
    Ok(Some((
        User {
            id: row.id,
            public_id: row.public_id,
            username: row.username,
            display_name: row.display_name,
            must_change_password: row.must_change_password,
        },
        permissions,
    )))
}
