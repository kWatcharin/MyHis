use sqlx::PgPool;

use super::RepositoryError;
use crate::domain::{Permission, RoleId};

#[derive(Debug)]
pub struct RoleWithPermissions {
    pub id: RoleId,
    pub name: String,
    pub description: Option<String>,
    pub is_system: bool,
    pub permissions: Vec<Permission>,
}

pub async fn list_roles(pool: &PgPool) -> Result<Vec<RoleWithPermissions>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select r.id as "id: RoleId", r.name, r.description, r.is_system,
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
                name: row.name,
                description: row.description,
                is_system: row.is_system,
                permissions: permission.into_iter().collect(),
            }),
        }
    }
    Ok(roles)
}

pub async fn find_role_by_id(
    pool: &PgPool,
    id: RoleId,
) -> Result<Option<RoleWithPermissions>, RepositoryError> {
    Ok(list_roles(pool).await?.into_iter().find(|r| r.id == id))
}

/// สร้าง role + permission ในทรานแซกชันเดียว
pub async fn insert_role(
    pool: &PgPool,
    name: &str,
    description: Option<&str>,
    permissions: &[Permission],
) -> Result<RoleId, RepositoryError> {
    let mut tx = pool.begin().await?;

    let role = sqlx::query!(
        r#"insert into roles (name, description) values ($1, $2) returning id as "id: RoleId""#,
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

    tx.commit().await?;
    Ok(role.id)
}

/// partial update — name/description ผ่าน COALESCE, permissions แทนที่ทั้งชุดถ้าส่งมา (Some)
pub async fn update_role(
    pool: &PgPool,
    id: RoleId,
    name: Option<&str>,
    description: Option<&str>,
    permissions: Option<&[Permission]>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        r#"update roles
           set name = coalesce($2, name),
               description = coalesce($3, description),
               updated_at = now()
           where id = $1"#,
        id.0,
        name,
        description
    )
    .execute(&mut *tx)
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

pub async fn delete_role(pool: &PgPool, id: RoleId) -> Result<(), RepositoryError> {
    sqlx::query!("delete from roles where id = $1", id.0)
        .execute(pool)
        .await?;
    Ok(())
}
