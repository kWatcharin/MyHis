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

/// นับ login ผิดของบัญชีนี้เพิ่มหนึ่ง — ถ้าล็อกครั้งก่อนหมดอายุแล้ว เริ่มนับใหม่ที่ 1 (ได้ครบ 5 ครั้งอีกรอบ)
pub async fn register_user_failure(
    pool: &PgPool,
    user_id: UserId,
    now: DateTime<Utc>,
) -> Result<i32, RepositoryError> {
    let row = sqlx::query!(
        r#"update users
           set failed_attempts = case when locked_until <= $2 then 1 else failed_attempts + 1 end,
               locked_until = case when locked_until <= $2 then null else locked_until end,
               updated_at = now()
           where id = $1 returning failed_attempts"#,
        user_id.0,
        now
    )
    .fetch_one(pool)
    .await?;
    Ok(row.failed_attempts)
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

/// นับ login ผิดของชื่อที่ไม่มีในระบบ — กติกาเดียวกับ `register_user_failure` (ล็อกหมดอายุ = เริ่มนับใหม่)
/// แล้วลบแถวที่ไม่ได้แตะเกิน 30 วัน กันตารางโตจากชื่อสุ่ม
pub async fn register_unknown_failure(
    pool: &PgPool,
    username_hash: &str,
    now: DateTime<Utc>,
) -> Result<i32, RepositoryError> {
    let row = sqlx::query!(
        r#"insert into login_throttle (username_hash, failed_attempts, updated_at)
           values ($1, 1, $2)
           on conflict (username_hash) do update
           set failed_attempts = case when login_throttle.locked_until <= $2 then 1
                                      else login_throttle.failed_attempts + 1 end,
               locked_until = case when login_throttle.locked_until <= $2 then null
                                   else login_throttle.locked_until end,
               updated_at = $2
           returning failed_attempts"#,
        username_hash,
        now
    )
    .fetch_one(pool)
    .await?;
    sqlx::query!(
        "delete from login_throttle where updated_at < $1::timestamptz - interval '30 days'",
        now
    )
    .execute(pool)
    .await?;
    Ok(row.failed_attempts)
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

pub async fn insert_session(
    pool: &PgPool,
    token_hash: &str,
    user_id: UserId,
    idle_expires_at: DateTime<Utc>,
    absolute_expires_at: DateTime<Utc>,
    user_agent: Option<&str>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"insert into sessions (token_hash, user_id, idle_expires_at, absolute_expires_at, user_agent)
           values ($1, $2, $3, $4, $5)"#,
        token_hash,
        user_id.0,
        idle_expires_at,
        absolute_expires_at,
        user_agent
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

/// ลบ session อื่นทั้งหมดของผู้ใช้ ยกเว้นอันที่กำลังใช้อยู่ตอนนี้ (เปลี่ยนรหัสผ่านสำเร็จ —
/// docs/auth-design.md ข้อ 8)
pub async fn delete_all_sessions_for_user_except(
    pool: &PgPool,
    user_id: UserId,
    keep_token_hash: &str,
) -> Result<u64, RepositoryError> {
    let result = sqlx::query!(
        "delete from sessions where user_id = $1 and token_hash != $2",
        user_id.0,
        keep_token_hash
    )
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

pub async fn update_password_hash(
    pool: &PgPool,
    user_id: UserId,
    new_password_hash: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"update users set password_hash = $2, must_change_password = false, updated_at = now()
           where id = $1"#,
        user_id.0,
        new_password_hash
    )
    .execute(pool)
    .await?;
    Ok(())
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
