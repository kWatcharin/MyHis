//! เมนู 2 ชั้นผ่าน `pages.ref_id` (migration `20261012100000_page_ref_id.sql`): โครงเมนูหลัง migrate,
//! การแปลงกลุ่มเมนูเดิม (group_label) ที่ admin ตั้งผ่านหน้าเว็บ, กฎโครงเมนูของ API `/pages` และการกรอง
//! `GET /menu` ตามสิทธิ์ (docs/auth-design.md ข้อ 11)

mod common;

use axum::http::{Method, StatusCode};
use axum::Router;
use common::{insert_user_with_password, login, run_migration_files, send, test_app};
use serde_json::{json, Value};
use sqlx::PgPool;

const ADMIN_PASSWORD: &str = "admin-test-password-1";
const NURSE_PASSWORD: &str = "nurse-test-password-1";
const REF_ID_MIGRATION: i64 = 20261012100000;

/// โครงเมนูจาก DB เป็นบรรทัดอ่านง่าย: เพจหลัก = "ชื่อ route", เพจย่อย = "- ชื่อ route" ใต้เพจหลักของมัน
async fn menu_lines(pool: &PgPool) -> Vec<String> {
    let rows: Vec<(String, Option<String>, Option<i32>)> = sqlx::query_as(
        "select p.label, p.route, p.ref_id from pages p left join pages m on m.id = p.ref_id
         order by coalesce(m.sort_order, p.sort_order), coalesce(m.id, p.id),
                  p.ref_id nulls first, p.sort_order, p.id",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter()
        .map(|(label, route, ref_id)| {
            let indent = if ref_id.is_some() { "- " } else { "" };
            format!("{indent}{label} {}", route.as_deref().unwrap_or("(กลุ่ม)"))
        })
        .collect()
}

async fn set_group(pool: &PgPool, route: &str, group: &str) {
    sqlx::query("update pages set group_label = $2 where route = $1")
        .bind(route)
        .bind(group)
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test]
async fn migration_turns_menu_groups_into_main_pages(pool: PgPool) {
    assert_eq!(
        menu_lines(&pool).await,
        [
            "Dashboard /dashboard",
            "ลงทะเบียน (กลุ่ม)",
            "- ผู้ป่วย /patients",
            "- ผู้ใช้งาน /users",
            "Lab (กลุ่ม)",
            "- นัดหมายแล็ป /lab/appointments",
            "สิทธิ์ (กลุ่ม)",
            "- การตั้งค่าเพจ /settings/pages",
            "- การตั้งค่า role /settings/roles",
            "การตั้งค่า (กลุ่ม)",
            "- ตั้งค่าปีงบประมาณ /settings/budget-years",
            "- ตั้งค่าโรค /settings/diseases",
            "- ตั้งค่าคำนำหน้า /settings/prenames",
            "- ตั้งค่าสัญชาติ /settings/nationalities",
            "- ตั้งค่ากลุ่มคนไข้ /settings/patient-groups",
            "- ตั้งค่าแลป /settings/labs",
            // master คลัง (20261026100000_inventory_masters.sql) ต่อท้ายกลุ่ม
            "- ตั้งค่าหมวดคลัง /settings/item-categories",
            "- ตั้งค่าหน่วยนับ /settings/item-units",
            "- ตั้งค่ารายการคลัง /settings/items",
        ]
    );

    let group_label_columns: i64 = sqlx::query_scalar(
        "select count(*) from information_schema.columns
         where table_name = 'pages' and column_name = 'group_label'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(group_label_columns, 0);

    // เพจหลักที่เป็นหัวข้อกลุ่มไม่ผูก permission — การเห็นในเมนูขึ้นกับเพจย่อย (ดู nurse_menu_*)
    let header_permissions: i64 = sqlx::query_scalar(
        "select count(*) from page_permissions pp join pages p on p.id = pp.page_id
         where p.route is null",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(header_permissions, 0);
}

/// ข้อมูลแบบใน DB จริงที่ admin แก้ผ่านหน้าการตั้งค่าเพจ: Dashboard อยู่กลุ่ม "Home", ชื่อกลุ่มมีช่องว่างเกิน,
/// และกลุ่มเป็น '' (ฟอร์มเดิมส่ง '' ตอนไม่เลือกกลุ่ม)
#[sqlx::test(migrations = false)]
async fn migration_converts_groups_set_through_the_ui(pool: PgPool) {
    run_migration_files(&pool, |version| version < REF_ID_MIGRATION).await;
    set_group(&pool, "/dashboard", "Home").await;
    set_group(&pool, "/patients", " ลงทะเบียน ").await;
    set_group(&pool, "/settings/labs", "").await;
    run_migration_files(&pool, |version| version >= REF_ID_MIGRATION).await;

    let lines = menu_lines(&pool).await;
    assert_eq!(lines[0], "Dashboard /dashboard");
    assert!(
        !lines.iter().any(|line| line.starts_with("Home")),
        "{lines:?}"
    );
    // ชื่อกลุ่มที่ต่างกันแค่ช่องว่างรวมเป็นเพจหลักเดียว
    assert_eq!(
        lines[1..4],
        ["ลงทะเบียน (กลุ่ม)", "- ผู้ป่วย /patients", "- ผู้ใช้งาน /users"]
    );
    // กลุ่ม '' = ไม่อยู่ในกลุ่ม → เป็นเพจหลักที่มี route ไม่สร้างหัวข้อชื่อว่าง
    assert!(
        lines.contains(&"ตั้งค่าแลป /settings/labs".to_string()),
        "{lines:?}"
    );
    let headers: Vec<String> =
        sqlx::query_scalar("select label from pages where route is null order by sort_order")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(headers, ["ลงทะเบียน", "Lab", "สิทธิ์", "การตั้งค่า"]);
}

async fn menu_labels(app: &Router, cookie: &str) -> Vec<(String, Value, Value)> {
    let response = send(app, Method::GET, "/api/menu", Some(cookie), None).await;
    assert_eq!(response.status, StatusCode::OK);
    response
        .body
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            (
                item["label"].as_str().unwrap().to_string(),
                item["route"].clone(),
                item["refId"].clone(),
            )
        })
        .collect()
}

/// nurse ไม่มีสิทธิ์หน้าตั้งค่า — ไม่เห็นหัวข้อ "สิทธิ์"/"การตั้งค่า" ที่ไม่มีเพจย่อยให้เห็น และในกลุ่ม "ลงทะเบียน"
/// เห็นแค่ "ผู้ป่วย" ที่ refId ชี้ไปหัวข้อของกลุ่ม
#[sqlx::test]
async fn nurse_menu_hides_groups_without_visible_pages(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "nurse.one", NURSE_PASSWORD, "nurse").await;
    let (cookie, _) = login(&app, "nurse.one", NURSE_PASSWORD).await;

    let menu = menu_labels(&app, &cookie).await;
    let labels: Vec<&str> = menu.iter().map(|(label, _, _)| label.as_str()).collect();
    assert_eq!(
        labels,
        ["Dashboard", "ลงทะเบียน", "ผู้ป่วย", "Lab", "นัดหมายแล็ป"]
    );

    let response = send(&app, Method::GET, "/api/menu", Some(&cookie), None).await;
    let items = response.body.as_array().unwrap();
    assert_eq!(items[0]["route"], "/dashboard");
    assert_eq!(items[0]["refId"], Value::Null);
    assert_eq!(items[1]["route"], Value::Null);
    assert_eq!(items[2]["refId"], items[1]["id"]);

    // permission บนหัวข้อกลุ่มคุมทั้งกลุ่ม — ผูก user.view ให้ "Lab" แล้ว nurse ไม่เห็นทั้งกลุ่ม
    sqlx::query(
        "insert into page_permissions (page_id, permission)
         select id, 'user.view' from pages where label = 'Lab' and route is null",
    )
    .execute(&pool)
    .await
    .unwrap();
    let labels: Vec<String> = menu_labels(&app, &cookie)
        .await
        .into_iter()
        .map(|(label, _, _)| label)
        .collect();
    assert_eq!(labels, ["Dashboard", "ลงทะเบียน", "ผู้ป่วย"]);
}

#[sqlx::test]
async fn admin_menu_has_every_page_in_menu_order(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (cookie, _) = login(&app, "admin.one", ADMIN_PASSWORD).await;

    let labels: Vec<String> = menu_labels(&app, &cookie)
        .await
        .into_iter()
        .map(|(label, _, _)| label)
        .collect();
    let expected: Vec<String> = menu_lines(&pool)
        .await
        .into_iter()
        .map(|line| {
            let line = line.trim_start_matches("- ");
            line[..line.rfind(' ').unwrap()].to_string()
        })
        .collect();
    assert_eq!(labels, expected);
}

struct Admin {
    app: Router,
    cookie: String,
}

impl Admin {
    async fn new(pool: &PgPool) -> Self {
        let app = test_app(pool.clone()).await;
        insert_user_with_password(pool, "admin.one", ADMIN_PASSWORD, "admin").await;
        let (cookie, _) = login(&app, "admin.one", ADMIN_PASSWORD).await;
        Self { app, cookie }
    }

    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let response = send(&self.app, method, path, Some(&self.cookie), body).await;
        (response.status, response.body)
    }

    /// page จาก GET /api/pages ด้วย label
    async fn page(&self, label: &str) -> Value {
        let (status, body) = self.call(Method::GET, "/api/pages", None).await;
        assert_eq!(status, StatusCode::OK);
        body.as_array()
            .unwrap()
            .iter()
            .find(|page| page["label"] == label)
            .unwrap_or_else(|| panic!("page {label} not found"))
            .clone()
    }

    async fn page_id(&self, label: &str) -> String {
        self.page(label).await["id"].as_str().unwrap().to_string()
    }
}

fn error_of(body: &Value) -> &str {
    body["error"].as_str().unwrap_or_default()
}

#[sqlx::test]
async fn pages_api_creates_groups_and_sub_pages(pool: PgPool) {
    let admin = Admin::new(&pool).await;

    // ไม่ส่ง route = เพจหลักที่เป็นหัวข้อกลุ่ม
    let (status, _) = admin
        .call(
            Method::POST,
            "/api/pages",
            Some(json!({ "label": "รายงาน", "sortOrder": 90, "permissions": [] })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let group = admin.page("รายงาน").await;
    assert_eq!(group["route"], Value::Null);
    assert_eq!(group["refId"], Value::Null);

    let (status, _) = admin
        .call(
            Method::POST,
            "/api/pages",
            Some(json!({
                "label": "รายงานประจำเดือน", "route": "/reports/monthly", "permissions": [],
                "refId": group["id"],
            })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(admin.page("รายงานประจำเดือน").await["refId"], group["id"]);

    // PATCH ที่ไม่ส่ง refId ไม่ย้ายเพจ, ส่ง null = ย้ายขึ้นเป็นเพจหลัก
    let sub_id = admin.page_id("รายงานประจำเดือน").await;
    let path = format!("/api/pages/{sub_id}");
    let (status, _) = admin
        .call(Method::PATCH, &path, Some(json!({ "sortOrder": 5 })))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(admin.page("รายงานประจำเดือน").await["refId"], group["id"]);
    let (status, _) = admin
        .call(Method::PATCH, &path, Some(json!({ "refId": null })))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(admin.page("รายงานประจำเดือน").await["refId"], Value::Null);

    // ย้ายกลับเข้ากลุ่มด้วย PATCH
    let (status, _) = admin
        .call(Method::PATCH, &path, Some(json!({ "refId": group["id"] })))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(admin.page("รายงานประจำเดือน").await["refId"], group["id"]);
}

#[sqlx::test]
async fn pages_api_keeps_the_menu_two_levels(pool: PgPool) {
    let admin = Admin::new(&pool).await;
    let dashboard = admin.page_id("Dashboard").await;
    let registration = admin.page_id("ลงทะเบียน").await;
    let lab = admin.page_id("Lab").await;
    let patients = admin.page_id("ผู้ป่วย").await;

    let create = |ref_id: Value, route: Value| json!({ "label": "ทดสอบ", "route": route, "permissions": [], "refId": ref_id });
    let cases = [
        // อยู่ใต้เพจที่มี route (Dashboard) หรือใต้เพจย่อย (ผู้ป่วย) ไม่ได้
        (
            create(json!(dashboard), json!("/t1")),
            StatusCode::BAD_REQUEST,
            "main page must be a top-level page without a route",
        ),
        (
            create(json!(patients), json!("/t2")),
            StatusCode::BAD_REQUEST,
            "main page must be a top-level page without a route",
        ),
        (
            create(json!("0190a000-0000-7000-8000-000000000000"), json!("/t3")),
            StatusCode::BAD_REQUEST,
            "main page not found",
        ),
        (
            create(json!(lab), Value::Null),
            StatusCode::BAD_REQUEST,
            "sub page requires a route",
        ),
        (
            create(json!(lab), json!("/patients")),
            StatusCode::CONFLICT,
            "route already taken",
        ),
    ];
    for (body, status, error) in cases {
        let (got_status, got_body) = admin
            .call(Method::POST, "/api/pages", Some(body.clone()))
            .await;
        assert_eq!((got_status, error_of(&got_body)), (status, error), "{body}");
    }

    let lab_path = format!("/api/pages/{lab}");
    let patch_cases = [
        // หัวข้อกลุ่มที่มีเพจย่อยย้ายไปอยู่ใต้กลุ่มอื่น (เมนู 3 ชั้น) หรือมี route ไม่ได้
        (
            json!({ "refId": registration }),
            StatusCode::CONFLICT,
            "page has sub pages",
        ),
        (
            json!({ "route": "/lab" }),
            StatusCode::CONFLICT,
            "page has sub pages",
        ),
        (
            json!({ "refId": lab }),
            StatusCode::BAD_REQUEST,
            "page cannot be its own main page",
        ),
    ];
    for (body, status, error) in patch_cases {
        let (got_status, got_body) = admin
            .call(Method::PATCH, &lab_path, Some(body.clone()))
            .await;
        assert_eq!((got_status, error_of(&got_body)), (status, error), "{body}");
    }

    // ลบหัวข้อที่ยังมีเพจย่อยไม่ได้ — ย้ายเพจย่อยออกก่อนแล้วลบได้
    let (status, body) = admin.call(Method::DELETE, &lab_path, None).await;
    assert_eq!(
        (status, error_of(&body)),
        (StatusCode::CONFLICT, "page has sub pages")
    );
    let appointments = admin.page_id("นัดหมายแล็ป").await;
    let (status, _) = admin
        .call(
            Method::PATCH,
            &format!("/api/pages/{appointments}"),
            Some(json!({ "refId": registration })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = admin.call(Method::DELETE, &lab_path, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(admin.page("นัดหมายแล็ป").await["refId"], json!(registration));
}

#[sqlx::test]
async fn nurse_cannot_manage_pages(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "nurse.one", NURSE_PASSWORD, "nurse").await;
    let (cookie, _) = login(&app, "nurse.one", NURSE_PASSWORD).await;
    let response = send(
        &app,
        Method::POST,
        "/api/pages",
        Some(&cookie),
        Some(json!({ "label": "กลุ่ม", "permissions": [] })),
    )
    .await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);

    // จัดลำดับใช้สิทธิ์ page.update — ปฏิเสธก่อนตรวจรายการ
    let response = send(
        &app,
        Method::PUT,
        "/api/pages/order",
        Some(&cookie),
        Some(json!({ "refId": null, "ids": ["00000000-0000-7000-8000-000000000000"] })),
    )
    .await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
}

/// เพจของระดับหนึ่งตามลำดับจาก GET /api/pages (`ref_id` null = เพจหลักชั้นบน) — (ชื่อ, uuid)
async fn level(admin: &Admin, ref_id: &Value) -> Vec<(String, Value)> {
    let (status, body) = admin.call(Method::GET, "/api/pages", None).await;
    assert_eq!(status, StatusCode::OK);
    body.as_array()
        .unwrap()
        .iter()
        .filter(|page| &page["refId"] == ref_id)
        .map(|page| {
            (
                page["label"].as_str().unwrap().to_string(),
                page["id"].clone(),
            )
        })
        .collect()
}

fn labels(pages: &[(String, Value)]) -> Vec<String> {
    pages.iter().map(|(label, _)| label.clone()).collect()
}

fn ids(pages: &[(String, Value)]) -> Vec<Value> {
    pages.iter().map(|(_, id)| id.clone()).collect()
}

fn last_label(pages: &[(String, Value)]) -> &str {
    &pages.last().unwrap().0
}

/// แถว audit ของการจัดลำดับ (target_id, metadata) ตามลำดับที่เขียน
async fn order_audits(pool: &PgPool) -> Vec<(Option<String>, Value)> {
    sqlx::query_as(
        "select target_id, metadata from audit_log where action = 'page_order_updated' order by id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn reorder_pages_rewrites_one_level(pool: PgPool) {
    let admin = Admin::new(&pool).await;

    // เพจหลักชั้นบน: กลับลำดับทั้งหมด — path คงที่ /pages/order ต้องไม่ถูก /pages/{id} แย่งไป
    let top = level(&admin, &Value::Null).await;
    let mut reversed = top.clone();
    reversed.reverse();
    let (status, _) = admin
        .call(
            Method::PUT,
            "/api/pages/order",
            Some(json!({ "refId": null, "ids": ids(&reversed) })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        labels(&level(&admin, &Value::Null).await),
        labels(&reversed)
    );
    let menu_top: Vec<String> = menu_labels(&admin.app, &admin.cookie)
        .await
        .into_iter()
        .filter(|(_, _, ref_id)| ref_id.is_null())
        .map(|(label, _, _)| label)
        .collect();
    assert_eq!(menu_top, labels(&reversed));
    let (_, pages) = admin.call(Method::GET, "/api/pages", None).await;
    let sort_orders: Vec<i64> = pages
        .as_array()
        .unwrap()
        .iter()
        .filter(|page| page["refId"].is_null())
        .map(|page| page["sortOrder"].as_i64().unwrap())
        .collect();
    let expected: Vec<i64> = (1..=sort_orders.len() as i64).map(|n| n * 10).collect();
    assert_eq!(sort_orders, expected);

    // เพจย่อยในกลุ่ม: ย้ายตัวสุดท้ายขึ้นบนสุด
    let group = admin.page("การตั้งค่า").await["id"].clone();
    let subs = level(&admin, &group).await;
    let mut moved = subs.clone();
    moved.rotate_right(1);
    let (status, _) = admin
        .call(
            Method::PUT,
            "/api/pages/order",
            Some(json!({ "refId": group, "ids": ids(&moved) })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(labels(&level(&admin, &group).await), labels(&moved));

    // audit แถวเดียวต่อการจัดลำดับหนึ่งครั้ง เก็บชื่อตามลำดับเดิม→ใหม่ (ชั้นบนไม่มี target_id)
    let audits = order_audits(&pool).await;
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[0].0, None);
    assert_eq!(audits[0].1["label"], "เมนูชั้นบน");
    assert_eq!(audits[0].1["before"]["order"], json!(labels(&top)));
    assert_eq!(audits[0].1["after"]["order"], json!(labels(&reversed)));
    assert_eq!(audits[1].0.as_deref(), group.as_str());
    assert_eq!(audits[1].1["label"], "การตั้งค่า");
    assert_eq!(audits[1].1["after"]["order"], json!(labels(&moved)));

    // ลำดับเท่าเดิม (ไม่ส่ง refId = ชั้นบน) ไม่เขียนอะไร
    let (status, _) = admin
        .call(
            Method::PUT,
            "/api/pages/order",
            Some(json!({ "ids": ids(&reversed) })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(order_audits(&pool).await.len(), 2);
}

#[sqlx::test]
async fn reorder_pages_rejects_stale_or_mixed_lists(pool: PgPool) {
    let admin = Admin::new(&pool).await;
    let top = level(&admin, &Value::Null).await;
    let top_ids = ids(&top);
    let group = admin.page("การตั้งค่า").await["id"].clone();
    let sub_ids = ids(&level(&admin, &group).await);

    let missing = top_ids[..top_ids.len() - 1].to_vec();
    let extra = [top_ids.clone(), vec![sub_ids[0].clone()]].concat();
    let mut duplicate = top_ids.clone();
    *duplicate.last_mut().unwrap() = top_ids[0].clone();
    let stale = "page order out of date";
    let not_group = "main page must be a top-level page without a route";
    let cases = [
        (
            json!({ "refId": null, "ids": missing }),
            StatusCode::CONFLICT,
            stale,
        ),
        (
            json!({ "refId": null, "ids": extra }),
            StatusCode::CONFLICT,
            stale,
        ),
        (
            json!({ "refId": null, "ids": duplicate }),
            StatusCode::CONFLICT,
            stale,
        ),
        // เพจย่อยของกลุ่มส่งมาเป็นชั้นบน (และกลับกัน) = ลากข้ามระดับ
        (
            json!({ "refId": null, "ids": sub_ids }),
            StatusCode::CONFLICT,
            stale,
        ),
        (
            json!({ "refId": group, "ids": top_ids }),
            StatusCode::CONFLICT,
            stale,
        ),
        (
            json!({ "refId": admin.page_id("Dashboard").await, "ids": top_ids }),
            StatusCode::BAD_REQUEST,
            not_group,
        ),
        (
            json!({ "refId": admin.page_id("ตั้งค่าโรค").await, "ids": top_ids }),
            StatusCode::BAD_REQUEST,
            not_group,
        ),
        (
            json!({ "refId": "00000000-0000-7000-8000-000000000000", "ids": top_ids }),
            StatusCode::BAD_REQUEST,
            "main page not found",
        ),
    ];
    for (body, expected_status, expected_error) in cases {
        let (status, response) = admin
            .call(Method::PUT, "/api/pages/order", Some(body.clone()))
            .await;
        assert_eq!(status, expected_status, "{body}");
        assert_eq!(error_of(&response), expected_error, "{body}");
    }
    let (status, _) = admin
        .call(
            Method::PUT,
            "/api/pages/order",
            Some(json!({ "refId": null, "ids": [] })),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    assert_eq!(labels(&level(&admin, &Value::Null).await), labels(&top));
    assert!(order_audits(&pool).await.is_empty());
}

#[sqlx::test]
async fn new_and_moved_pages_go_to_the_end_of_their_level(pool: PgPool) {
    let admin = Admin::new(&pool).await;
    let create = |body: Value| {
        let admin = &admin;
        async move {
            let (status, _) = admin.call(Method::POST, "/api/pages", Some(body)).await;
            assert_eq!(status, StatusCode::CREATED);
        }
    };

    // ไม่ส่ง sortOrder = ต่อท้ายระดับของมัน (เดิมเป็น 0 = ขึ้นบนสุด)
    create(json!({ "label": "รายงาน", "permissions": [] })).await;
    assert_eq!(last_label(&level(&admin, &Value::Null).await), "รายงาน");
    let settings = admin.page("การตั้งค่า").await["id"].clone();
    create(json!({
        "label": "ตั้งค่าใหม่", "route": "/settings/new", "permissions": [], "refId": settings,
    }))
    .await;
    assert_eq!(last_label(&level(&admin, &settings).await), "ตั้งค่าใหม่");

    // บันทึกฟอร์มโดยส่ง refId เดิม (หน้าเว็บส่งทุกครั้ง) ต้องไม่ย้ายตำแหน่ง
    let before = labels(&level(&admin, &settings).await);
    let disease = admin.page_id("ตั้งค่าโรค").await;
    let path = format!("/api/pages/{disease}");
    let (status, _) = admin
        .call(
            Method::PATCH,
            &path,
            Some(json!({ "label": "ตั้งค่าโรค", "refId": settings })),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(labels(&level(&admin, &settings).await), before);

    // ย้ายกลุ่ม = ต่อท้ายกลุ่มใหม่, ย้ายขึ้นเป็นเพจหลัก = ต่อท้ายชั้นบน
    let reports = admin.page("รายงาน").await["id"].clone();
    create(json!({
        "label": "รายงานประจำเดือน", "route": "/reports/monthly", "permissions": [], "refId": reports,
    }))
    .await;
    let (status, _) = admin
        .call(Method::PATCH, &path, Some(json!({ "refId": reports })))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        labels(&level(&admin, &reports).await),
        ["รายงานประจำเดือน", "ตั้งค่าโรค"]
    );
    let (status, _) = admin
        .call(Method::PATCH, &path, Some(json!({ "refId": null })))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(last_label(&level(&admin, &Value::Null).await), "ตั้งค่าโรค");
}

/// route ของเมนูต้องเป็น path ภายในแอป: `//host`, `/\host`, `https://host`, `javascript:` ฯลฯ ถูกปฏิเสธทั้งตอนสร้างและตอนแก้
/// (ผู้จัดการเพจที่ไม่ใช่ admin ใช้เปลี่ยนลิงก์ในแถบข้างของ admin ให้พาออกนอกโดเมนไม่ได้) และไม่มีอะไรถูกบันทึกเมื่อถูกปฏิเสธ
#[sqlx::test]
async fn page_routes_must_be_paths_inside_the_app(pool: PgPool) {
    let admin = Admin::new(&pool).await;
    let roles = admin.page_id("การตั้งค่า role").await;
    let before = admin.page("การตั้งค่า role").await["route"].clone();

    for route in [
        "//attacker.invalid/settings/roles",
        r"/\attacker.invalid/y",
        "https://attacker.invalid/x",
        "javascript:alert(1)",
        " /x",
        "/a b",
        "relative/path",
    ] {
        let (status, body) = admin
            .call(
                Method::POST,
                "/api/pages",
                Some(json!({ "label": "ลิงก์ภายนอก", "route": route, "permissions": [] })),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "create {route:?}");
        assert_eq!(error_of(&body), "route must be a path inside the app");

        let (status, _) = admin
            .call(
                Method::PATCH,
                &format!("/api/pages/{roles}"),
                Some(json!({ "route": route })),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "update {route:?}");
    }
    assert_eq!(admin.page("การตั้งค่า role").await["route"], before);
    let (_, all) = admin.call(Method::GET, "/api/pages", None).await;
    assert!(all
        .as_array()
        .unwrap()
        .iter()
        .all(|page| page["label"] != "ลิงก์ภายนอก"));

    // path ภายในแอปยังสร้าง/แก้ได้ตามปกติ
    let (status, _) = admin
        .call(
            Method::POST,
            "/api/pages",
            Some(json!({ "label": "รายงานใหม่", "route": "/reports/new", "permissions": [] })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
}

/// ข้อมูลเมนูที่ผู้ถือ `page.create` ใส่ได้ต้องเล็ก (security audit รอบ 2): label ≤ 100, icon ≤ 64, เพจทั้งระบบ ≤ 300 — เมนูถูกอ่านทั้งตาราง
/// ทุกครั้งที่ใครเปิดแอป ผู้ถือสิทธิ์ย่อยจึงขับต้นทุนของ endpoint ที่ทุกคนเรียกไม่ได้
#[sqlx::test]
async fn page_text_and_count_are_bounded(pool: PgPool) {
    let admin = Admin::new(&pool).await;
    let page = |label: &str, icon: Option<&str>, route: &str| json!({ "label": label, "icon": icon, "route": route, "permissions": [] });

    // label / icon ที่ยาวเกิน → 400 ทั้งตอนสร้างและแก้ไข ค่าที่พอดีผ่าน
    let long_label = "ก".repeat(101);
    let long_icon = "i".repeat(65);
    for (body, what) in [
        (page(&long_label, None, "/bound-1"), "label"),
        (page("ปกติ", Some(&long_icon), "/bound-2"), "icon"),
    ] {
        let (status, response) = admin.call(Method::POST, "/api/pages", Some(body)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{what}: {response}");
    }
    let exact_label = "ก".repeat(100);
    let exact_icon = "i".repeat(64);
    let (status, created) = admin
        .call(
            Method::POST,
            "/api/pages",
            Some(page(&exact_label, Some(&exact_icon), "/bound-3")),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = admin.page_id(&exact_label).await;
    for patch in [json!({ "label": long_label }), json!({ "icon": long_icon })] {
        let (status, _) = admin
            .call(Method::PATCH, &format!("/api/pages/{id}"), Some(patch))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    // เพดานจำนวนเพจ: เติมให้ครบ 300 โดยตรงใน DB แล้วสร้างอีกหนึ่งต้องถูกปฏิเสธ
    let existing: i64 = sqlx::query_scalar("select count(*) from pages")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "insert into pages (label, route, sort_order)
         select 'fill ' || g, '/fill-' || g, 1000 + g from generate_series(1, 300 - $1::int) g",
    )
    .bind(i32::try_from(existing).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = admin
        .call(
            Method::POST,
            "/api/pages",
            Some(page("เกินเพดาน", None, "/over")),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(error_of(&body), "too many pages");
}
