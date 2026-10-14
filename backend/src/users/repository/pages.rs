use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::{insert_labeled_audit, LabeledAudit, RepositoryError};
use crate::domain::{changes, PageId, Permission, UserId};

#[derive(Debug, Clone)]
pub struct PageWithPermissions {
    pub id: PageId,
    pub public_id: Uuid,
    pub label: String,
    pub icon: Option<String>,
    /// None = เพจหลักที่เป็นหัวข้อกลุ่มเมนู (ไม่มีหน้าของตัวเอง)
    pub route: Option<String>,
    pub sort_order: i32,
    pub permissions: Vec<Permission>,
    /// เพจหลักที่เพจนี้อยู่ใต้ (None = เป็นเพจหลักเอง) — id ภายในใช้ตรวจโครงเมนู, uuid ส่งออก API
    pub ref_id: Option<PageId>,
    pub ref_public_id: Option<Uuid>,
}

/// รายชื่อ page ทั้งหมดพร้อม permission เรียงตามโครงเมนู: เพจหลักตาม sort_order แต่ละอันตามด้วยเพจย่อย
/// ของมัน (ตาม sort_order) — ใช้ทั้งสำหรับ `GET /pages` (admin) และคำนวณ `GET /menu` (กรองตามสิทธิ์ผู้ใช้
/// ต่อใน service) ตารางเล็ก ดึงมาแล้วกรองใน Rust ง่ายกว่าเขียน SQL ที่มี array ตัดกันซับซ้อน
/// (docs/auth-design.md ข้อ 11)
pub async fn list_pages(pool: &PgPool) -> Result<Vec<PageWithPermissions>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select p.id as "id: PageId", p.uuid as public_id, p.label, p.icon, p.route, p.sort_order,
                  p.ref_id as "ref_id: PageId", m.uuid as "ref_public_id?",
                  pp.permission as "permission?"
           from pages p
           left join pages m on m.id = p.ref_id
           left join page_permissions pp on pp.page_id = p.id
           order by coalesce(m.sort_order, p.sort_order), coalesce(m.id, p.id),
                    p.ref_id nulls first, p.sort_order, p.id, pp.permission"#
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
                public_id: row.public_id,
                label: row.label,
                icon: row.icon,
                route: row.route,
                sort_order: row.sort_order,
                permissions: permission.into_iter().collect(),
                ref_id: row.ref_id,
                ref_public_id: row.ref_public_id,
            }),
        }
    }
    Ok(pages)
}

/// ค่าของเพจใหม่ — รวมเป็น struct เพราะแยกเป็น argument จะเกิน clippy's too_many_arguments (7)
pub struct NewPageFields<'a> {
    pub label: &'a str,
    pub icon: Option<&'a str>,
    pub route: Option<&'a str>,
    pub sort_order: i32,
    pub permissions: &'a [Permission],
    pub ref_id: Option<PageId>,
}

/// สร้างเพจ + permission + audit ในทรานแซกชันเดียว
pub async fn insert_page(
    pool: &PgPool,
    actor: UserId,
    fields: NewPageFields<'_>,
) -> Result<PageId, RepositoryError> {
    let mut tx = pool.begin().await?;

    let page = sqlx::query!(
        r#"insert into pages (label, icon, route, sort_order, ref_id)
           values ($1, $2, $3, $4, $5)
           returning id as "id: PageId", uuid"#,
        fields.label,
        fields.icon,
        fields.route,
        fields.sort_order,
        fields.ref_id.map(|id| id.0)
    )
    .fetch_one(&mut *tx)
    .await?;

    for permission in fields.permissions {
        sqlx::query!(
            "insert into page_permissions (page_id, permission) values ($1, $2)",
            page.id.0,
            permission.as_str()
        )
        .execute(&mut *tx)
        .await?;
    }

    let after = snapshot(&mut tx, page.id).await?;
    insert_labeled_audit(
        &mut tx,
        actor,
        LabeledAudit {
            action: "page_created",
            target_type: "page",
            target_public_id: page.uuid,
            label: fields.label,
            details: Some(changes::created(after)),
        },
    )
    .await?;

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
    /// เหมือน icon — `Some(None)` = ย้ายขึ้นเป็นเพจหลัก
    pub ref_id: Option<Option<PageId>>,
}

/// แก้เพจ + audit (label = ชื่อหลังแก้) ในทรานแซกชันเดียว
pub async fn update_page(
    pool: &PgPool,
    actor: UserId,
    id: PageId,
    fields: UpdatePageFields<'_>,
) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let before = snapshot(&mut tx, id).await?;

    if let Some(icon) = fields.icon {
        sqlx::query!(
            "update pages set icon = $2, updated_at = now() where id = $1",
            id.0,
            icon
        )
        .execute(&mut *tx)
        .await?;
    }

    if let Some(ref_id) = fields.ref_id {
        sqlx::query!(
            "update pages set ref_id = $2, updated_at = now() where id = $1",
            id.0,
            ref_id.map(|id| id.0)
        )
        .execute(&mut *tx)
        .await?;
    }

    let updated = sqlx::query!(
        r#"update pages
           set label = coalesce($2, label),
               route = coalesce($3, route),
               sort_order = coalesce($4, sort_order),
               updated_at = now()
           where id = $1
           returning uuid, label"#,
        id.0,
        fields.label,
        fields.route,
        fields.sort_order
    )
    .fetch_optional(&mut *tx)
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

    if let Some(page) = &updated {
        let after = snapshot(&mut tx, id).await?;
        insert_labeled_audit(
            &mut tx,
            actor,
            LabeledAudit {
                action: "page_updated",
                target_type: "page",
                target_public_id: page.uuid,
                label: &page.label,
                details: Some(changes::updated(&before, &after)),
            },
        )
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

/// ลบจริง (เพจไม่มี soft delete) — audit เก็บชื่อไว้ใน label ให้ประวัติยังแสดงชื่อได้หลังลบ
pub async fn delete_page(pool: &PgPool, actor: UserId, id: PageId) -> Result<(), RepositoryError> {
    let mut tx = pool.begin().await?;
    let deleted = sqlx::query!(
        "delete from pages where id = $1 returning uuid, label",
        id.0
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(page) = &deleted {
        insert_labeled_audit(
            &mut tx,
            actor,
            LabeledAudit {
                action: "page_deleted",
                target_type: "page",
                target_public_id: page.uuid,
                label: &page.label,
                details: None,
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// ค่าของเพจสำหรับรายละเอียดในประวัติการใช้งาน (เพจหลักเป็นชื่อ ไม่ใช่ id, สิทธิ์เรียงแล้ว) — ล็อกแถวไว้จนจบทรานแซกชัน
async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    id: PageId,
) -> Result<Value, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"select jsonb_build_object(
                  'label', p.label,
                  'icon', p.icon,
                  'route', p.route,
                  'main_page', m.label,
                  'sort_order', p.sort_order,
                  'permissions', coalesce((select jsonb_agg(pp.permission order by pp.permission)
                                           from page_permissions pp where pp.page_id = p.id), '[]'::jsonb)
              ) as "snapshot!"
           from pages p left join pages m on m.id = p.ref_id
           where p.id = $1 for update of p"#,
        id.0
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.unwrap_or(Value::Null))
}
