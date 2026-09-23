//! กฎทั้งหมด: lockout, ตรวจรหัสผ่าน, ออก/เพิกถอน session (docs/auth-design.md ข้อ 6-9)
//! ห้าม import ชนิดข้อมูลของ Axum — คืนชนิดโดเมนหรือ AuthError แล้วให้ handler แปลงเป็น HTTP
//! เวลาเป็นพารามิเตอร์เสมอ ไม่เรียก Utc::now() ในนี้ตรง ๆ (docs/rust-guidelines.md ข้อ 14)

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::auth::repository::{self, AuditEntry, RepositoryError};
use crate::auth::session;
use crate::config::Config;
use crate::domain::{Permission, User, UserId};
use crate::password;

const MAX_FAILED_ATTEMPTS: i32 = 5;

fn lockout_duration() -> chrono::Duration {
    chrono::Duration::minutes(15)
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("account temporarily locked")]
    AccountLocked { until: DateTime<Utc> },
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

/// ขั้นตอน login แบบ timing-safe ตาม docs/auth-design.md ข้อ 6 — ไม่พบผู้ใช้ก็ยังคำนวณ argon2
/// กับ dummy hash เพื่อให้เวลาตอบกลับใกล้เคียงกับกรณีรหัสผ่านผิดจริง
pub struct LoginParams<'a> {
    pub username: &'a str,
    pub password: &'a str,
    pub user_agent: Option<&'a str>,
    pub now: DateTime<Utc>,
}

pub async fn login(
    pool: &PgPool,
    dummy_password_hash: &str,
    config: &Config,
    params: LoginParams<'_>,
) -> Result<LoginSuccess, AuthError> {
    let normalized_username = params.username.trim().to_lowercase();
    let existing = repository::find_user_by_username(pool, &normalized_username).await?;

    // ถูกล็อกอยู่ → ตอบ 429 ทันที ไม่เสียเวลาคำนวณ argon2 (ยังไม่ถือเป็นความพยายามผิดครั้งใหม่)
    if let Some(user) = &existing {
        if let Some(until) = user.locked_until {
            if until > params.now {
                return Err(AuthError::AccountLocked { until });
            }
        }
    }

    let hash_to_check = existing
        .as_ref()
        .map(|u| u.password_hash.clone())
        .unwrap_or_else(|| dummy_password_hash.to_string());
    let password_input = params.password.to_string();
    let password_ok =
        tokio::task::spawn_blocking(move || password::verify(&password_input, &hash_to_check))
            .await
            .map_err(|_| password::PasswordError::Hash)??;

    let Some(user) = existing else {
        record_login_failure(pool, None, &normalized_username).await?;
        return Err(AuthError::InvalidCredentials);
    };

    if !password_ok || !user.is_active {
        let new_count = repository::increment_failed_attempts(pool, user.id).await?;
        if new_count >= MAX_FAILED_ATTEMPTS {
            let until = params.now + lockout_duration();
            repository::lock_account(pool, user.id, until).await?;
        }
        record_login_failure(pool, Some(user.id), &user.username).await?;
        return Err(AuthError::InvalidCredentials);
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
        params.user_agent,
    )
    .await?;

    repository::insert_audit_log(
        pool,
        AuditEntry {
            actor_user_id: Some(user.id),
            action: "login_succeeded",
            target_type: Some("user"),
            target_id: Some(&user.username),
            metadata: None,
        },
    )
    .await?;

    let permissions = repository::fetch_permissions_for_user(pool, user.id).await?;

    Ok(LoginSuccess {
        user: User {
            id: user.id,
            username: user.username,
            display_name: user.display_name,
            must_change_password: user.must_change_password,
        },
        permissions,
        session_token: token,
    })
}

async fn record_login_failure(
    pool: &PgPool,
    actor_user_id: Option<UserId>,
    target_username: &str,
) -> Result<(), RepositoryError> {
    repository::insert_audit_log(
        pool,
        AuditEntry {
            actor_user_id,
            action: "login_failed",
            target_type: Some("user"),
            target_id: Some(target_username),
            metadata: None,
        },
    )
    .await
}

pub async fn logout(pool: &PgPool, token_hash: &str) -> Result<(), AuthError> {
    repository::delete_session(pool, token_hash).await?;
    Ok(())
}

pub async fn logout_all(pool: &PgPool, user_id: UserId) -> Result<u64, AuthError> {
    let count = repository::delete_all_sessions_for_user(pool, user_id).await?;
    repository::insert_audit_log(
        pool,
        AuditEntry {
            actor_user_id: Some(user_id),
            action: "logout_all",
            target_type: Some("user"),
            target_id: None,
            metadata: None,
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

    let current_password_owned = current_password.to_string();
    let stored_hash = user.password_hash.clone();
    let ok = tokio::task::spawn_blocking(move || {
        password::verify(&current_password_owned, &stored_hash)
    })
    .await
    .map_err(|_| password::PasswordError::Hash)??;

    if !ok {
        return Err(AuthError::InvalidCredentials);
    }

    let new_password_owned = new_password.to_string();
    let new_hash = tokio::task::spawn_blocking(move || password::hash(&new_password_owned))
        .await
        .map_err(|_| password::PasswordError::Hash)??;

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

    let password_owned = config.initial_admin_password.clone();
    let hash = tokio::task::spawn_blocking(move || password::hash(&password_owned))
        .await
        .map_err(|_| password::PasswordError::Hash)??;

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
        },
    )
    .await?;

    Ok(())
}

/// รวม permission จากทุก role ที่ผู้ใช้ถืออยู่ (docs/auth-design.md ข้อ 10) — ใช้ทั้งจาก extractor
/// (สร้าง CurrentUser) และ handler /auth/me
pub async fn current_user_with_permissions(
    pool: &PgPool,
    user_id: UserId,
) -> Result<Option<(User, Vec<Permission>)>, AuthError> {
    let Some(row) = repository::find_user_by_id(pool, user_id).await? else {
        return Ok(None);
    };
    let permissions = repository::fetch_permissions_for_user(pool, user_id).await?;
    Ok(Some((
        User {
            id: row.id,
            username: row.username,
            display_name: row.display_name,
            must_change_password: row.must_change_password,
        },
        permissions,
    )))
}
