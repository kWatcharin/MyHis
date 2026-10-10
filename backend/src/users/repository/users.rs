use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::registration_links::{insert_link, revoke_pending_links, NewRegistrationLink};
use super::RepositoryError;
use crate::domain::{Permission, PrenameId, RoleId, UserId};

#[derive(Debug, Clone)]
pub struct RoleRef {
    /// uuid ของ role (id ที่ API เห็น) — ไม่ใช่ pk int ภายใน
    pub public_id: Uuid,
    pub name: String,
}

#[derive(Debug)]
pub struct UserWithRoles {
    pub id: UserId,
    pub public_id: Uuid,
    pub username: String,
    pub display_name: String,
    pub is_active: bool,
    pub must_change_password: bool,
    /// ถูกล็อกชั่วคราวจากการใส่รหัสผิดหลายครั้งถึงเวลานี้ (docs/auth-design.md ข้อ 9) — ค่าในอดีต = ไม่ล็อกแล้ว
    pub locked_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub prename_public_id: Option<Uuid>,
    pub prename_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub position: Option<String>,
    /// ตั้งรหัสผ่านเองผ่านลิงก์ลงทะเบียนสำเร็จเมื่อ — None = ยังไม่ลงทะเบียน (login ไม่ได้)
    pub registered_at: Option<DateTime<Utc>>,
    /// หมดอายุของลิงก์ล่าสุดที่ยังไม่ใช้และไม่ถูกแทน (อาจหมดอายุไปแล้ว) — None = ไม่มีลิงก์ค้าง
    pub registration_expires_at: Option<DateTime<Utc>>,
    pub roles: Vec<RoleRef>,
}

/// role ที่จะผูกให้ผู้ใช้ + ธงว่า role นี้ให้สิทธิ์ `user.manage` ไหม (service ใช้กันแอดมินถอดสิทธิ์จัดการ
/// ผู้ใช้ออกจากตัวเอง)
#[derive(Debug)]
pub struct AssignableRole {
    pub id: RoleId,
    pub grants_user_manage: bool,
}

/// ชื่อที่แสดง + ข้อมูลตัวผู้ใช้ (ผ่านการ normalize/ตรวจแล้วที่ service) สำหรับแทนที่ทั้งชุด — None = ล้างค่า
pub struct UserDetailsWrite<'a> {
    pub display_name: &'a str,
    pub prename_id: Option<PrenameId>,
    pub first_name: Option<&'a str>,
    pub last_name: Option<&'a str>,
    pub position: Option<&'a str>,
}

/// รายชื่อผู้ใช้ทั้งหมดพร้อม role — join เดียว group ใน Rust แทนการ query ในลูป
/// (docs/rust-guidelines.md ข้อ 11 — ระวัง N+1) ทุก user มี role อย่างน้อย 1 อยู่แล้ว (invariant)
/// เลยใช้ inner join ธรรมดาได้ — คำนำหน้า/ลิงก์ที่ค้างเป็น left join (อาจไม่มี)
pub async fn list_users(pool: &PgPool) -> Result<Vec<UserWithRoles>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select
             u.id as "user_id: UserId", u.uuid as user_public_id, u.username, u.display_name,
             u.is_active, u.must_change_password, u.locked_until, u.created_at,
             p.uuid as "prename_public_id?", p.name as "prename_name?",
             u.first_name, u.last_name, u.position, u.registered_at,
             link.expires_at as "registration_expires_at?",
             r.uuid as role_public_id, r.name as role_name
           from users u
           join user_roles ur on ur.user_id = u.id
           join roles r on r.id = ur.role_id
           left join prename p on p.id = u.prename_id
           left join lateral (
             select l.expires_at from registration_links l
             where l.user_id = u.id and l.used_at is null and l.revoked_at is null
             order by l.created_at desc
             limit 1
           ) link on true
           order by u.created_at, u.id, r.name"#
    )
    .fetch_all(pool)
    .await?;

    let mut users: Vec<UserWithRoles> = Vec::new();
    for row in rows {
        let role = RoleRef {
            public_id: row.role_public_id,
            name: row.role_name,
        };
        match users.last_mut() {
            Some(last) if last.id == row.user_id => last.roles.push(role),
            _ => users.push(UserWithRoles {
                id: row.user_id,
                public_id: row.user_public_id,
                username: row.username,
                display_name: row.display_name,
                is_active: row.is_active,
                must_change_password: row.must_change_password,
                locked_until: row.locked_until,
                created_at: row.created_at,
                prename_public_id: row.prename_public_id,
                prename_name: row.prename_name,
                first_name: row.first_name,
                last_name: row.last_name,
                position: row.position,
                registered_at: row.registered_at,
                registration_expires_at: row.registration_expires_at,
                roles: vec![role],
            }),
        }
    }
    Ok(users)
}

/// แปลง uuid ที่มาจาก API เป็น pk ภายใน — None = ไม่มีผู้ใช้คนนี้ ยังไม่มี endpoint ไหนต้องอ่าน
/// รายละเอียดผู้ใช้คนเดียว (GET /users คืนทุกคนพร้อม role อยู่แล้ว) เลยแค่หา id พอ
pub async fn find_user_id_by_public_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<UserId>, RepositoryError> {
    let row = sqlx::query!(
        r#"select id as "id: UserId" from users where uuid = $1"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.id))
}

/// คำนำหน้าที่ยังไม่ถูกลบ (master prename — อ่านด้วย query ของโมดูลนี้เอง โมดูลฟีเจอร์ไม่เรียกกันเอง)
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

/// หา role หลายตัวจาก uuid ใน query เดียว (แทนการหาทีละตัวในลูป) — คืนเฉพาะ role ที่พบ caller เทียบ
/// จำนวนเองว่าครบไหม
pub async fn find_roles_for_assignment(
    pool: &PgPool,
    public_ids: &[Uuid],
) -> Result<Vec<AssignableRole>, RepositoryError> {
    let rows = sqlx::query_as!(
        AssignableRole,
        r#"select r.id as "id: RoleId",
                  exists(select 1 from role_permissions rp
                         where rp.role_id = r.id and rp.permission = $2) as "grants_user_manage!"
           from roles r
           where r.uuid = any($1)"#,
        public_ids,
        Permission::UserManage.as_str()
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// สร้างผู้ใช้ที่ยังไม่ลงทะเบียน (`registered_at` null, รหัสผ่านใช้ไม่ได้) + ผูก role + ออกลิงก์ลงทะเบียน +
/// audit ในทรานแซกชันเดียว (docs/rust-guidelines.md ข้อ 11)
pub async fn insert_user_with_role(
    pool: &PgPool,
    actor: UserId,
    username: &str,
    display_name: &str,
    unusable_password_hash: &str,
    role_id: RoleId,
    link: &NewRegistrationLink<'_>,
) -> Result<(UserId, Uuid), RepositoryError> {
    let mut tx = pool.begin().await?;

    let user = sqlx::query!(
        r#"insert into users (username, display_name, password_hash, must_change_password, registered_at)
           values ($1, $2, $3, false, null) returning id as "id: UserId", uuid as public_id"#,
        username,
        display_name,
        unusable_password_hash
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

    insert_link(&mut tx, user.id, actor, link).await?;
    insert_audit(&mut tx, actor, "user_created", user.public_id).await?;

    tx.commit().await?;
    Ok((user.id, user.public_id))
}

/// partial update ผ่าน COALESCE — ค่า None ไม่แตะคอลัมน์เดิม ปิดบัญชี (`is_active = false`) ลบ session
/// ทั้งหมดของคนนั้นในทรานแซกชันเดียวกัน ให้หลุดจากระบบทันที (docs/auth-design.md ข้อ 2) — ไม่แตะลิงก์ที่ค้าง
/// (ลิงก์ของบัญชีที่ถูกปิดใช้ไม่ได้อยู่แล้ว เปิดกลับแล้วลิงก์ที่ยังไม่หมดอายุใช้ต่อได้)
pub async fn update_user(
    pool: &PgPool,
    actor: UserId,
    id: UserId,
    public_id: Uuid,
    display_name: Option<&str>,
    is_active: Option<bool>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;

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
    .execute(&mut *tx)
    .await?;

    if display_name.is_some() {
        insert_audit(&mut tx, actor, "user_updated", public_id).await?;
    }
    match is_active {
        Some(true) => insert_audit(&mut tx, actor, "user_activated", public_id).await?,
        Some(false) => {
            delete_sessions(&mut tx, id).await?;
            insert_audit(&mut tx, actor, "user_deactivated", public_id).await?;
        }
        None => {}
    }

    tx.commit().await?;
    Ok(())
}

/// แทนที่ชื่อที่แสดง + ข้อมูลตัว + ชุด role ในทรานแซกชันเดียว (ฟอร์มแก้ไขผู้ใช้ของแอดมิน) — caller ตรวจ role_ids
/// มาแล้ว (ไม่ว่าง ไม่ซ้ำ มีอยู่จริง กฎกันตัวเอง) audit `user_roles_replaced` เพิ่มเฉพาะเมื่อชุด role เปลี่ยนจริง
pub async fn replace_user(
    pool: &PgPool,
    actor: UserId,
    id: UserId,
    public_id: Uuid,
    details: &UserDetailsWrite<'_>,
    role_ids: &[RoleId],
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        r#"update users
           set display_name = $2, prename_id = $3, first_name = $4, last_name = $5, position = $6,
               updated_at = now()
           where id = $1"#,
        id.0,
        details.display_name,
        details.prename_id.map(|p| p.0),
        details.first_name,
        details.last_name,
        details.position
    )
    .execute(&mut *tx)
    .await?;

    let current: Vec<RoleId> = sqlx::query_scalar!(
        r#"select role_id as "role_id: RoleId" from user_roles where user_id = $1"#,
        id.0
    )
    .fetch_all(&mut *tx)
    .await?;
    let roles_changed =
        current.len() != role_ids.len() || !role_ids.iter().all(|role| current.contains(role));
    if roles_changed {
        write_roles(&mut tx, id, role_ids).await?;
        insert_audit(&mut tx, actor, "user_roles_replaced", public_id).await?;
    }
    insert_audit(&mut tx, actor, "user_updated", public_id).await?;

    tx.commit().await?;
    Ok(())
}

/// รีเซ็ตรหัสผ่าน: ตั้งรหัสที่ใช้ไม่ได้ (รหัสเดิมใช้ไม่ได้ทันที) + ปลดล็อก + เพิกถอน session ทั้งหมด + แทนลิงก์เดิม
/// ด้วยลิงก์ใหม่ + audit ในทรานแซกชันเดียว (docs/rust-guidelines.md ข้อ 11) — ผู้ใช้ตั้งรหัสใหม่เองผ่านลิงก์
pub async fn reset_password(
    pool: &PgPool,
    actor: UserId,
    id: UserId,
    public_id: Uuid,
    unusable_password_hash: &str,
    link: &NewRegistrationLink<'_>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        r#"update users
           set password_hash = $2, must_change_password = false,
               failed_attempts = 0, locked_until = null, updated_at = now()
           where id = $1"#,
        id.0,
        unusable_password_hash
    )
    .execute(&mut *tx)
    .await?;

    delete_sessions(&mut tx, id).await?;
    revoke_pending_links(&mut tx, id).await?;
    insert_link(&mut tx, id, actor, link).await?;
    insert_audit(&mut tx, actor, "user_password_reset", public_id).await?;

    tx.commit().await?;
    Ok(())
}

/// แทนที่ชุด role ทั้งหมดของผู้ใช้ + audit ในทรานแซกชันเดียว — caller ต้องเช็คว่า role_ids ไม่ว่าง
/// ไม่ซ้ำ และมีอยู่จริงก่อนเรียก (invariant ผู้ใช้ต้องมี role อย่างน้อย 1 อยู่ที่ service layer)
pub async fn replace_user_roles(
    pool: &PgPool,
    actor: UserId,
    id: UserId,
    public_id: Uuid,
    role_ids: &[RoleId],
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    write_roles(&mut tx, id, role_ids).await?;
    insert_audit(&mut tx, actor, "user_roles_replaced", public_id).await?;
    tx.commit().await?;
    Ok(())
}

async fn write_roles(
    tx: &mut Transaction<'_, Postgres>,
    id: UserId,
    role_ids: &[RoleId],
) -> Result<(), RepositoryError> {
    sqlx::query!("delete from user_roles where user_id = $1", id.0)
        .execute(&mut **tx)
        .await?;
    for role_id in role_ids {
        sqlx::query!(
            "insert into user_roles (user_id, role_id) values ($1, $2)",
            id.0,
            role_id.0
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn delete_sessions(
    tx: &mut Transaction<'_, Postgres>,
    id: UserId,
) -> Result<(), RepositoryError> {
    sqlx::query!("delete from sessions where user_id = $1", id.0)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// audit ของงานจัดการผู้ใช้ — target_id เป็น uuid ของผู้ใช้ที่ถูกกระทำ ไม่มี metadata (ห้ามมีรหัสผ่าน/ความลับ)
async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    action: &str,
    target_public_id: Uuid,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id)
         values ($1, $2, 'user', $3)",
        actor.0,
        action,
        target_public_id.to_string()
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
