//! page/menu: รายชื่อ, สร้าง, แก้, ลบ page และคำนวณเมนูของผู้ใช้ปัจจุบัน (docs/auth-design.md ข้อ 11)
//!
//! โครงเมนู 2 ชั้นผ่าน `pages.ref_id`: เพจหลัก (ref_id null) มี route = ลิงก์เดี่ยว หรือไม่มี route = หัวข้อกลุ่ม
//! ที่มีเพจย่อย, เพจย่อยอ้างเพจหลักที่ไม่มี route และต้องมี route เอง — กฎที่ DB ตรวจในแถวเดียวไม่ได้อยู่ที่
//! `check_menu_shape` (DB กันแค่ ref_id ชี้ตัวเอง, เพจย่อยไม่มี route และลบเพจหลักที่ยังมีเพจย่อย)

use std::collections::HashSet;

use sqlx::PgPool;
use uuid::Uuid;

use super::{map_unique_violation, parse_permissions, require, UsersError};
use crate::auth::CurrentUser;
use crate::domain::{Action, PageId, Resource};
use crate::users::repository::{
    self, NewPageFields, PageOrder, PageWithPermissions, UpdatePageFields,
};

/// ระยะห่างของ sort_order — เพจที่ต่อท้ายกลุ่ม = ค่ามากสุด + 10, เรียงใหม่ทั้งระดับ = 10, 20, 30…
const SORT_STEP: i32 = 10;

/// จำนวนเพจทั้งระบบสูงสุด — ตารางนี้ถูกอ่านทั้งก้อนทุกครั้งที่ใครเปิดเมนู (`list_pages`) ต้องเล็กเสมอ; ต้องไม่เกิน 500 ซึ่งเป็นเพดานของ
/// `ReorderPagesRequest.ids` (ไม่งั้นเรียงลำดับระดับที่ใหญ่เกินไม่ได้) นับแล้วค่อย insert จึงเป็นเพดานแบบหลวม — คำขอที่ชนกันพอดีเกินได้
/// เล็กน้อยซึ่งไม่เป็นปัญหา (ไม่ใช่ขอบเขตความปลอดภัยที่ต้องแม่น)
const MAX_PAGES: usize = 300;

pub async fn list_pages(
    pool: &PgPool,
    actor: &CurrentUser,
) -> Result<Vec<repository::PageWithPermissions>, UsersError> {
    require(actor, Resource::Page, Action::View)?;
    Ok(repository::list_pages(pool).await?)
}

/// แปลง uuid ของเพจหลักจาก API เป็น id ภายใน — ไม่พบ = ผู้ใช้ส่งค่าผิด (400) ไม่ใช่ 404 ของเพจที่กำลังแก้
fn resolve_ref(pages: &[PageWithPermissions], ref_id: Uuid) -> Result<PageId, UsersError> {
    pages
        .iter()
        .find(|p| p.public_id == ref_id)
        .map(|p| p.id)
        .ok_or_else(|| UsersError::Validation("main page not found".to_string()))
}

/// sort_order ที่ต่อท้ายเพจในระดับ `ref_id` (None = เพจหลักชั้นบน) — ใช้กับเพจใหม่และเพจที่ย้ายกลุ่ม
fn end_of_level(pages: &[PageWithPermissions], ref_id: Option<PageId>) -> i32 {
    pages
        .iter()
        .filter(|p| p.ref_id == ref_id)
        .map(|p| p.sort_order)
        .max()
        .map_or(SORT_STEP, |max| max.saturating_add(SORT_STEP))
}

/// route ของเมนูยาวสุด (ตัวอักษร)
const MAX_ROUTE_CHARS: usize = 200;

/// route ของเมนูต้องเป็น path ภายในแอป: ขึ้นต้น `/` ตัวเดียว (ไม่ใช่ `//host` หรือ `/\host` ที่เบราว์เซอร์อ่านเป็นโดเมนอื่น,
/// ไม่ใช่ `https://…` / `javascript:…`) และไม่มี `\`, ช่องว่าง, control — ค่าที่เก็บถูกส่งให้ทุกคนที่เห็นเพจ และ sidebar ทำเป็นลิงก์
/// ผู้จัดการเพจที่ไม่ใช่ admin จึงใช้ช่องนี้เปลี่ยนลิงก์ในแถบข้างของผู้อื่นให้พาออกนอกโดเมนไม่ได้ (กฎเดียวกับฟอร์ม: PageFormDialog.tsx)
fn is_in_app_route(route: &str) -> bool {
    let Some(rest) = route.strip_prefix('/') else {
        return false;
    };
    !rest.starts_with('/')
        && route.chars().count() <= MAX_ROUTE_CHARS
        && route
            .chars()
            .all(|c| c != '\\' && !c.is_whitespace() && !c.is_control())
}

fn check_route(route: Option<&str>) -> Result<(), UsersError> {
    match route {
        Some(route) if !is_in_app_route(route) => Err(UsersError::Validation(
            "route must be a path inside the app".to_string(),
        )),
        _ => Ok(()),
    }
}

/// ตรวจโครงเมนูของเพจหนึ่งหน้าหลังบันทึก (`page` = None ตอนสร้างใหม่) — `ref_id`/`has_route` คือค่าสุดท้าย
/// ที่จะลง DB ไม่ใช่แค่ field ที่ส่งมา
fn check_menu_shape(
    pages: &[PageWithPermissions],
    page: Option<PageId>,
    ref_id: Option<PageId>,
    has_route: bool,
) -> Result<(), UsersError> {
    let has_sub_pages = page.is_some_and(|id| pages.iter().any(|p| p.ref_id == Some(id)));
    match ref_id {
        Some(parent_id) => {
            if Some(parent_id) == page {
                return Err(UsersError::Validation(
                    "page cannot be its own main page".to_string(),
                ));
            }
            // เมนู 2 ชั้นเท่านั้น และเพจหลักที่มีเพจย่อยเป็นหัวข้อกลุ่ม (กดพับ/กาง) จึงต้องไม่มี route
            let parent = pages.iter().find(|p| p.id == parent_id);
            if parent.is_none_or(|p| p.ref_id.is_some() || p.route.is_some()) {
                return Err(UsersError::Validation(
                    "main page must be a top-level page without a route".to_string(),
                ));
            }
            if has_sub_pages {
                return Err(UsersError::Conflict("page has sub pages".to_string()));
            }
            if !has_route {
                return Err(UsersError::Validation(
                    "sub page requires a route".to_string(),
                ));
            }
        }
        None => {
            if has_route && has_sub_pages {
                return Err(UsersError::Conflict("page has sub pages".to_string()));
            }
        }
    }
    Ok(())
}

/// พารามิเตอร์สร้าง page — รวมเป็น struct เดียวเพราะแยกเป็น argument จะเกิน clippy's
/// too_many_arguments (7) ตรงกับแพทเทิร์นเดียวกับ `UpdatePageInput` ด้านล่าง
pub struct CreatePageInput<'a> {
    pub label: &'a str,
    pub icon: Option<&'a str>,
    /// None = เพจหลักที่เป็นหัวข้อกลุ่มเมนู
    pub route: Option<&'a str>,
    /// None = ต่อท้ายกลุ่มของมัน
    pub sort_order: Option<i32>,
    pub permissions: &'a [String],
    /// uuid ของเพจหลัก (None = เป็นเพจหลักเอง)
    pub ref_id: Option<Uuid>,
}

pub async fn create_page(
    pool: &PgPool,
    actor: &CurrentUser,
    input: CreatePageInput<'_>,
) -> Result<PageId, UsersError> {
    require(actor, Resource::Page, Action::Create)?;
    check_route(input.route)?;
    let permissions = parse_permissions(input.permissions)?;

    let pages = repository::list_pages(pool).await?;
    if pages.len() >= MAX_PAGES {
        return Err(UsersError::Conflict("too many pages".to_string()));
    }
    let ref_id = input.ref_id.map(|id| resolve_ref(&pages, id)).transpose()?;
    check_menu_shape(&pages, None, ref_id, input.route.is_some())?;

    repository::insert_page(
        pool,
        actor.user.id,
        NewPageFields {
            label: input.label,
            icon: input.icon,
            route: input.route,
            sort_order: input
                .sort_order
                .unwrap_or_else(|| end_of_level(&pages, ref_id)),
            permissions: &permissions,
            ref_id,
        },
    )
    .await
    .map_err(|e| map_unique_violation(e, "route already taken"))
}

pub struct UpdatePageInput<'a> {
    pub label: Option<&'a str>,
    pub icon: Option<Option<&'a str>>,
    /// ตั้ง route ใหม่ได้ แต่ล้างเป็น null ผ่าน endpoint นี้ไม่ได้
    pub route: Option<&'a str>,
    pub sort_order: Option<i32>,
    pub permissions: Option<&'a [String]>,
    /// None = ไม่แตะ, Some(None) = ย้ายขึ้นเป็นเพจหลัก, Some(Some(uuid)) = ย้ายไปอยู่ใต้เพจหลักนั้น
    pub ref_id: Option<Option<Uuid>>,
}

pub async fn update_page(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: UpdatePageInput<'_>,
) -> Result<(), UsersError> {
    require(actor, Resource::Page, Action::Update)?;
    check_route(input.route)?;

    let pages = repository::list_pages(pool).await?;
    let page = pages
        .iter()
        .find(|p| p.public_id == public_id)
        .ok_or(UsersError::PageNotFound)?;
    let permissions = input.permissions.map(parse_permissions).transpose()?;
    let ref_id = input
        .ref_id
        .map(|r| r.map(|id| resolve_ref(&pages, id)).transpose())
        .transpose()?;
    let new_ref_id = ref_id.unwrap_or(page.ref_id);
    check_menu_shape(
        &pages,
        Some(page.id),
        new_ref_id,
        input.route.is_some() || page.route.is_some(),
    )?;
    // ย้ายกลุ่มจริงและไม่ได้ระบุลำดับเอง = ไปต่อท้ายกลุ่มใหม่ — หน้าเว็บส่ง refId มาทุกครั้งที่บันทึก จึงต้องเทียบกับค่าเดิม
    // ไม่งั้นแค่แก้ชื่อก็เด้งไปท้ายกลุ่ม
    let sort_order = input
        .sort_order
        .or_else(|| (new_ref_id != page.ref_id).then(|| end_of_level(&pages, new_ref_id)));

    repository::update_page(
        pool,
        actor.user.id,
        page.id,
        UpdatePageFields {
            label: input.label,
            icon: input.icon,
            route: input.route,
            sort_order,
            permissions: permissions.as_deref(),
            ref_id,
        },
    )
    .await
    .map_err(|e| map_unique_violation(e, "route already taken"))?;
    Ok(())
}

/// เรียงเพจทั้งระดับใหม่ตาม `ids` (ลากวางในหน้าตั้งค่าเพจ) — `ref_id` None = เพจหลักชั้นบน, Some = เพจย่อยของหัวข้อกลุ่มนั้น
///
/// `ids` ต้องเป็นเพจของระดับนั้นครบชุดพอดี (ไม่ซ้ำ ไม่ขาด ไม่เกิน) ไม่งั้นรายการที่หน้าเว็บถืออยู่เก่าแล้ว → 409 ให้โหลดใหม่
/// ย้ายข้ามกลุ่มผ่านทางนี้ไม่ได้โดยตั้งใจ: สิทธิ์ของเพจหลักคุมทั้งกลุ่ม ย้ายกลุ่มจึงเปลี่ยนว่าใครเห็นเพจ ต้องทำผ่าน PATCH
pub async fn reorder_pages(
    pool: &PgPool,
    actor: &CurrentUser,
    ref_id: Option<Uuid>,
    ids: &[Uuid],
) -> Result<(), UsersError> {
    require(actor, Resource::Page, Action::Update)?;
    let pages = repository::list_pages(pool).await?;

    let parent = match ref_id {
        Some(id) => {
            let parent_id = resolve_ref(&pages, id)?;
            let parent = pages.iter().find(|p| p.id == parent_id);
            match parent {
                Some(p) if p.ref_id.is_none() && p.route.is_none() => Some(p),
                _ => {
                    return Err(UsersError::Validation(
                        "main page must be a top-level page without a route".to_string(),
                    ))
                }
            }
        }
        None => None,
    };
    let parent_id = parent.map(|p| p.id);

    // list_pages เรียงตามโครงเมนูอยู่แล้ว = ลำดับปัจจุบันของระดับนี้
    let current: Vec<&PageWithPermissions> =
        pages.iter().filter(|p| p.ref_id == parent_id).collect();
    let requested: Vec<&PageWithPermissions> = ids
        .iter()
        .filter_map(|id| current.iter().find(|p| p.public_id == *id).copied())
        .collect();
    let distinct: HashSet<&Uuid> = ids.iter().collect();
    if requested.len() != ids.len()
        || distinct.len() != ids.len()
        || requested.len() != current.len()
    {
        return Err(UsersError::Conflict("page order out of date".to_string()));
    }
    if requested
        .iter()
        .map(|p| p.id)
        .eq(current.iter().map(|p| p.id))
    {
        return Ok(());
    }

    let order: Vec<PageId> = requested.iter().map(|p| p.id).collect();
    repository::reorder_pages(
        pool,
        actor.user.id,
        PageOrder {
            parent: parent.map(|p| (p.id, p.public_id, p.label.as_str())),
            ids: &order,
            step: SORT_STEP,
        },
    )
    .await?;
    Ok(())
}

pub async fn delete_page(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
) -> Result<(), UsersError> {
    require(actor, Resource::Page, Action::Delete)?;
    let pages = repository::list_pages(pool).await?;
    let page = pages
        .iter()
        .find(|p| p.public_id == public_id)
        .ok_or(UsersError::PageNotFound)?;
    // ต้องย้าย/ลบเพจย่อยก่อน ไม่ลบตามกันเงียบ ๆ (FK เป็น restrict กันซ้ำอีกชั้น)
    if pages.iter().any(|p| p.ref_id == Some(page.id)) {
        return Err(UsersError::Conflict("page has sub pages".to_string()));
    }
    repository::delete_page(pool, actor.user.id, page.id).await?;
    Ok(())
}

// ============================================================================
// menu — docs/auth-design.md หัวข้อ 11
// ============================================================================

pub struct MenuItem {
    pub public_id: Uuid,
    pub label: String,
    pub icon: Option<String>,
    pub route: Option<String>,
    /// uuid ของเพจหลัก (None = เมนูชั้นบน)
    pub ref_id: Option<Uuid>,
}

/// กรอง page ที่ current user เข้าถึงได้ — endpoint นี้ไม่ต้องมีสิทธิ์ใด เพราะทุกคนที่ login แล้วเรียกได้
/// - เพจผ่านสิทธิ์เมื่อมี permission ตรงกันอย่างน้อยหนึ่งตัว (OR) หรือไม่ผูก permission เลย (หน้าสาธารณะ)
/// - เพจย่อยเห็นเมื่อทั้งตัวเองและเพจหลักผ่านสิทธิ์ (permission ของเพจหลักคุมทั้งกลุ่ม)
/// - เพจหลักที่ไม่มี route (หัวข้อกลุ่ม) เห็นเมื่อมีเพจย่อยที่เห็นได้อย่างน้อยหนึ่งหน้า ไม่โชว์หัวข้อเปล่า
///
/// หน้าเว็บกรองด้วยกฎเดียวกันตอน preview สิทธิ์ของ role อื่น (frontend/src/features/auth/menu.ts)
pub async fn menu_for(pool: &PgPool, actor: &CurrentUser) -> Result<Vec<MenuItem>, UsersError> {
    let pages = repository::list_pages(pool).await?;

    let allowed: HashSet<PageId> = pages
        .iter()
        .filter(|page| {
            page.permissions.is_empty() || page.permissions.iter().any(|p| actor.has_permission(*p))
        })
        .map(|page| page.id)
        .collect();
    let sub_page_visible = |page: &PageWithPermissions| {
        page.ref_id
            .is_some_and(|parent| allowed.contains(&page.id) && allowed.contains(&parent))
    };
    let groups_with_visible_pages: HashSet<PageId> = pages
        .iter()
        .filter(|page| sub_page_visible(page))
        .filter_map(|page| page.ref_id)
        .collect();

    Ok(pages
        .iter()
        .filter(|page| match page.ref_id {
            Some(_) => sub_page_visible(page),
            None => {
                allowed.contains(&page.id)
                    && (page.route.is_some() || groups_with_visible_pages.contains(&page.id))
            }
        })
        .map(|page| MenuItem {
            public_id: page.public_id,
            label: page.label.clone(),
            icon: page.icon.clone(),
            route: page.route.clone(),
            ref_id: page.ref_public_id,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_app_routes_are_accepted() {
        for route in [
            "/",
            "/dashboard",
            "/settings/pages",
            "/lab/appointments",
            "/a?b=1#c",
        ] {
            assert!(is_in_app_route(route), "{route}");
        }
    }

    #[test]
    fn anything_that_can_leave_the_app_or_hide_a_target_is_rejected() {
        let long = format!("/{}", "a".repeat(MAX_ROUTE_CHARS));
        for route in [
            "",
            "dashboard",
            "//attacker.invalid/x",
            r"/\attacker.invalid/x",
            "https://attacker.invalid/x",
            "javascript:alert(1)",
            " /x",
            "/a b",
            "/a\tb",
            "/a\nb",
            "/a\u{a0}b",
            "/a\u{7f}b",
            r"/a\b",
            long.as_str(),
        ] {
            assert!(!is_in_app_route(route), "{route:?}");
        }
        assert!(is_in_app_route(&format!(
            "/{}",
            "a".repeat(MAX_ROUTE_CHARS - 1)
        )));
    }

    #[test]
    fn a_missing_route_is_not_a_route_error() {
        assert!(check_route(None).is_ok());
        assert!(check_route(Some("/ok")).is_ok());
        assert!(matches!(
            check_route(Some("//bad")),
            Err(UsersError::Validation(_))
        ));
    }
}
