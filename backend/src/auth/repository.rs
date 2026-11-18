//! query SQLx ต่อ users, sessions, roles, user_roles, role_permissions, audit_log, login_throttle
//! (ห้ามรู้เรื่อง HTTP) ใช้ query!/query_as! เพื่อตรวจตอน compile ห้ามประกอบ SQL ด้วย format!
//! (docs/rust-guidelines.md ข้อ 11) เขียน audit_log ตรงจากที่นี่ (ไม่เรียกโมดูล audit — โมดูล
//! ฟีเจอร์ไม่เรียกกันเอง, audit_log เป็นแค่ตารางที่ทุกโมดูลเขียนตรงได้)

use std::net::IpAddr;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{Permission, RoleId, UserId};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("unknown permission stored in database: {0}")]
    UnknownPermission(String),
}

pub struct UserRow {
    pub id: UserId,
    pub public_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub password_hash: String,
    pub is_active: bool,
    pub must_change_password: bool,
    pub failed_attempts: i32,
    pub locked_until: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for UserRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserRow")
            .field("id", &self.id)
            .field("public_id", &self.public_id)
            .field("username", &self.username)
            .field("display_name", &self.display_name)
            .field("password_hash", &"[redacted]")
            .field("is_active", &self.is_active)
            .field("must_change_password", &self.must_change_password)
            .field("failed_attempts", &self.failed_attempts)
            .field("locked_until", &self.locked_until)
            .finish()
    }
}

/// ข้อมูลผู้ใช้สำหรับสร้าง `CurrentUser` ทุก request — **ไม่มี** `password_hash` (ไม่ต้องใช้ จึงไม่ดึงมาค้างในหน่วยความจำ)
#[derive(Debug)]
pub struct ProfileRow {
    pub id: UserId,
    pub public_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub is_active: bool,
    pub must_change_password: bool,
}

#[derive(Debug)]
pub struct SessionRow {
    pub user_id: UserId,
    pub last_seen_at: DateTime<Utc>,
    pub idle_expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
}

pub async fn find_user_by_username(
    pool: &PgPool,
    username: &str,
) -> Result<Option<UserRow>, RepositoryError> {
    let row = sqlx::query_as!(
        UserRow,
        r#"select id as "id: UserId", uuid as public_id, username, display_name, password_hash, is_active,
           must_change_password, failed_attempts, locked_until
           from users where username = $1"#,
        username
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_user_by_id(
    pool: &PgPool,
    user_id: UserId,
) -> Result<Option<UserRow>, RepositoryError> {
    let row = sqlx::query_as!(
        UserRow,
        r#"select id as "id: UserId", uuid as public_id, username, display_name, password_hash, is_active,
           must_change_password, failed_attempts, locked_until
           from users where id = $1"#,
        user_id.0
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// ผู้ใช้ตาม id แบบไม่มี password_hash — ใช้ใน extractor (ทุก request ที่ login แล้ว)
pub async fn find_profile_by_id(
    pool: &PgPool,
    user_id: UserId,
) -> Result<Option<ProfileRow>, RepositoryError> {
    let row = sqlx::query_as!(
        ProfileRow,
        r#"select id as "id: UserId", uuid as public_id, username, display_name, is_active,
           must_change_password
           from users where id = $1"#,
        user_id.0
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// ผลการจองครั้งตรวจ login หนึ่งครั้ง (`reserve_*`)
#[derive(Debug, PartialEq, Eq)]
pub enum Reservation {
    /// จองได้เป็นครั้งที่ `attempt` ของรอบนี้ (เริ่มที่ 1) — เกิน `max` แปลว่ารอบนี้ใช้ครบแล้ว (ห้าม verify)
    /// `locked_until` = ล็อกที่ตั้งไว้ตอนจอง (ตั้งตอนจองครั้งที่ `max + 1` เป็นต้นไป)
    Granted {
        attempt: i32,
        locked_until: Option<DateTime<Utc>>,
    },
    /// ล็อกอยู่แล้ว — ไม่ได้จอง ไม่ได้เขียนอะไร
    Locked { until: DateTime<Utc> },
}

/// จองครั้งตรวจรหัสของบัญชีนี้ **ก่อน verify** (atomic — คำขอพร้อมกันได้เลขต่างกัน จึงผ่านด่าน verify ได้ไม่เกิน `max` ครั้งต่อรอบ
/// ไม่ว่าจะรอคิว argon2 นานแค่ไหน) ล็อกหมดอายุแล้ว = เริ่มนับใหม่ที่ 1; ระหว่างล็อกไม่เขียนอะไร
///
/// เลขที่จองเกิน `max` ตั้งล็อก (`lock_until`) ให้เองตอนจอง: ครั้งที่ `max` ที่ถูกตัดกลางทาง (timeout ของ request) จะไม่ได้ตั้งล็อก
/// เองตอนล้มเหลว รอบนี้จึงปิดโดยผู้จองครั้งถัดไป ไม่ค้างเป็นรอบที่ไม่มีทางหมดอายุ
pub async fn reserve_user_attempt(
    pool: &PgPool,
    user_id: UserId,
    now: DateTime<Utc>,
    max: i32,
    lock_until: DateTime<Utc>,
) -> Result<Reservation, RepositoryError> {
    let row = sqlx::query!(
        r#"update users
           set failed_attempts = case when locked_until <= $2 then 1
                                      else least(failed_attempts + 1, $3::int4 + 1) end,
               locked_until = case when locked_until <= $2 then null
                                   when failed_attempts + 1 > $3::int4 then $4::timestamptz
                                   else locked_until end,
               updated_at = now()
           where id = $1 and (locked_until is null or locked_until <= $2)
           returning failed_attempts, locked_until"#,
        user_id.0,
        now,
        max,
        lock_until
    )
    .fetch_optional(pool)
    .await?;
    if let Some(row) = row {
        return Ok(Reservation::Granted {
            attempt: row.failed_attempts,
            locked_until: row.locked_until,
        });
    }
    // ไม่ได้จอง = ล็อกอยู่ตอนที่ update ประเมินเงื่อนไข อ่านเวลาปลดล็อกมาตอบ (ล็อกหมดอายุพอดีระหว่างสองคำสั่งนี้ = ถือว่ายังล็อก)
    let until = sqlx::query_scalar!("select locked_until from users where id = $1", user_id.0)
        .fetch_optional(pool)
        .await?
        .flatten()
        .unwrap_or(lock_until);
    Ok(Reservation::Locked { until })
}

pub async fn lock_account(
    pool: &PgPool,
    user_id: UserId,
    until: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "update users set locked_until = $2, updated_at = now() where id = $1",
        user_id.0,
        until
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// ล็อกของชื่อที่ไม่มีในระบบ (`login_throttle` — key = SHA-256 ของชื่อ ไม่เก็บชื่อดิบ) ไม่มีแถว = ไม่เคยผิด
pub async fn find_unknown_lock(
    pool: &PgPool,
    username_hash: &str,
) -> Result<Option<DateTime<Utc>>, RepositoryError> {
    let row = sqlx::query!(
        "select locked_until from login_throttle where username_hash = $1",
        username_hash
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.and_then(|r| r.locked_until))
}

/// จองครั้งตรวจรหัสของชื่อที่ไม่มีในระบบ — กติกาเดียวกับ `reserve_user_attempt` ทุกประการ (ตัวนับอยู่ใน `login_throttle`)
/// เมื่อจองได้ลบแถวที่ไม่ได้แตะเกิน 30 วันด้วย (กันตารางโตจากชื่อสุ่ม — ใช้ index `login_throttle_updated_at_idx`)
pub async fn reserve_unknown_attempt(
    pool: &PgPool,
    username_hash: &str,
    now: DateTime<Utc>,
    max: i32,
    lock_until: DateTime<Utc>,
) -> Result<Reservation, RepositoryError> {
    let row = sqlx::query!(
        r#"insert into login_throttle (username_hash, failed_attempts, updated_at)
           values ($1, 1, $2)
           on conflict (username_hash) do update
           set failed_attempts = case when login_throttle.locked_until <= $2 then 1
                                      else least(login_throttle.failed_attempts + 1, $3::int4 + 1) end,
               locked_until = case when login_throttle.locked_until <= $2 then null
                                   when login_throttle.failed_attempts + 1 > $3::int4 then $4::timestamptz
                                   else login_throttle.locked_until end,
               updated_at = $2
           where login_throttle.locked_until is null or login_throttle.locked_until <= $2
           returning failed_attempts, locked_until"#,
        username_hash,
        now,
        max,
        lock_until
    )
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        let until = find_unknown_lock(pool, username_hash)
            .await?
            .unwrap_or(lock_until);
        return Ok(Reservation::Locked { until });
    };
    sqlx::query!(
        "delete from login_throttle where updated_at < $1::timestamptz - interval '30 days'",
        now
    )
    .execute(pool)
    .await?;
    Ok(Reservation::Granted {
        attempt: row.failed_attempts,
        locked_until: row.locked_until,
    })
}

pub async fn lock_unknown(
    pool: &PgPool,
    username_hash: &str,
    until: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "update login_throttle set locked_until = $2 where username_hash = $1",
        username_hash,
        until
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reset_failed_attempts(pool: &PgPool, user_id: UserId) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"update users set failed_attempts = 0, locked_until = null, updated_at = now()
           where id = $1"#,
        user_id.0
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// สร้าง session เฉพาะเมื่อ credential ที่ login เพิ่งตรวจยังเป็นของจริงอยู่ — `false` = รหัสผ่านถูกเปลี่ยน/รีเซ็ต
/// หรือบัญชีถูกปิดระหว่างที่ login นี้รอคิว argon2 (ไม่สร้างแถว)
///
/// `for share` ล็อกแถวผู้ใช้ให้ชนกับ `update users` ของรีเซ็ต/เปลี่ยนรหัส: ถ้ารีเซ็ตยังไม่ commit คำสั่งนี้รอ แล้วประเมิน
/// `password_hash = $6` ใหม่หลังรีเซ็ต commit (ไม่ตรง → ไม่ insert); ถ้าคำสั่งนี้ได้ก่อน รีเซ็ตที่ตามมาจะลบแถวนี้เอง
/// (`insert ... where exists` เฉยๆ ไม่พอ เพราะ READ COMMITTED มองไม่เห็นรีเซ็ตที่ยังไม่ commit)
pub async fn insert_session_if_credential_unchanged(
    pool: &PgPool,
    token_hash: &str,
    user_id: UserId,
    verified_password_hash: &str,
    idle_expires_at: DateTime<Utc>,
    absolute_expires_at: DateTime<Utc>,
    user_agent: Option<&str>,
) -> Result<bool, RepositoryError> {
    let result = sqlx::query!(
        r#"insert into sessions (token_hash, user_id, idle_expires_at, absolute_expires_at, user_agent)
           select $1::text, u.id, $3::timestamptz, $4::timestamptz, $5::text
           from (select id from users
                 where id = $2 and password_hash = $6 and is_active
                 for share) u"#,
        token_hash,
        user_id.0,
        idle_expires_at,
        absolute_expires_at,
        user_agent,
        verified_password_hash
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// จองครั้งตรวจรหัสผ่านปัจจุบันของ session นี้ก่อน verify (atomic — คำขอพร้อมกันได้เลขต่างกัน จึงเกินเพดานไม่ได้)
/// คืนเลขครั้งที่จอง; None = ไม่มี session นี้แล้ว
pub async fn register_password_check(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<i32>, RepositoryError> {
    let row = sqlx::query!(
        "update sessions set password_check_count = password_check_count + 1
         where token_hash = $1 returning password_check_count",
        token_hash
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.password_check_count))
}

/// รหัสผ่านปัจจุบันถูกต้อง → เริ่มนับใหม่
pub async fn reset_password_checks(pool: &PgPool, token_hash: &str) -> Result<(), RepositoryError> {
    sqlx::query!(
        "update sessions set password_check_count = 0 where token_hash = $1",
        token_hash
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// ลบ session ที่หมดอายุแล้ว (idle หรือ absolute) ของทุกคน — เรียกตอน login สำเร็จ ไม่ต้องมีงานตั้งเวลาแยก
/// (แถวที่หมดอายุใช้ต่อไม่ได้อยู่แล้ว แค่ไม่ให้ค้างในตาราง)
pub async fn delete_expired_sessions(
    pool: &PgPool,
    now: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "delete from sessions where idle_expires_at < $1 or absolute_expires_at < $1",
        now
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_session_by_token_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<SessionRow>, RepositoryError> {
    let row = sqlx::query_as!(
        SessionRow,
        r#"select user_id as "user_id: UserId", last_seen_at, idle_expires_at, absolute_expires_at
           from sessions where token_hash = $1"#,
        token_hash
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn touch_session(
    pool: &PgPool,
    token_hash: &str,
    now: DateTime<Utc>,
    new_idle_expires_at: DateTime<Utc>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "update sessions set last_seen_at = $2, idle_expires_at = $3 where token_hash = $1",
        token_hash,
        now,
        new_idle_expires_at
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// ลบ session แล้วคืนเจ้าของ (คำสั่งเดียว atomic) — None = ไม่มี session นี้ (หมดอายุ/ลบไปแล้ว)
pub async fn delete_session(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<UserId>, RepositoryError> {
    let row = sqlx::query!(
        r#"delete from sessions where token_hash = $1 returning user_id as "user_id: UserId""#,
        token_hash
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.user_id))
}

pub async fn delete_all_sessions_for_user(
    pool: &PgPool,
    user_id: UserId,
) -> Result<u64, RepositoryError> {
    let result = sqlx::query!("delete from sessions where user_id = $1", user_id.0)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn fetch_permissions_for_user(
    pool: &PgPool,
    user_id: UserId,
) -> Result<Vec<Permission>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select distinct rp.permission
           from user_roles ur
           join role_permissions rp on rp.role_id = ur.role_id
           where ur.user_id = $1"#,
        user_id.0
    )
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            Permission::parse(&row.permission)
                .ok_or_else(|| RepositoryError::UnknownPermission(row.permission.clone()))
        })
        .collect()
}

/// เปลี่ยนรหัสผ่านแบบ compare-and-swap ในทรานแซกชันเดียวกับการเพิกถอน session อื่นและบันทึก audit
/// (docs/auth-design.md ข้อ 8) — `false` = hash ในฐานไม่ใช่ตัวที่เพิ่งตรวจแล้ว (ถูกรีเซ็ต/เปลี่ยนไประหว่างรอคิว argon2)
/// หรือบัญชีถูกปิด: ไม่เขียนอะไรเลย ใต้ READ COMMITTED เงื่อนไขถูกประเมินใหม่หลังรอ row lock ของรีเซ็ตที่ commit ก่อน
pub async fn change_password_if_unchanged(
    pool: &PgPool,
    user_id: UserId,
    verified_password_hash: &str,
    new_password_hash: &str,
    keep_token_hash: &str,
) -> Result<bool, RepositoryError> {
    let mut tx = pool.begin().await?;
    let updated = sqlx::query!(
        r#"update users set password_hash = $3, must_change_password = false, updated_at = now()
           where id = $1 and password_hash = $2 and is_active"#,
        user_id.0,
        verified_password_hash,
        new_password_hash
    )
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if updated == 0 {
        return Ok(false);
    }
    // เพิกถอน session อื่นทั้งหมดยกเว้นอันปัจจุบัน
    sqlx::query!(
        "delete from sessions where user_id = $1 and token_hash != $2",
        user_id.0,
        keep_token_hash
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query!(
        r#"insert into audit_log (actor_user_id, action, target_type) values ($1, 'password_changed', 'user')"#,
        user_id.0
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(true)
}

pub async fn count_users(pool: &PgPool) -> Result<i64, RepositoryError> {
    let row = sqlx::query!(r#"select count(*) as "count!" from users"#)
        .fetch_one(pool)
        .await?;
    Ok(row.count)
}

pub async fn find_role_id_by_name(
    pool: &PgPool,
    name: &str,
) -> Result<Option<RoleId>, RepositoryError> {
    let row = sqlx::query!(
        r#"select id as "id: RoleId" from roles where name = $1"#,
        name
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.id))
}

/// สร้างผู้ใช้ + ผูก role ในทรานแซกชันเดียว (docs/rust-guidelines.md ข้อ 11)
pub async fn insert_user_with_role(
    pool: &PgPool,
    username: &str,
    display_name: &str,
    password_hash: &str,
    must_change_password: bool,
    role_id: RoleId,
) -> Result<UserId, RepositoryError> {
    let mut tx = pool.begin().await?;

    let user = sqlx::query!(
        r#"insert into users (username, display_name, password_hash, must_change_password)
           values ($1, $2, $3, $4) returning id as "id: UserId""#,
        username,
        display_name,
        password_hash,
        must_change_password
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        "insert into user_roles (user_id, role_id) values ($1, $2)",
        user.id.0,
        role_id.0
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(user.id)
}

pub struct AuditEntry<'a> {
    pub actor_user_id: Option<UserId>,
    pub action: &'a str,
    pub target_type: Option<&'a str>,
    pub target_id: Option<&'a str>,
    pub metadata: Option<serde_json::Value>,
    /// IP ของผู้ใช้จาก header (ประวัติการเข้าออก) — None = ไม่ทราบ/ไม่เกี่ยว
    pub ip: Option<IpAddr>,
}

pub async fn insert_audit_log(pool: &PgPool, entry: AuditEntry<'_>) -> Result<(), RepositoryError> {
    // ส่ง IP เป็นข้อความแล้ว cast เป็น inet ใน SQL — ไม่ต้องเปิด feature ipnetwork ของ sqlx (dependency เพิ่ม)
    sqlx::query!(
        r#"insert into audit_log (actor_user_id, action, target_type, target_id, metadata, ip)
           values ($1, $2, $3, $4, $5, ($6::text)::inet)"#,
        entry.actor_user_id.map(|id| id.0),
        entry.action,
        entry.target_type,
        entry.target_id,
        entry.metadata,
        entry.ip.map(|ip| ip.to_string())
    )
    .execute(pool)
    .await?;
    Ok(())
}
