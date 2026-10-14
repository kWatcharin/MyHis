use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{insert_labeled_audit, LabeledAudit, RepositoryError};
use crate::domain::{changes, Permission, RoleId, UserId};

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

    for permission in permissions {
        sqlx::query!(
            "insert into role_permissions (role_id, permission) values ($1, $2)",
            role.id.0,
            permission.as_str()
        )
        .execute(&mut *tx)
        .await?;
    }

    let after = snapshot(&mut tx, role.id).await?;
    insert_labeled_audit(
        &mut tx,
        actor,
        LabeledAudit {
            action: "role_created",
            target_type: "role",
            target_public_id: role.uuid,
            label: name,
            details: Some(changes::created(after)),
        },
    )
    .await?;

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
    let before = snapshot(&mut tx, id).await?;

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

    if let Some(role) = &updated {
        let after = snapshot(&mut tx, id).await?;
        insert_labeled_audit(
            &mut tx,
            actor,
            LabeledAudit {
                action: "role_updated",
                target_type: "role",
                target_public_id: role.uuid,
                label: &role.name,
                details: Some(changes::updated(&before, &after)),
            },
        )
        .await?;
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
            LabeledAudit {
                action: "role_deleted",
                target_type: "role",
                target_public_id: role.uuid,
                label: &role.name,
                details: None,
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// ค่าของ role สำหรับรายละเอียดในประวัติการใช้งาน (ชื่อ, คำอธิบาย, สิทธิ์เรียงแล้ว) — ล็อกแถวไว้จนจบทรานแซกชัน
async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    id: RoleId,
) -> Result<Value, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"select jsonb_build_object(
                  'name', r.name,
                  'description', r.description,
                  'permissions', coalesce((select jsonb_agg(rp.permission order by rp.permission)
                                           from role_permissions rp where rp.role_id = r.id), '[]'::jsonb)
              ) as "snapshot!"
           from roles r where r.id = $1 for update"#,
        id.0
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.unwrap_or(Value::Null))
}
