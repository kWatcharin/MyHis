use sqlx::PgPool;
use uuid::Uuid;

use super::{insert_labeled_audit, RepositoryError};
use crate::domain::{Permission, RoleId, UserId};

#[derive(Debug)]
pub struct RoleWithPermissions {
    pub id: RoleId,
    pub public_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub permissions: Vec<Permission>,
}

pub async fn list_roles(pool: &PgPool) -> Result<Vec<RoleWithPermissions>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select r.id as "id: RoleId", r.uuid as public_id, r.name, r.description, r.is_system,
                  rp.permission as "permission?"
           from roles r
           left join role_permissions rp on rp.role_id = r.id
           order by r.name, rp.permission"#
    )
    .fetch_all(pool)
    .await?;

    let mut roles: Vec<RoleWithPermissions> = Vec::new();
    for row in rows {
        let permission = row
            .permission
            .map(|p| Permission::parse(&p).ok_or(p))
            .transpose()
            .map_err(RepositoryError::UnknownPermission)?;

        match roles.last_mut() {
            Some(last) if last.id == row.id => {
                if let Some(p) = permission {
                    last.permissions.push(p);
                }
            }
            _ => roles.push(RoleWithPermissions {
                id: row.id,
                public_id: row.public_id,
                name: row.name,
                description: row.description,
                is_system: row.is_system,
                permissions: permission.into_iter().collect(),
            }),
        }
    }
    Ok(roles)
}

pub async fn find_role_by_public_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<RoleWithPermissions>, RepositoryError> {
    Ok(list_roles(pool)
        .await?
        .into_iter()
        .find(|r| r.public_id == public_id))
}

/// role ที่ผู้ใช้คนหนึ่งถืออยู่ (id ภายใน) — ใช้กับกฎกันแอดมินตัดสิทธิ์จัดการ role ของตัวเองตอนแก้สิทธิ์ของ role
pub async fn role_ids_of_user(pool: &PgPool, user: UserId) -> Result<Vec<RoleId>, RepositoryError> {
    Ok(sqlx::query_scalar!(
        r#"select role_id as "role_id: RoleId" from user_roles where user_id = $1"#,
        user.0
    )
    .fetch_all(pool)
    .await?)
}

/// สร้าง role + permission + audit ในทรานแซกชันเดียว
pub async fn insert_role(
    pool: &PgPool,
    actor: UserId,
    name: &str,
    description: Option<&str>,
    permissions: &[Permission],
) -> Result<RoleId, RepositoryError> {
    let mut tx = pool.begin().await?;

    let role = sqlx::query!(
        r#"insert into roles (name, description) values ($1, $2)
           returning id as "id: RoleId", uuid"#,
        name,
        description
    )
    .fetch_one(&mut *tx)
    .await?;
    insert_labeled_audit(&mut tx, actor, "role_created", "role", role.uuid, name).await?;

    for permission in permissions {
        sqlx::query!(
            "insert into role_permissions (role_id, permission) values ($1, $2)",
            role.id.0,
            permission.as_str()
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(role.id)
}

/// partial update — name/description ผ่าน COALESCE, permissions แทนที่ทั้งชุดถ้าส่งมา (Some) + audit
/// (label = ชื่อหลังแก้) ในทรานแซกชันเดียว
pub async fn update_role(
    pool: &PgPool,
    actor: UserId,
    id: RoleId,
    name: Option<&str>,
    description: Option<&str>,
    permissions: Option<&[Permission]>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;

    let updated = sqlx::query!(
        r#"update roles
           set name = coalesce($2, name),
               description = coalesce($3, description),
               updated_at = now()
           where id = $1
           returning uuid, name"#,
        id.0,
        name,
        description
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(role) = &updated {
        insert_labeled_audit(
            &mut tx,
            actor,
            "role_updated",
            "role",
            role.uuid,
            &role.name,
        )
        .await?;
    }

    if let Some(permissions) = permissions {
        sqlx::query!("delete from role_permissions where role_id = $1", id.0)
            .execute(&mut *tx)
            .await?;
        for permission in permissions {
            sqlx::query!(
                "insert into role_permissions (role_id, permission) values ($1, $2)",
                id.0,
                permission.as_str()
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

pub async fn role_in_use(pool: &PgPool, id: RoleId) -> Result<bool, RepositoryError> {
    let row = sqlx::query!(
        r#"select exists(select 1 from user_roles where role_id = $1) as "exists!""#,
        id.0
    )
    .fetch_one(pool)
    .await?;
    Ok(row.exists)
}

/// ลบจริง (role ไม่มี soft delete) — audit เก็บชื่อไว้ใน label ให้ประวัติยังแสดงชื่อได้หลังลบ
pub async fn delete_role(pool: &PgPool, actor: UserId, id: RoleId) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query!("delete from roles where id = $1 returning uuid, name", id.0)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(role) = &deleted {
        insert_labeled_audit(
            &mut tx,
            actor,
            "role_deleted",
            "role",
            role.uuid,
            &role.name,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
