//! query SQLx ของการลงทะเบียนผ่านลิงก์ (ห้ามรู้เรื่อง HTTP) — อ่าน/ใช้ลิงก์ใน `registration_links` และตั้งรหัสผ่าน +
//! ข้อมูลตัวใน `users` ด้วย query ของโมดูลนี้เอง (โมดูลฟีเจอร์ไม่เรียกกันเอง — การออกลิงก์อยู่ใน users module)
//! เขียน audit ในทรานแซกชันเดียวกัน ไม่มี metadata

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{PrenameId, UserId};

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

/// ลิงก์ที่ยังใช้ได้ (ยังไม่ใช้ ไม่ถูกแทน ไม่หมดอายุ บัญชียังเปิดใช้งาน) + ข้อมูลบัญชีที่ลิงก์นี้ตั้งรหัสให้
/// ไม่ derive Debug — มีชื่อ-สกุลเจ้าหน้าที่ (ข้อมูลส่วนบุคคล)
pub struct UsableLink {
    pub username: String,
    pub display_name: String,
    /// None = ยังไม่เคยลงทะเบียน (ลิงก์นี้เป็นการลงทะเบียนครั้งแรก) มีค่า = ลิงก์ตั้งรหัสใหม่หลังรีเซ็ต
    pub registered_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub prename_public_id: Option<Uuid>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub position: Option<String>,
}

pub struct PrenameOptionRow {
    pub public_id: Uuid,
    pub name: String,
}

/// ข้อมูลตัวที่ผู้ใช้กรอกตอนลงทะเบียนครั้งแรก (ตรวจแล้วที่ service)
pub struct ProfileWrite<'a> {
    pub prename_id: PrenameId,
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub position: Option<&'a str>,
}

pub async fn find_usable_link(
    pool: &PgPool,
    token_hash: &str,
    now: DateTime<Utc>,
) -> Result<Option<UsableLink>, RepositoryError> {
    let row = sqlx::query_as!(
        UsableLink,
        r#"select u.username, u.display_name, u.registered_at, l.expires_at,
                  p.uuid as "prename_public_id?",
                  u.first_name, u.last_name, u.position
           from registration_links l
           join users u on u.id = l.user_id
           left join prename p on p.id = u.prename_id
           where l.token_hash = $1 and l.used_at is null and l.revoked_at is null
             and l.expires_at > $2 and u.is_active"#,
        token_hash,
        now
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// ตัวเลือกคำนำหน้าของฟอร์มลงทะเบียน (master ที่ยังไม่ถูกลบ) — หน้าลงทะเบียนเรียก /prenames ไม่ได้ (ไม่ได้ login)
pub async fn list_prenames(pool: &PgPool) -> Result<Vec<PrenameOptionRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        PrenameOptionRow,
        r#"select uuid as public_id, name from prename
           where deleted_at is null order by sort_order, name"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_prename_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<PrenameId>, RepositoryError> {
    let row = sqlx::query!(
        r#"select id as "id: PrenameId" from prename where uuid = $1 and deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.id))
}

/// ใช้ลิงก์ (ครั้งเดียว) + ตั้งรหัสผ่าน (ปลดล็อก, เลิกสถานะรหัสชั่วคราว) + ข้อมูลตัว (เฉพาะลงทะเบียนครั้งแรก) + ลบ session
/// ของบัญชีนี้ + audit ในทรานแซกชันเดียว — คืน false ถ้าลิงก์ใช้ไม่ได้แล้ว ณ ตอนเขียน (มีคนใช้ไปพร้อมกัน/หมดอายุ/
/// บัญชีถูกปิดระหว่างนั้น) โดยไม่เขียนอะไรเลย
pub async fn complete_registration(
    pool: &PgPool,
    token_hash: &str,
    now: DateTime<Utc>,
    password_hash: &str,
    profile: Option<&ProfileWrite<'_>>,
) -> Result<bool, RepositoryError> {
    let mut tx = pool.begin().await?;

    let Some(link) = sqlx::query!(
        r#"update registration_links l set used_at = $2
           where l.token_hash = $1 and l.used_at is null and l.revoked_at is null and l.expires_at > $2
             and exists (select 1 from users u where u.id = l.user_id and u.is_active)
           returning l.user_id as "user_id: UserId""#,
        token_hash,
        now
    )
    .fetch_optional(&mut *tx)
    .await?
    else {
        return Ok(false);
    };

    let user = sqlx::query!(
        r#"update users
           set password_hash = $2, must_change_password = false, failed_attempts = 0,
               locked_until = null, registered_at = coalesce(registered_at, $3), updated_at = $3
           where id = $1
           returning uuid as public_id"#,
        link.user_id.0,
        password_hash,
        now
    )
    .fetch_one(&mut *tx)
    .await?;

    if let Some(profile) = profile {
        sqlx::query!(
            r#"update users set prename_id = $2, first_name = $3, last_name = $4, position = $5
               where id = $1"#,
            link.user_id.0,
            profile.prename_id.0,
            profile.first_name,
            profile.last_name,
            profile.position
        )
        .execute(&mut *tx)
        .await?;
    }

    // ตั้งรหัสใหม่แล้ว session เก่า (ถ้ามี) ต้องใช้ไม่ได้ — รีเซ็ตลบไปแล้วรอบหนึ่ง ลบซ้ำกันพลาด
    sqlx::query!("delete from sessions where user_id = $1", link.user_id.0)
        .execute(&mut *tx)
        .await?;

    let action = if profile.is_some() {
        "user_registered"
    } else {
        "user_password_set"
    };
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id)
         values ($1, $2, 'user', $3)",
        link.user_id.0,
        action,
        user.public_id.to_string()
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(true)
}
