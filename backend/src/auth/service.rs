//! กฎทั้งหมด: lockout, ตรวจรหัสผ่าน, ออก/เพิกถอน session (docs/auth-design.md ข้อ 6-9)
//! ห้าม import ชนิดข้อมูลของ Axum — คืนชนิดโดเมนหรือ AuthError แล้วให้ handler แปลงเป็น HTTP
//! เวลาเป็นพารามิเตอร์เสมอ ไม่เรียก Utc::now() ในนี้ตรง ๆ (docs/rust-guidelines.md ข้อ 14)

use std::net::IpAddr;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::auth::audit_budget::{AuditBudget, Take, UNKNOWN_NAME_AUDIT_PER_MINUTE};
use crate::auth::ip_budget::{IpBudget, LOGIN_FAILURES_PER_IP};
use crate::auth::repository::{self, AuditEntry, RepositoryError, Reservation};
use crate::auth::session;
use crate::auth::user_budget::{UserBudget, LOGINS_PER_USER};
use crate::config::Config;
use crate::domain::{username, Permission, User, UserId};
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
    /// มี login ค้างคิว argon2 เต็ม — ปฏิเสธก่อนจองครั้งตรวจ (ไม่นับเป็นครั้งที่ผิด) ลองใหม่ภายหลัง
    #[error("server busy")]
    Busy,
    /// IP นี้ login ไม่สำเร็จครบเพดานของหน้าต่างแล้ว (docs/auth-design.md ข้อ 9.2) — ปฏิเสธก่อนเรียก Turnstile/argon2
    /// ตอบเหมือนกันไม่ว่าชื่อมีในระบบหรือไม่ (นับตาม IP อย่างเดียว) — หรือบัญชีนี้ login **สำเร็จ** ครบเพดานของหน้าต่างแล้ว
    /// (ข้อ 9.3; หลังรหัสถูกและก่อนสร้าง session) ข้อความเดียวกันเพราะหน้า login แสดงแบบเดียวกัน
    #[error("too many attempts")]
    TooManyAttempts { until: DateTime<Utc> },
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

    /// จองครั้งตรวจรหัสหนึ่งครั้ง **ก่อน verify** (atomic ในฐานข้อมูล) คืนเลขครั้งที่จองได้ (1..=MAX)
    ///
    /// ล็อกอยู่ หรือรอบนี้ใช้ครบ MAX ครั้งไปแล้ว (รวมครั้งที่ยังรอคิว argon2 อยู่) = 429 โดยไม่ verify — คำขอพร้อมกันจึงผ่านด่าน
    /// verify ได้ไม่เกิน MAX ครั้งต่อรอบ ไม่ว่าจะรอคิวนานแค่ไหน (docs/auth-design.md ข้อ 9)
    async fn reserve(&self, pool: &PgPool, now: DateTime<Utc>) -> Result<i32, AuthError> {
        let lock_until = now + lockout_duration();
        let reservation = match self {
            ThrottleKey::User(id) => {
                repository::reserve_user_attempt(pool, *id, now, MAX_FAILED_ATTEMPTS, lock_until)
                    .await?
            }
            ThrottleKey::Unknown(hash) => {
                repository::reserve_unknown_attempt(
                    pool,
                    hash,
                    now,
                    MAX_FAILED_ATTEMPTS,
                    lock_until,
                )
                .await?
            }
        };
        match reservation {
            Reservation::Locked { until } => Err(AuthError::AccountLocked { until }),
            Reservation::Granted {
                attempt,
                locked_until,
            } if attempt > MAX_FAILED_ATTEMPTS => Err(AuthError::AccountLocked {
                until: locked_until.unwrap_or(lock_until),
            }),
            Reservation::Granted { attempt, .. } => Ok(attempt),
        }
    }

    /// ครั้งที่จองไว้ (`attempt`) verify ไม่ผ่าน — ครั้งสุดท้ายของรอบ (MAX) ตั้งล็อก คืน error ที่ต้องตอบ
    /// (401 พร้อมจำนวนที่เหลือ หรือ 429) ตัวนับเพิ่มไปตอนจองแล้ว จึงไม่เขียนเพิ่ม
    async fn reject(
        &self,
        pool: &PgPool,
        attempt: i32,
        now: DateTime<Utc>,
    ) -> Result<AuthError, RepositoryError> {
        if attempt < MAX_FAILED_ATTEMPTS {
            return Ok(AuthError::LoginRejected {
                remaining_attempts: MAX_FAILED_ATTEMPTS - attempt,
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

/// เพดานในหน่วยความจำของ login ทั้งสามชนิด — รวมเป็น struct เดียวเพื่อไม่ให้พารามิเตอร์ของ `login` ล้น (clippy::too_many_arguments)
#[derive(Clone, Copy)]
pub struct LoginBudgets<'a> {
    /// เพดานแถว audit ของ login ที่ล้มเหลวด้วยชื่อที่ไม่มีในระบบ (docs/auth-design.md ข้อ 9)
    pub unknown_name_audit: &'a AuditBudget,
    /// เพดาน login ที่ไม่สำเร็จต่อ IP (ข้อ 9.2)
    pub ip: &'a IpBudget,
    /// เพดาน login ที่ **สำเร็จ** ต่อผู้ใช้ (ข้อ 9.3)
    pub success: &'a UserBudget,
}

pub async fn login(
    pool: &PgPool,
    dummy_password_hash: &str,
    config: &Config,
    turnstile: Option<&Turnstile>,
    budgets: LoginBudgets<'_>,
    params: LoginParams<'_>,
) -> Result<LoginSuccess, AuthError> {
    let ip_budget = budgets.ip;
    // ด่านแรกสุดของทุกอย่าง (ก่อน Turnstile ที่เรียกออก Cloudflare และก่อนคิว argon2): จองโควตาของ IP นี้ — ชนเพดานแล้วปฏิเสธทันที
    let ticket = match ip_budget.reserve(params.client.ip, params.now) {
        Ok(ticket) => ticket,
        Err(blocked) => {
            if blocked.first {
                if let Some(ip) = params.client.ip {
                    tracing::warn!(
                        %ip,
                        limit = LOGIN_FAILURES_PER_IP,
                        "login blocked: too many unsuccessful attempts from this address"
                    );
                }
            }
            return Err(AuthError::TooManyAttempts {
                until: blocked.until,
            });
        }
    };

    let result = login_attempt(
        pool,
        dummy_password_hash,
        config,
        turnstile,
        budgets,
        params,
    )
    .await;

    // คืนโควตาเมื่อไม่ใช่ความพยายามที่ผิด: สำเร็จ หรือพังเพราะฝั่งเรา (503 / error ภายใน) ที่เหลือ (401, 429, captcha ไม่ผ่าน) นับ
    // ถ้าคำขอถูกยกเลิกกลางทาง (future ถูก drop) จะไม่มาถึงตรงนี้ → ไม่คืน (นับเป็นผิดไว้ก่อน)
    if matches!(
        result,
        Ok(_)
            | Err(AuthError::Busy
                | AuthError::CaptchaUnavailable
                | AuthError::Repository(_)
                | AuthError::Password(_))
    ) {
        ip_budget.refund(ticket);
    }
    result
}

async fn login_attempt(
    pool: &PgPool,
    dummy_password_hash: &str,
    config: &Config,
    turnstile: Option<&Turnstile>,
    budgets: LoginBudgets<'_>,
    params: LoginParams<'_>,
) -> Result<LoginSuccess, AuthError> {
    // ขั้นแรกสุดของการตรวจตัวตน ก่อนค้นผู้ใช้/เช็กล็อก: ไม่ผ่านแล้วตอบเหมือนกันทุกชื่อ ไม่แตะตัวนับ และไม่เปลือง argon2 กับบอท
    if let Some(turnstile) = turnstile {
        verify_turnstile(turnstile, params.turnstile_token, params.client.ip).await?;
    }

    let normalized_username = params.username.trim().to_lowercase();
    // ชื่อที่ไม่ตรงรูปแบบที่เก็บได้ (domain::username) ไม่มีทางมีอยู่ — ข้ามการค้นหาแล้วเดินเส้นทาง ‘ชื่อที่ไม่มีในระบบ’ เดิมทุกอย่าง
    // (verify กับ dummy hash, ตัวนับ, 401 เหมือนกัน จึงไม่เป็น oracle ของชื่อ) ไม่ส่งอักขระอย่าง NUL เข้า Postgres ที่ปฏิเสธด้วย error
    // ซึ่งกลายเป็น 500 ที่ไม่ถูกนับเพดาน (คืนโควตา IP เพราะเป็น error ภายใน)
    let existing = if username::is_well_formed(&normalized_username) {
        repository::find_user_by_username(pool, &normalized_username).await?
    } else {
        None
    };
    // ชื่อที่ไม่มีในระบบใช้แค่ hash ในตัวนับ (login_throttle) — ชื่อที่พิมพ์มาอาจเป็นรหัสผ่านที่พิมพ์ผิดช่อง
    let throttle = match &existing {
        Some(user) => ThrottleKey::User(user.id),
        None => ThrottleKey::Unknown(session::hash_token(&normalized_username)),
    };

    // ถูกล็อกอยู่ → ตอบ 429 ทันที ไม่เสียเวลาเข้าคิว argon2 (ยังไม่ถือเป็นความพยายามผิดครั้งใหม่)
    // ชื่อที่ไม่มีในระบบก็ถูกล็อกได้เหมือนกัน — อ่านอย่างเดียวเพื่อตัดทางลัด; ตัวตัดสินจริงคือการจองด้านล่าง
    let user_lock = existing.as_ref().and_then(|user| user.locked_until);
    if let Some(until) = throttle.locked_until(pool, user_lock).await? {
        if until > params.now {
            return Err(AuthError::AccountLocked { until });
        }
    }

    // เข้าคิว argon2 ของ login ก่อนจองครั้งตรวจ: คิวเต็มแล้วปฏิเสธ (503) โดยไม่ลดโอกาสผิดของใคร
    let admission = password::admit_login().await.map_err(|err| match err {
        password::PasswordError::Busy => AuthError::Busy,
        other => AuthError::Password(other),
    })?;
    // จองครั้งตรวจแบบ atomic **ก่อน verify** — ไม่ใช่เช็กล็อกจากภาพเก่าแล้วค่อยนับตอนผิด (ไม่งั้นคำขอที่รอคิว argon2 อยู่พร้อมกัน
    // ผ่านเช็กล็อกได้หมดก่อนที่ครั้งที่ 5 จะเขียนล็อก)
    let attempt = throttle.reserve(pool, params.now).await?;

    let hash_to_check = existing
        .as_ref()
        .map(|u| u.password_hash.clone())
        .unwrap_or_else(|| dummy_password_hash.to_string());
    let password_ok =
        password::verify_admitted(admission, params.password.to_string(), hash_to_check).await?;

    let Some(user) = existing else {
        // ตัวนับของชื่อที่ไม่มีในระบบอยู่ใน login_throttle แล้ว (key เป็น hash) — audit ไม่เก็บอะไรที่คำนวณจากชื่อที่พิมพ์
        // และมีเพดานต่อนาที (ชื่อสุ่มสร้างได้ไม่จำกัด) เกินแล้วข้ามแถว audit ไม่ข้ามตัวนับ
        record_unknown_name_failure(pool, budgets.unknown_name_audit, params.client, params.now)
            .await?;
        return Err(throttle.reject(pool, attempt, params.now).await?);
    };

    if !password_ok || !user.is_active {
        record_login_failure(pool, Some(user.id), Some(&user.username), params.client).await?;
        return Err(throttle.reject(pool, attempt, params.now).await?);
    }

    // จุดที่เว้นไว้สำหรับ MFA ในอนาคต (docs/auth-design.md ข้อ 6 ข้อ 8): ระหว่างตรงนี้กับก่อน
    // สร้าง session จริงด้านล่าง

    // เพดานการเข้าสำเร็จต่อผู้ใช้ (ข้อ 9.3): จองก่อนสร้าง session — ผู้ถือรหัสผ่านที่ถูกต้องวนล็อกอินรัวไม่ได้ (session, แถว audit
    // `login_succeeded`, sweep session หมดอายุ โตตามจำนวนครั้ง) เกินเพดาน: ล้างตัวนับผิดที่จองไว้ก่อน verify (ไม่งั้นคำขอที่ 429
    // เหล่านี้สะสมจนล็อกบัญชี) แล้วตอบ 429 ไม่สร้าง session ไม่เขียน audit
    if let Err(blocked) = budgets.success.reserve(user.id, params.now) {
        if blocked.first {
            tracing::warn!(
                user_id = user.id.0,
                limit = LOGINS_PER_USER,
                "login blocked: too many successful logins for this account"
            );
        }
        repository::reset_failed_attempts(pool, user.id).await?;
        return Err(AuthError::TooManyAttempts {
            until: blocked.until,
        });
    }

    let token = session::generate_token();
    let token_hash = session::hash_token(&token);
    let idle_expires_at = params.now + config.session_idle_ttl;
    let absolute_expires_at = params.now + config.session_absolute_ttl;

    // สร้าง session ผูกกับ hash ที่เพิ่งตรวจ: ถ้ารหัสผ่านถูกรีเซ็ต/เปลี่ยน หรือบัญชีถูกปิด ระหว่างที่ login นี้รอคิว argon2
    // จะไม่ได้ session (ตอบเหมือน login ไม่ผ่าน) — การเพิกถอนลบได้แค่ session ที่มีอยู่ตอนนั้น
    let issued = repository::insert_session_if_credential_unchanged(
        pool,
        &token_hash,
        user.id,
        &user.password_hash,
        idle_expires_at,
        absolute_expires_at,
        params.client.user_agent,
    )
    .await?;
    if !issued {
        record_login_failure(pool, Some(user.id), Some(&user.username), params.client).await?;
        return Err(throttle.reject(pool, attempt, params.now).await?);
    }
    repository::reset_failed_attempts(pool, user.id).await?;
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

/// target_id = username ของบัญชีที่มีอยู่ — ชื่อที่ไม่มีในระบบเป็น NULL: ชื่อที่พิมพ์มาอาจเป็นรหัสผ่านที่พิมพ์ผิดช่อง
/// จึงไม่เก็บทั้งดิบและ hash (ตัวนับอยู่ใน login_throttle แล้ว; แถวก่อน 2026-10-12 ยังเป็นชื่อดิบ
/// และแถวก่อนหน้าการแก้นี้เป็น SHA-256 — ดู docs/auth-design.md) API ประวัติการเข้าออกไม่อ่านคอลัมน์นี้
async fn record_login_failure(
    pool: &PgPool,
    actor_user_id: Option<UserId>,
    target_username: Option<&str>,
    client: ClientInfo<'_>,
) -> Result<(), RepositoryError> {
    repository::insert_audit_log(
        pool,
        AuditEntry {
            actor_user_id,
            action: "login_failed",
            target_type: Some("user"),
            target_id: target_username,
            metadata: client.audit_metadata(),
            ip: client.ip,
        },
    )
    .await
}

/// audit `login_failed` ของชื่อที่ไม่มีในระบบ ภายใต้เพดานต่อนาที (`AuditBudget`) — เกินเพดานข้ามแถว audit และแจ้งใน log ครั้งเดียวต่อหน้าต่าง
async fn record_unknown_name_failure(
    pool: &PgPool,
    budget: &AuditBudget,
    client: ClientInfo<'_>,
    now: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    match budget.try_take(now) {
        Take::Recorded => record_login_failure(pool, None, None, client).await,
        Take::FirstSuppressed => {
            tracing::warn!(
                limit = UNKNOWN_NAME_AUDIT_PER_MINUTE,
                "audit rows for logins with unknown names are being dropped (per-minute cap); login_throttle still counts them"
            );
            Ok(())
        }
        Take::Suppressed => Ok(()),
    }
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

    // จองครั้งตรวจก่อน verify (atomic) — การตรวจรหัสปัจจุบันต้องทนการเดาซ้ำไม่น้อยกว่าประตู login: ครบเพดานแล้ว
    // ลบ session นี้ (ไม่ล็อกบัญชี ผู้ถือ session ที่ถูกขโมยจะได้ไม่ใช้ล็อกเจ้าของ)
    let checks = repository::register_password_check(pool, current_token_hash)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;
    if checks > MAX_FAILED_ATTEMPTS {
        repository::delete_session(pool, current_token_hash).await?;
        return Err(AuthError::InvalidCredentials);
    }

    let ok =
        password::verify_async(current_password.to_string(), user.password_hash.clone()).await?;

    if !ok {
        repository::insert_audit_log(
            pool,
            AuditEntry {
                actor_user_id: Some(user_id),
                action: "password_change_failed",
                target_type: Some("user"),
                target_id: None,
                metadata: None,
                ip: None,
            },
        )
        .await?;
        if checks >= MAX_FAILED_ATTEMPTS {
            repository::delete_session(pool, current_token_hash).await?;
        }
        return Err(AuthError::InvalidCredentials);
    }
    repository::reset_password_checks(pool, current_token_hash).await?;
    // ตรวจหลัง verify รหัสเดิมผ่านแล้วเท่านั้น (ผู้เรียกรู้รหัสเดิมอยู่แล้ว เทียบตรง ๆ ไม่รั่วอะไร)
    if new_password == current_password {
        return Err(AuthError::PasswordUnchanged);
    }

    let new_hash = password::hash_async(new_password.to_string()).await?;

    // เขียนทับเฉพาะเมื่อ hash ในฐานยังเป็นตัวที่เพิ่งตรวจ: ถ้าแอดมินรีเซ็ต/ปิดบัญชีระหว่างรอคิว argon2 จะไม่เขียนทับผลนั้น
    // เพิกถอน session อื่นทั้งหมดยกเว้นอันปัจจุบัน และบันทึก password_changed ในทรานแซกชันเดียวกัน
    // (docs/auth-design.md ข้อ 8)
    let changed = repository::change_password_if_unchanged(
        pool,
        user_id,
        &user.password_hash,
        &new_hash,
        current_token_hash,
    )
    .await?;
    if !changed {
        return Err(AuthError::InvalidCredentials);
    }

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
