//! role: รายชื่อ, สร้าง, แก้ (role ของระบบห้ามเปลี่ยนชื่อ), ลบ (ห้ามลบ role ของระบบ/role ที่ยังมีคนถือ)

use sqlx::PgPool;
use uuid::Uuid;

use super::{map_unique_violation, parse_permissions, require_user_manage, UsersError};
use crate::auth::CurrentUser;
use crate::domain::RoleId;
use crate::users::repository;

pub async fn list_roles(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<repository::RoleWithPermissions>, UsersError> {
    require_user_manage(actor)?;
    Ok(repository::list_roles(pool).await?)
}

pub async fn create_role(
    pool: &PgPool,
    actor: &CurrentUser,
    name: &str,
    description: Option<&str>,
    permissions: &[String],
) -> Result<RoleId, UsersError> {
    require_user_manage(actor)?;
    let permissions = parse_permissions(permissions)?;

    repository::insert_role(pool, name, description, &permissions)
        .await
        .map_err(|e| map_unique_violation(e, "role name already taken"))
}

pub async fn update_role(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    name: Option<&str>,
    description: Option<&str>,
    permissions: Option<&[String]>,
) -> Result<(), UsersError> {
    require_user_manage(actor)?;

    let role = repository::find_role_by_public_id(pool, public_id)
        .await?
        .ok_or(UsersError::RoleNotFound)?;
    if role.is_system && name.is_some() {
        return Err(UsersError::Validation(
            "cannot rename a system role".to_string(),
        ));
    }

    let permissions = permissions.map(parse_permissions).transpose()?;

    repository::update_role(pool, role.id, name, description, permissions.as_deref())
        .await
        .map_err(|e| map_unique_violation(e, "role name already taken"))?;
    Ok(())
}

pub async fn delete_role(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
) -> Result<(), UsersError> {
    require_user_manage(actor)?;

    let role = repository::find_role_by_public_id(pool, public_id)
        .await?
        .ok_or(UsersError::RoleNotFound)?;
    if role.is_system {
        return Err(UsersError::Conflict(
            "cannot delete a system role".to_string(),
        ));
    }
    if repository::role_in_use(pool, role.id).await? {
        return Err(UsersError::Conflict(
            "role is still assigned to at least one user".to_string(),
        ));
    }

    repository::delete_role(pool, role.id).await?;
    Ok(())
}
