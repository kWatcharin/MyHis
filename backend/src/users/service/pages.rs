//! page/menu: รายชื่อ, สร้าง, แก้, ลบ page และคำนวณเมนูของผู้ใช้ปัจจุบัน (docs/auth-design.md ข้อ 11)

use sqlx::PgPool;
use uuid::Uuid;

use super::{map_unique_violation, parse_permissions, require_user_manage, UsersError};
use crate::auth::CurrentUser;
use crate::domain::PageId;
use crate::users::repository::{self, UpdatePageFields};

pub async fn list_pages(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<repository::PageWithPermissions>, UsersError> {
    require_user_manage(actor)?;
    Ok(repository::list_pages(pool).await?)
}

/// พารามิเตอร์สร้าง page — รวมเป็น struct เดียวเพราะแยกเป็น argument จะเกิน clippy's
/// too_many_arguments (7) ตรงกับแพทเทิร์นเดียวกับ `UpdatePageInput` ด้านล่าง
pub struct CreatePageInput<'a> {
    pub label: &'a str,
    pub icon: Option<&'a str>,
    pub route: &'a str,
    pub sort_order: i32,
    pub permissions: &'a [String],
    pub group_label: Option<&'a str>,
}

pub async fn create_page(
    pool: &PgPool,
    actor: &CurrentUser,
    input: CreatePageInput<'_>,
) -> Result<PageId, UsersError> {
    require_user_manage(actor)?;
    let permissions = parse_permissions(input.permissions)?;

    repository::insert_page(
        pool,
        input.label,
        input.icon,
        input.route,
        input.sort_order,
        &permissions,
        input.group_label,
    )
    .await
    .map_err(|e| map_unique_violation(e, "route already taken"))
}

pub struct UpdatePageInput<'a> {
    pub label: Option<&'a str>,
    pub icon: Option<Option<&'a str>>,
    pub route: Option<&'a str>,
    pub sort_order: Option<i32>,
    pub permissions: Option<&'a [String]>,
    pub group_label: Option<Option<&'a str>>,
}

pub async fn update_page(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: UpdatePageInput<'_>,
) -> Result<(), UsersError> {
    require_user_manage(actor)?;

    let page = repository::find_page_by_public_id(pool, public_id)
        .await?
        .ok_or(UsersError::PageNotFound)?;
    let permissions = input.permissions.map(parse_permissions).transpose()?;

    repository::update_page(
        pool,
        page.id,
        UpdatePageFields {
            label: input.label,
            icon: input.icon,
            route: input.route,
            sort_order: input.sort_order,
            permissions: permissions.as_deref(),
            group_label: input.group_label,
        },
    )
    .await
    .map_err(|e| map_unique_violation(e, "route already taken"))?;
    Ok(())
}

pub async fn delete_page(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
) -> Result<(), UsersError> {
    require_user_manage(actor)?;
    let page = repository::find_page_by_public_id(pool, public_id)
        .await?
        .ok_or(UsersError::PageNotFound)?;
    repository::delete_page(pool, page.id).await?;
    Ok(())
}

// ============================================================================
// menu — docs/auth-design.md หัวข้อ 11
// ============================================================================

pub struct MenuItem {
    pub public_id: Uuid,
    pub label: String,
    pub icon: Option<String>,
    pub route: String,
    pub group_label: Option<String>,
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
            public_id: page.public_id,
            label: page.label,
            icon: page.icon,
            route: page.route,
            group_label: page.group_label,
        })
        .collect())
}
