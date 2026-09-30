use sqlx::PgPool;

use super::RepositoryError;
use crate::domain::{PageId, Permission};

#[derive(Debug, Clone)]
pub struct PageWithPermissions {
    pub id: PageId,
    pub label: String,
    pub icon: Option<String>,
    pub route: String,
    pub sort_order: i32,
    pub permissions: Vec<Permission>,
    pub group_label: Option<String>,
}

/// รายชื่อ page ทั้งหมดพร้อม permission เรียงตาม sort_order — ใช้ทั้งสำหรับ `GET /pages` (admin)
/// และคำนวณ `GET /menu` (กรองตามสิทธิ์ผู้ใช้ต่อใน service) ตารางเล็ก ดึงมาแล้วกรองใน Rust ง่ายกว่า
/// เขียน SQL ที่มี array ตัดกันซับซ้อน (docs/auth-design.md ข้อ 11)
pub async fn list_pages(pool: &PgPool) -> Result<Vec<PageWithPermissions>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select p.id as "id: PageId", p.label, p.icon, p.route, p.sort_order, p.group_label,
                  pp.permission as "permission?"
           from pages p
           left join page_permissions pp on pp.page_id = p.id
           order by p.sort_order, p.id, pp.permission"#
    )
    .fetch_all(pool)
    .await?;

    let mut pages: Vec<PageWithPermissions> = Vec::new();
    for row in rows {
        let permission = row
            .permission
            .map(|p| Permission::parse(&p).ok_or(p))
            .transpose()
            .map_err(RepositoryError::UnknownPermission)?;

        match pages.last_mut() {
            Some(last) if last.id == row.id => {
                if let Some(p) = permission {
                    last.permissions.push(p);
                }
            }
            _ => pages.push(PageWithPermissions {
                id: row.id,
                label: row.label,
                icon: row.icon,
                route: row.route,
                sort_order: row.sort_order,
                group_label: row.group_label,
                permissions: permission.into_iter().collect(),
            }),
        }
    }
    Ok(pages)
}

pub async fn find_page_by_id(
    pool: &PgPool,
    id: PageId,
) -> Result<Option<PageWithPermissions>, RepositoryError> {
    Ok(list_pages(pool).await?.into_iter().find(|p| p.id == id))
}

pub async fn insert_page(
    pool: &PgPool,
    label: &str,
    icon: Option<&str>,
    route: &str,
    sort_order: i32,
    permissions: &[Permission],
    group_label: Option<&str>,
) -> Result<PageId, RepositoryError> {
    let mut tx = pool.begin().await?;

    let page = sqlx::query!(
        r#"insert into pages (label, icon, route, sort_order, group_label)
           values ($1, $2, $3, $4, $5)
           returning id as "id: PageId""#,
        label,
        icon,
        route,
        sort_order,
        group_label
    )
    .fetch_one(&mut *tx)
    .await?;

    for permission in permissions {
        sqlx::query!(
            "insert into page_permissions (page_id, permission) values ($1, $2)",
            page.id.0,
            permission.as_str()
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(page.id)
}

pub struct UpdatePageFields<'a> {
    pub label: Option<&'a str>,
    /// `None` = ไม่แตะ icon, `Some(None)` = ตั้งเป็น null, `Some(Some(v))` = ตั้งค่าใหม่ — nullable
    /// column ที่ต้อง "ตั้งเป็น null ได้จริง" เลยแยกจาก COALESCE ปกติที่ใช้กับ field อื่น
    pub icon: Option<Option<&'a str>>,
    pub route: Option<&'a str>,
    pub sort_order: Option<i32>,
    pub permissions: Option<&'a [Permission]>,
    /// เหมือน icon — nullable column ที่ต้องแยก "ไม่แตะ"/"ตั้งเป็น null"/"ตั้งค่าใหม่" ออกจากกัน
    pub group_label: Option<Option<&'a str>>,
}

pub async fn update_page(
    pool: &PgPool,
    id: PageId,
    fields: UpdatePageFields<'_>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;

    if let Some(icon) = fields.icon {
        sqlx::query!(
            "update pages set icon = $2, updated_at = now() where id = $1",
            id.0,
            icon
        )
        .execute(&mut *tx)
        .await?;
    }

    if let Some(group_label) = fields.group_label {
        sqlx::query!(
            "update pages set group_label = $2, updated_at = now() where id = $1",
            id.0,
            group_label
        )
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query!(
        r#"update pages
           set label = coalesce($2, label),
               route = coalesce($3, route),
               sort_order = coalesce($4, sort_order),
               updated_at = now()
           where id = $1"#,
        id.0,
        fields.label,
        fields.route,
        fields.sort_order
    )
    .execute(&mut *tx)
    .await?;

    if let Some(permissions) = fields.permissions {
        sqlx::query!("delete from page_permissions where page_id = $1", id.0)
            .execute(&mut *tx)
            .await?;
        for permission in permissions {
            sqlx::query!(
                "insert into page_permissions (page_id, permission) values ($1, $2)",
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

pub async fn delete_page(pool: &PgPool, id: PageId) -> Result<(), RepositoryError> {
    sqlx::query!("delete from pages where id = $1", id.0)
        .execute(pool)
        .await?;
    Ok(())
}
