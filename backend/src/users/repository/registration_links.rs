//! ลิงก์ลงทะเบียน/ตั้งรหัสผ่านใหม่ฝั่งแอดมิน — ออกลิงก์ใหม่และ revoke ลิงก์เดิมที่ยังไม่ใช้ ภายในทรานแซกชันของ
//! การสร้างผู้ใช้/รีเซ็ตรหัส (users.rs) เท่านั้น การตรวจ/ใช้ลิงก์ของผู้ใช้อยู่ในโมดูล registration (query ของตัวเอง)

use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};

use super::RepositoryError;
use crate::domain::UserId;

/// ลิงก์ที่จะออก — DB เก็บแค่ hash ของ token (token ดิบอยู่ใน response ครั้งเดียว)
pub struct NewRegistrationLink<'a> {
    pub token_hash: &'a str,
    pub expires_at: DateTime<Utc>,
}

pub(super) async fn insert_link(
    tx: &mut Transaction<'_, Postgres>,
    user_id: UserId,
    created_by: UserId,
    link: &NewRegistrationLink<'_>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into registration_links (token_hash, user_id, created_by, expires_at)
         values ($1, $2, $3, $4)",
        link.token_hash,
        user_id.0,
        created_by.0,
        link.expires_at
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// ให้ลิงก์เดิมที่ยังไม่ถูกใช้ใช้ไม่ได้อีก — ผู้ใช้หนึ่งคนมีลิงก์ที่ใช้ได้อย่างมากหนึ่งอันเสมอ
pub(super) async fn revoke_pending_links(
    tx: &mut Transaction<'_, Postgres>,
    user_id: UserId,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "update registration_links set revoked_at = now()
         where user_id = $1 and used_at is null and revoked_at is null",
        user_id.0
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
