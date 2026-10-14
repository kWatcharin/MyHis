//! role: รายชื่อ, สร้าง, แก้ (role ของระบบห้ามเปลี่ยนชื่อ แต่แก้คำอธิบาย/สิทธิ์ได้, ห้ามตัดสิทธิ์จัดการ role
//! สุดท้ายของตัวเอง), ลบ (ห้ามลบ role ของระบบ/role ที่ยังมีคนถือ) — สิทธิ์ `role.view/create/update/delete`
//! ห้ามให้สิทธิ์เกินตัวเอง: สร้าง/แก้ role ให้มีสิทธิ์ที่ตัวเองไม่มีไม่ได้ และแก้/ลบ role ที่สิทธิ์มากกว่าตัวเองไม่ได้

use sqlx::PgPool;
use uuid::Uuid;

use super::{
    map_unique_violation, parse_permissions, require, require_role_management_kept,
    require_within_own, UsersError, GRANT_BEYOND_OWN, STRONGER_ROLE,
};
use crate::auth::CurrentUser;
use crate::domain::{Action, Permission, Resource, RoleId};
use crate::users::repository;

/// อ่านรายการได้ทั้งคนที่ดูหน้าการตั้งค่าสิทธิ์ได้ และคนที่ดูหน้าผู้ใช้งานได้ (ฟอร์มผู้ใช้เลือก role จากรายการนี้)
pub async fn list_roles(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<repository::RoleWithPermissions>, UsersError> {
    if !actor.can(Resource::User, Action::View) {
        require(actor, Resource::Role, Action::View)?;
    }
    Ok(repository::list_roles(pool).await?)
}

pub async fn create_role(
    pool: &PgPool,
    actor: &CurrentUser,
    name: &str,
    description: Option<&str>,
    permissions: &[String],
) -> Result<RoleId, UsersError> {
    require(actor, Resource::Role, Action::Create)?;
    let permissions = parse_permissions(permissions)?;
    require_within_own(actor, &permissions, GRANT_BEYOND_OWN)?;

    repository::insert_role(pool, actor.user.id, name, description, &permissions)
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
    require(actor, Resource::Role, Action::Update)?;

    let role = repository::find_role_by_public_id(pool, public_id)
        .await?
        .ok_or(UsersError::RoleNotFound)?;
    // role ที่มีสิทธิ์มากกว่าตัวเอง (เช่น admin) แก้อะไรไม่ได้เลย แม้แค่คำอธิบายหรือลดสิทธิ์
    require_within_own(actor, &role.permissions, STRONGER_ROLE)?;
    // ส่งชื่อเดิมมาด้วย (ฟอร์มแก้สิทธิ์) ไม่ใช่การเปลี่ยนชื่อ — ปฏิเสธเฉพาะชื่อที่ต่างจากเดิมจริง
    let name = name.filter(|n| *n != role.name);
    if role.is_system && name.is_some() {
        return Err(UsersError::Validation(
            "cannot rename a system role".to_string(),
        ));
    }

    let permissions = permissions.map(parse_permissions).transpose()?;
    if let Some(permissions) = &permissions {
        require_within_own(actor, permissions, GRANT_BEYOND_OWN)?;
        require_role_management_kept_after(pool, actor, role.id, permissions).await?;
    }

    repository::update_role(
        pool,
        actor.user.id,
        role.id,
        name,
        description,
        permissions.as_deref(),
    )
    .await
    .map_err(|e| map_unique_violation(e, "role name already taken"))?;
    Ok(())
}

/// กฎกันตัวเอง (คู่กับตอนแก้ role ของผู้ใช้ใน users.rs): แก้สิทธิ์ของ role ที่ตัวเองถืออยู่แล้วสิทธิ์รวมของตัวเอง
/// (role อื่นที่ถือ + สิทธิ์ชุดใหม่ของ role นี้) ต้องยังจัดการ role ได้ — role ที่ตัวเองไม่ได้ถือแก้ได้อิสระ
async fn require_role_management_kept_after(
    pool: &PgPool,
    actor: &CurrentUser,
    role_id: RoleId,
    new_permissions: &[Permission],
) -> Result<(), UsersError> {
    let held = repository::role_ids_of_user(pool, actor.user.id).await?;
    if !held.contains(&role_id) {
        return Ok(());
    }
    let roles = repository::list_roles(pool).await?;
    let from_other_roles = roles
        .iter()
        .filter(|role| role.id != role_id && held.contains(&role.id))
        .flat_map(|role| role.permissions.iter().copied());
    require_role_management_kept(from_other_roles.chain(new_permissions.iter().copied()))
}

pub async fn delete_role(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
) -> Result<(), UsersError> {
    require(actor, Resource::Role, Action::Delete)?;

    let role = repository::find_role_by_public_id(pool, public_id)
        .await?
        .ok_or(UsersError::RoleNotFound)?;
    require_within_own(actor, &role.permissions, STRONGER_ROLE)?;
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

    repository::delete_role(pool, actor.user.id, role.id).await?;
    Ok(())
}
