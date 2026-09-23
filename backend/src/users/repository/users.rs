use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::RepositoryError;
use crate::domain::{RoleId, UserId};

#[derive(Debug, Clone)]
pub struct RoleRef {
    pub id: RoleId,
    pub name: String,
}

#[derive(Debug)]
pub struct UserWithRoles {
    pub id: UserId,
    pub username: String,
    pub display_name: String,
    pub is_active: bool,
    pub must_change_password: bool,
    pub created_at: DateTime<Utc>,
    pub roles: Vec<RoleRef>,
}

/// รายชื่อผู้ใช้ทั้งหมดพร้อม role — join เดียว group ใน Rust แทนการ query ในลูป
/// (docs/rust-guidelines.md ข้อ 11 — ระวัง N+1) ทุก user มี role อย่างน้อย 1 อยู่แล้ว (invariant)
/// เลยใช้ inner join ธรรมดาได้ ไม่ต้อง left join
pub async fn list_users(pool: &PgPool) -> Result<Vec<UserWithRoles>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select
             u.id as "user_id: UserId", u.username, u.display_name, u.is_active,
             u.must_change_password, u.created_at,
             r.id as "role_id: RoleId", r.name as role_name
           from users u
           join user_roles ur on ur.user_id = u.id
           join roles r on r.id = ur.role_id
           order by u.created_at, r.name"#
    )
    .fetch_all(pool)
    .await?;

    let mut users: Vec<UserWithRoles> = Vec::new();
    for row in rows {
        let role = RoleRef {
            id: row.role_id,
            name: row.role_name,
        };
        match users.last_mut() {
            Some(last) if last.id == row.user_id => last.roles.push(role),
            _ => users.push(UserWithRoles {
                id: row.user_id,
                username: row.username,
                display_name: row.display_name,
                is_active: row.is_active,
                must_change_password: row.must_change_password,
                created_at: row.created_at,
                roles: vec![role],
            }),
        }
    }
    Ok(users)
}

/// เช็คว่าผู้ใช้คนนี้มีอยู่จริงไหม — ยังไม่มี endpoint ไหนต้องอ่านรายละเอียดผู้ใช้คนเดียว
/// (GET /users คืนทุกคนพร้อม role อยู่แล้ว) เลยยังไม่มี row type เต็ม แค่ exists check พอ
pub async fn user_exists(pool: &PgPool, id: UserId) -> Result<bool, RepositoryError> {
    let row = sqlx::query!(
        r#"select exists(select 1 from users where id = $1) as "exists!""#,
        id.0
    )
    .fetch_one(pool)
    .await?;
    Ok(row.exists)
}

/// สร้างผู้ใช้ + ผูก role ในทรานแซกชันเดียว (docs/rust-guidelines.md ข้อ 11)
pub async fn insert_user_with_role(
    pool: &PgPool,
    username: &str,
    display_name: &str,
    password_hash: &str,
    role_id: RoleId,
) -> Result<UserId, RepositoryError> {
    let mut tx = pool.begin().await?;

    let user = sqlx::query!(
        r#"insert into users (username, display_name, password_hash, must_change_password)
           values ($1, $2, $3, true) returning id as "id: UserId""#,
        username,
        display_name,
        password_hash
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

/// partial update ผ่าน COALESCE — ค่า None ไม่แตะคอลัมน์เดิม
pub async fn update_user(
    pool: &PgPool,
    id: UserId,
    display_name: Option<&str>,
    is_active: Option<bool>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"update users
           set display_name = coalesce($2, display_name),
               is_active = coalesce($3, is_active),
               updated_at = now()
           where id = $1"#,
        id.0,
        display_name,
        is_active
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// ตั้งรหัสชั่วคราว + เคลียร์สถานะล็อกเดิม (admin reset ให้) — ต่างจาก self-change-password ที่
/// เคลียร์ must_change_password เป็น false ตัวนี้บังคับ true เพื่อให้ผู้ใช้ต้องเปลี่ยนเอง
pub async fn reset_password(
    pool: &PgPool,
    id: UserId,
    new_password_hash: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"update users
           set password_hash = $2, must_change_password = true,
               failed_attempts = 0, locked_until = null, updated_at = now()
           where id = $1"#,
        id.0,
        new_password_hash
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_all_sessions_for_user(
    pool: &PgPool,
    id: UserId,
) -> Result<u64, RepositoryError> {
    let result = sqlx::query!("delete from sessions where user_id = $1", id.0)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// แทนที่ชุด role ทั้งหมดของผู้ใช้ในทรานแซกชันเดียว — caller ต้องเช็ค role_ids ไม่ว่างและมีอยู่จริง
/// ก่อนเรียก (invariant ผู้ใช้ต้องมี role อย่างน้อย 1 อยู่ที่ service layer)
pub async fn replace_user_roles(
    pool: &PgPool,
    id: UserId,
    role_ids: &[RoleId],
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;

    sqlx::query!("delete from user_roles where user_id = $1", id.0)
        .execute(&mut *tx)
        .await?;

    for role_id in role_ids {
        sqlx::query!(
            "insert into user_roles (user_id, role_id) values ($1, $2)",
            id.0,
            role_id.0
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}
