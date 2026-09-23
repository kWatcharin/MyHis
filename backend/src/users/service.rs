//! กฎทั้งหมดของ users module: สิทธิ์ (ต้องมี user.manage), invariant (≥1 role เสมอ), lockout
//! ของ role/page (is_system, in-use) ห้าม import ชนิดข้อมูลของ Axum (docs/auth-design.md ข้อ 10)
//!
//! **จุดตรวจสิทธิ์**: ทุกฟังก์ชันรับ `actor: &CurrentUser` เป็นพารามิเตอร์แรกและเช็ค `user.manage`
//! ที่นี่ — "Service เป็นที่ตรวจ permission จริง" (docs/auth-design.md ข้อ 10)

use sqlx::PgPool;

use crate::auth::CurrentUser;
use crate::domain::{PageId, Permission, RoleId, UserId};
use crate::password;
use crate::users::repository::{self, RepositoryError, UpdatePageFields};

const TEMP_PASSWORD_LEN: usize = 16;
const TEMP_PASSWORD_CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789"; // ตัด 0/O/1/l/I ที่สับสนง่ายออก

#[derive(Debug, thiserror::Error)]
pub enum UsersError {
    #[error("forbidden")]
    Forbidden,
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("user not found")]
    UserNotFound,
    #[error("role not found")]
    RoleNotFound,
    #[error("page not found")]
    PageNotFound,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Password(#[from] password::PasswordError),
}

fn require_user_manage(actor: &CurrentUser) -> Result<(), UsersError> {
    if actor.has_permission(Permission::UserManage) {
        Ok(())
    } else {
        Err(UsersError::Forbidden)
    }
}

/// สุ่มรหัสผ่านชั่วคราวให้มนุษย์พิมพ์ได้จริง (ต่างจาก session token ที่เป็น hex ยาว 64 ตัว) —
/// ยาวพอเกินเกณฑ์ 12 ตัวของ docs/auth-design.md ข้อ 8 อยู่แล้ว
fn generate_temp_password() -> String {
    use rand::rngs::OsRng;
    use rand::RngCore;

    let mut bytes = [0u8; TEMP_PASSWORD_LEN];
    OsRng.fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|b| TEMP_PASSWORD_CHARSET[(*b as usize) % TEMP_PASSWORD_CHARSET.len()] as char)
        .collect()
}

async fn hash_password(plain: String) -> Result<String, UsersError> {
    tokio::task::spawn_blocking(move || password::hash(&plain))
        .await
        .map_err(|_| password::PasswordError::Hash)?
        .map_err(UsersError::from)
}

fn map_unique_violation(err: RepositoryError, message: &str) -> UsersError {
    if err.is_unique_violation() {
        UsersError::Conflict(message.to_string())
    } else {
        UsersError::from(err)
    }
}

/// parse permission string จาก DTO — ค่าที่ไม่รู้จัก = ผู้ใช้ส่งข้อมูลผิด ไม่ใช่ข้อมูล DB เสีย
/// (ต่างจาก RepositoryError::UnknownPermission ที่หมายถึงอ่านจาก DB แล้วเจอค่าที่ไม่รู้จัก)
fn parse_permissions(raw: &[String]) -> Result<Vec<Permission>, UsersError> {
    raw.iter()
        .map(|p| {
            Permission::parse(p)
                .ok_or_else(|| UsersError::Validation(format!("unknown permission: {p}")))
        })
        .collect()
}

// ============================================================================
// users
// ============================================================================

pub struct CreatedUser {
    pub id: UserId,
    pub username: String,
    pub display_name: String,
    pub role: repository::RoleRef,
    pub temporary_password: String,
}

pub async fn list_users(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<repository::UserWithRoles>, UsersError> {
    require_user_manage(actor)?;
    Ok(repository::list_users(pool).await?)
}

pub async fn create_user(
    pool: &PgPool,
    actor: &CurrentUser,
    username: &str,
    display_name: &str,
    role_id: RoleId,
) -> Result<CreatedUser, UsersError> {
    require_user_manage(actor)?;

    let role = repository::find_role_by_id(pool, role_id)
        .await?
        .ok_or(UsersError::RoleNotFound)?;

    let normalized_username = username.trim().to_lowercase();
    let temporary_password = generate_temp_password();
    let password_hash = hash_password(temporary_password.clone()).await?;

    let id = repository::insert_user_with_role(
        pool,
        &normalized_username,
        display_name,
        &password_hash,
        role_id,
    )
    .await
    .map_err(|e| map_unique_violation(e, "username already taken"))?;

    Ok(CreatedUser {
        id,
        username: normalized_username,
        display_name: display_name.to_string(),
        role: repository::RoleRef {
            id: role.id,
            name: role.name,
        },
        temporary_password,
    })
}

pub async fn update_user(
    pool: &PgPool,
    actor: &CurrentUser,
    id: UserId,
    display_name: Option<&str>,
    is_active: Option<bool>,
) -> Result<(), UsersError> {
    require_user_manage(actor)?;
    if !repository::user_exists(pool, id).await? {
        return Err(UsersError::UserNotFound);
    }
    repository::update_user(pool, id, display_name, is_active).await?;
    Ok(())
}

pub async fn reset_password(
    pool: &PgPool,
    actor: &CurrentUser,
    id: UserId,
) -> Result<String, UsersError> {
    require_user_manage(actor)?;
    if !repository::user_exists(pool, id).await? {
        return Err(UsersError::UserNotFound);
    }

    let temporary_password = generate_temp_password();
    let password_hash = hash_password(temporary_password.clone()).await?;

    repository::reset_password(pool, id, &password_hash).await?;
    repository::delete_all_sessions_for_user(pool, id).await?;

    Ok(temporary_password)
}

pub async fn replace_user_roles(
    pool: &PgPool,
    actor: &CurrentUser,
    id: UserId,
    role_ids: &[RoleId],
) -> Result<(), UsersError> {
    require_user_manage(actor)?;

    if role_ids.is_empty() {
        return Err(UsersError::Validation(
            "a user must have at least one role".to_string(),
        ));
    }
    if !repository::user_exists(pool, id).await? {
        return Err(UsersError::UserNotFound);
    }
    for role_id in role_ids {
        if repository::find_role_by_id(pool, *role_id).await?.is_none() {
            return Err(UsersError::RoleNotFound);
        }
    }

    repository::replace_user_roles(pool, id, role_ids).await?;
    Ok(())
}

// ============================================================================
// roles
// ============================================================================

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
    id: RoleId,
    name: Option<&str>,
    description: Option<&str>,
    permissions: Option<&[String]>,
) -> Result<(), UsersError> {
    require_user_manage(actor)?;

    let role = repository::find_role_by_id(pool, id)
        .await?
        .ok_or(UsersError::RoleNotFound)?;
    if role.is_system && name.is_some() {
        return Err(UsersError::Validation(
            "cannot rename a system role".to_string(),
        ));
    }

    let permissions = permissions.map(parse_permissions).transpose()?;

    repository::update_role(pool, id, name, description, permissions.as_deref())
        .await
        .map_err(|e| map_unique_violation(e, "role name already taken"))?;
    Ok(())
}

pub async fn delete_role(pool: &PgPool, actor: &CurrentUser, id: RoleId) -> Result<(), UsersError> {
    require_user_manage(actor)?;

    let role = repository::find_role_by_id(pool, id)
        .await?
        .ok_or(UsersError::RoleNotFound)?;
    if role.is_system {
        return Err(UsersError::Conflict(
            "cannot delete a system role".to_string(),
        ));
    }
    if repository::role_in_use(pool, id).await? {
        return Err(UsersError::Conflict(
            "role is still assigned to at least one user".to_string(),
        ));
    }

    repository::delete_role(pool, id).await?;
    Ok(())
}

// ============================================================================
// pages
// ============================================================================

pub async fn list_pages(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<repository::PageWithPermissions>, UsersError> {
    require_user_manage(actor)?;
    Ok(repository::list_pages(pool).await?)
}

pub async fn create_page(
    pool: &PgPool,
    actor: &CurrentUser,
    label: &str,
    icon: Option<&str>,
    route: &str,
    sort_order: i32,
    permissions: &[String],
) -> Result<PageId, UsersError> {
    require_user_manage(actor)?;
    let permissions = parse_permissions(permissions)?;

    repository::insert_page(pool, label, icon, route, sort_order, &permissions)
        .await
        .map_err(|e| map_unique_violation(e, "route already taken"))
}

pub struct UpdatePageInput<'a> {
    pub label: Option<&'a str>,
    pub icon: Option<Option<&'a str>>,
    pub route: Option<&'a str>,
    pub sort_order: Option<i32>,
    pub permissions: Option<&'a [String]>,
}

pub async fn update_page(
    pool: &PgPool,
    actor: &CurrentUser,
    id: PageId,
    input: UpdatePageInput<'_>,
) -> Result<(), UsersError> {
    require_user_manage(actor)?;

    if repository::find_page_by_id(pool, id).await?.is_none() {
        return Err(UsersError::PageNotFound);
    }
    let permissions = input.permissions.map(parse_permissions).transpose()?;

    repository::update_page(
        pool,
        id,
        UpdatePageFields {
            label: input.label,
            icon: input.icon,
            route: input.route,
            sort_order: input.sort_order,
            permissions: permissions.as_deref(),
        },
    )
    .await
    .map_err(|e| map_unique_violation(e, "route already taken"))?;
    Ok(())
}

pub async fn delete_page(pool: &PgPool, actor: &CurrentUser, id: PageId) -> Result<(), UsersError> {
    require_user_manage(actor)?;
    if repository::find_page_by_id(pool, id).await?.is_none() {
        return Err(UsersError::PageNotFound);
    }
    repository::delete_page(pool, id).await?;
    Ok(())
}

// ============================================================================
// menu — docs/auth-design.md หัวข้อ 11
// ============================================================================

pub struct MenuItem {
    pub id: PageId,
    pub label: String,
    pub icon: Option<String>,
    pub route: String,
}

/// กรอง page ที่ current user เข้าถึงได้: มี permission ตรงกันอย่างน้อยหนึ่งตัว (OR) หรือ page
/// ไม่ผูก permission เลย (หน้าสาธารณะ) — ไม่ต้องมี user.manage เพราะทุกคนที่ login แล้วเรียกได้
pub async fn menu_for(pool: &PgPool, actor: &CurrentUser) -> Result<Vec<MenuItem>, UsersError> {
    let pages = repository::list_pages(pool).await?;

    Ok(pages
        .into_iter()
        .filter(|page| {
            page.permissions.is_empty() || page.permissions.iter().any(|p| actor.has_permission(*p))
        })
        .map(|page| MenuItem {
            id: page.id,
            label: page.label,
            icon: page.icon,
            route: page.route,
        })
        .collect())
}
