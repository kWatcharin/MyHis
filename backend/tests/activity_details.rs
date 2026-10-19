//! รายละเอียดการเปลี่ยนแปลงในประวัติการใช้งาน (`details` ของ `GET /api/audit/activities` — ปุ่ม "ดู"):
//! เพิ่ม = ค่าที่เพิ่ม, แก้ไข = ค่าเดิม→ใหม่เฉพาะช่องที่เปลี่ยน (FK เป็นชื่อ ไม่มี id int), ลบ = ไม่มี
//! **ข้อมูลผู้ป่วยเก็บแค่ชื่อช่องที่เปลี่ยน — ค่าไม่อยู่ทั้งใน response และในตาราง audit_log**

mod common;

use axum::http::{Method, StatusCode};
use axum::Router;
use chrono::{Duration, Utc};
use common::{insert_user_with_password, login, send, test_app};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const ADMIN_PASSWORD: &str = "admin-test-password-1";

async fn admin_session(pool: &PgPool, app: &Router) -> (String, i32) {
    let (id, _) = insert_user_with_password(pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (cookie, _) = login(app, "admin.one", ADMIN_PASSWORD).await;
    (cookie, id)
}

async fn call(
    app: &Router,
    cookie: &str,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Value {
    let response = send(app, method.clone(), path, Some(cookie), body).await;
    assert!(
        response.status.is_success(),
        "{method} {path} → {} {}",
        response.status,
        response.body
    );
    response.body
}

async fn find_id(app: &Router, cookie: &str, path: &str, field: &str, value: &str) -> String {
    let list = call(app, cookie, Method::GET, path, None).await;
    list.as_array()
        .unwrap()
        .iter()
        .find(|item| item[field] == value)
        .unwrap_or_else(|| panic!("{value:?} not in {path}"))["id"]
        .as_str()
        .unwrap()
        .to_string()
}

/// ประวัติการใช้งานรอบวันนี้ (เวลาไทย) — คืน (body, ข้อความ body ทั้งก้อน)
async fn activities(app: &Router, cookie: &str) -> (Value, String) {
    let today = (Utc::now() + Duration::hours(7)).date_naive();
    let path = format!(
        "/api/audit/activities?from={}&to={}",
        today - Duration::days(1),
        today + Duration::days(1)
    );
    let response = send(app, Method::GET, &path, Some(cookie), None).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    let text = response.body.to_string();
    (response.body, text)
}

/// details ของทุกแถวที่ action ตรงกัน เรียงใหม่→เก่า
fn details_of(body: &Value, action: &str) -> Vec<Value> {
    body["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["action"] == action)
        .map(|e| e["details"].clone())
        .collect()
}

fn keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

#[sqlx::test]
async fn master_create_update_delete_details(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (cookie, _) = admin_session(&pool, &app).await;

    call(
        &app,
        &cookie,
        Method::POST,
        "/api/diseases",
        Some(json!({ "name": "โรคทดสอบ", "abbreviation": "TT" })),
    )
    .await;
    let id = find_id(&app, &cookie, "/api/diseases", "name", "โรคทดสอบ").await;
    let item = format!("/api/diseases/{id}");
    call(
        &app,
        &cookie,
        Method::PATCH,
        &item,
        Some(json!({ "abbreviation": "TT2", "sortOrder": 9 })),
    )
    .await;
    call(
        &app,
        &cookie,
        Method::PATCH,
        &item,
        Some(json!({ "sortOrder": 9 })),
    )
    .await; // ไม่มีค่าที่เปลี่ยน
    call(&app, &cookie, Method::DELETE, &item, None).await;

    let (body, _) = activities(&app, &cookie).await;
    let created = &details_of(&body, "disease_created")[0];
    assert_eq!(
        keys(&created["after"]),
        ["abbreviation", "is_active", "name", "sort_order"]
    );
    assert_eq!(created["after"]["name"], "โรคทดสอบ");
    assert_eq!(created["after"]["abbreviation"], "TT");
    assert!(created.get("before").is_none());

    let updates = details_of(&body, "disease_updated");
    assert_eq!(updates.len(), 2);
    // ใหม่→เก่า: [ไม่มีค่าที่เปลี่ยน, แก้ชื่อย่อ + ลำดับ]
    assert_eq!(updates[0], json!({ "before": {}, "after": {} }));
    assert_eq!(keys(&updates[1]["before"]), ["abbreviation", "sort_order"]);
    assert_eq!(updates[1]["before"]["abbreviation"], "TT");
    assert_eq!(
        updates[1]["after"],
        json!({ "abbreviation": "TT2", "sort_order": 9 })
    );

    assert_eq!(details_of(&body, "disease_deleted"), [Value::Null]);
}

#[sqlx::test]
async fn role_and_page_details(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (cookie, _) = admin_session(&pool, &app).await;

    call(
        &app,
        &cookie,
        Method::POST,
        "/api/roles",
        Some(json!({ "name": "ผู้ช่วยทดสอบ", "permissions": ["patient.view"] })),
    )
    .await;
    let role = find_id(&app, &cookie, "/api/roles", "name", "ผู้ช่วยทดสอบ").await;
    call(
        &app,
        &cookie,
        Method::PATCH,
        &format!("/api/roles/{role}"),
        Some(json!({ "permissions": ["patient.view", "patient.update"] })),
    )
    .await;

    call(
        &app,
        &cookie,
        Method::POST,
        "/api/pages",
        Some(json!({ "label": "กลุ่มทดสอบ", "permissions": [] })),
    )
    .await;
    call(&app, &cookie, Method::POST, "/api/pages",
         Some(json!({ "label": "รายงานทดสอบ", "route": "/t-report", "permissions": ["patient.view"] }))).await;
    let group = find_id(&app, &cookie, "/api/pages", "label", "กลุ่มทดสอบ").await;
    let page = find_id(&app, &cookie, "/api/pages", "label", "รายงานทดสอบ").await;
    call(
        &app,
        &cookie,
        Method::PATCH,
        &format!("/api/pages/{page}"),
        Some(json!({ "refId": group })),
    )
    .await;

    let (body, _) = activities(&app, &cookie).await;
    assert_eq!(
        details_of(&body, "role_created")[0]["after"],
        json!({ "name": "ผู้ช่วยทดสอบ", "description": null, "permissions": ["patient.view"] })
    );
    assert_eq!(
        details_of(&body, "role_updated")[0],
        json!({
            "before": { "permissions": ["patient.view"] },
            "after": { "permissions": ["patient.update", "patient.view"] },
        })
    );
    let page_created = details_of(&body, "page_created");
    assert_eq!(page_created[0]["after"]["route"], "/t-report");
    assert_eq!(
        page_created[0]["after"]["permissions"],
        json!(["patient.view"])
    );
    // เพจหลักเป็นชื่อ ไม่ใช่ id — ย้ายกลุ่มแล้วไปต่อท้ายกลุ่มใหม่ (กลุ่มว่าง = ลำดับ 10) ลำดับจึงเปลี่ยนด้วย
    let page_updated = &details_of(&body, "page_updated")[0];
    assert_eq!(page_updated["before"]["main_page"], Value::Null);
    assert_eq!(page_updated["after"]["main_page"], "กลุ่มทดสอบ");
    assert_eq!(page_updated["after"]["sort_order"], 10);
    // ชื่อรายการ (label) ยังอยู่
    let labels: Vec<&str> = body["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["action"] == "page_updated")
        .map(|e| e["targetLabel"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["รายงานทดสอบ"]);
}

#[sqlx::test]
async fn user_details_use_names_not_ids(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (cookie, admin_id) = admin_session(&pool, &app).await;
    let prename: Uuid = sqlx::query_scalar(
        "insert into prename (name, created_by, updated_by) values ('นาง', $1, $1) returning uuid",
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let nurse_role: Uuid = sqlx::query_scalar("select uuid from roles where name = 'nurse'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let admin_role: Uuid = sqlx::query_scalar("select uuid from roles where name = 'admin'")
        .fetch_one(&pool)
        .await
        .unwrap();

    call(
        &app,
        &cookie,
        Method::POST,
        "/api/users",
        Some(json!({ "username": "staff.one", "displayName": "เจ้าหน้าที่", "roleId": nurse_role })),
    )
    .await;
    let user: Uuid = sqlx::query_scalar("select uuid from users where username = 'staff.one'")
        .fetch_one(&pool)
        .await
        .unwrap();
    call(
        &app,
        &cookie,
        Method::PUT,
        &format!("/api/users/{user}"),
        Some(json!({
            "displayName": "เจ้าหน้าที่ใหม่",
            "prenameId": prename,
            "firstName": "สมศรี",
            "lastName": "ใจดี",
            "position": null,
            "roleIds": [nurse_role, admin_role],
        })),
    )
    .await;

    let (body, text) = activities(&app, &cookie).await;
    assert_eq!(
        details_of(&body, "user_created")[0]["after"],
        json!({ "username": "staff.one", "display_name": "เจ้าหน้าที่", "roles": ["nurse"] })
    );
    assert_eq!(
        details_of(&body, "user_updated")[0],
        json!({
            "before": { "display_name": "เจ้าหน้าที่", "prename": null, "first_name": null, "last_name": null },
            "after": { "display_name": "เจ้าหน้าที่ใหม่", "prename": "นาง", "first_name": "สมศรี", "last_name": "ใจดี" },
        })
    );
    assert_eq!(
        details_of(&body, "user_roles_replaced")[0],
        json!({ "before": { "roles": ["nurse"] }, "after": { "roles": ["admin", "nurse"] } })
    );
    assert!(!text.contains(&prename.to_string()), "prename uuid leaked");
}

/// ปีงบ + กลุ่ม แล้วสร้างนัดหมาย — คืน uuid ของนัดหมาย
async fn create_lab_group(
    pool: &PgPool,
    app: &Router,
    cookie: &str,
    user_id: i32,
    year: &str,
) -> String {
    let year_id: Uuid = sqlx::query_scalar(
        "insert into budget_year (year, start_date, end_date, created_by, updated_by)
         values ($1, '2026-10-01', '2027-09-30', $2, $2) returning uuid",
    )
    .bind(year)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let group: Uuid = sqlx::query_scalar(
        "insert into patient_group (name, created_by, updated_by)
         values ('กลุ่มทดสอบนัด', $1, $1) returning uuid",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let created = call(
        app,
        cookie,
        Method::POST,
        "/api/lab-visit-groups",
        Some(json!({
            "budgetYearId": year_id,
            "patientGroupId": group,
            "cd4FirstDate": "2026-11-02",
            "cd4SecondDate": "2027-05-03",
            "vlDate": "2026-11-02",
        })),
    )
    .await;
    created["id"].as_str().unwrap().to_string()
}

#[sqlx::test]
async fn lab_group_details_show_year_and_group_names(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (cookie, admin_id) = admin_session(&pool, &app).await;
    let group = create_lab_group(&pool, &app, &cookie, admin_id, "2569").await;
    let year_2570: Uuid = sqlx::query_scalar(
        "insert into budget_year (year, start_date, end_date, created_by, updated_by)
         values ('2570', '2027-10-01', '2028-09-30', $1, $1) returning uuid",
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let detail = call(
        &app,
        &cookie,
        Method::GET,
        &format!("/api/lab-visit-groups/{group}"),
        None,
    )
    .await;
    call(
        &app,
        &cookie,
        Method::PUT,
        &format!("/api/lab-visit-groups/{group}"),
        Some(json!({
            "budgetYearId": year_2570,
            "patientGroupId": detail["patientGroupId"],
            "cd4FirstDate": "2026-11-02",
            "cd4SecondDate": "2027-05-03",
            "vlDate": "2026-11-09",
        })),
    )
    .await;

    let (body, _) = activities(&app, &cookie).await;
    let created = &details_of(&body, "lab_visit_group_created")[0]["after"];
    assert_eq!(created["budget_year"], "2569");
    assert_eq!(created["patient_group"], "กลุ่มทดสอบนัด");
    assert_eq!(created["cd4_first_date"], "2026-11-02");
    assert_eq!(
        details_of(&body, "lab_visit_group_updated")[0],
        json!({
            "before": { "budget_year": "2569", "vl_date": "2026-11-02" },
            "after": { "budget_year": "2570", "vl_date": "2026-11-09" },
        })
    );
}

/// ทั้งตาราง audit_log เป็นข้อความเดียว (ไว้ตรวจว่าค่าข้อมูลผู้ป่วยไม่ถูกเก็บ)
async fn whole_audit_log(pool: &PgPool) -> String {
    sqlx::query_scalar::<_, Option<String>>(
        "select string_agg(coalesce(target_id, '') || ' ' || coalesce(metadata::text, ''), ' | ') from audit_log",
    )
    .fetch_one(pool)
    .await
    .unwrap()
    .unwrap_or_default()
}

#[sqlx::test]
async fn patient_and_lab_visit_details_have_field_names_only(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (cookie, admin_id) = admin_session(&pool, &app).await;
    let prename: Uuid = sqlx::query_scalar(
        "insert into prename (name, created_by, updated_by) values ('นาย', $1, $1) returning uuid",
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // ข้อมูลสมมติ — ค่าที่ห้ามโผล่ในประวัติ
    let secrets = [
        "HN-DET-9191",
        "NAP-DET-9191",
        "ชื่อลับเดิม",
        "ชื่อลับใหม่",
        "สกุลลับคงเดิม",
        "63.37",
        "71.29",
        "VL-SECRET-4321",
    ];
    let patient = |first: &str, weight: f64, age: i32| {
        json!({
            "hn": secrets[0], "napNo": secrets[1], "prenameId": prename,
            "firstName": first, "lastName": secrets[4], "age": age, "weight": weight,
            "sex": 1, "isThai": true,
        })
    };
    call(
        &app,
        &cookie,
        Method::POST,
        "/api/patients",
        Some(patient(secrets[2], 63.37, 40)),
    )
    .await;
    let id = find_id(&app, &cookie, "/api/patients", "hn", secrets[0]).await;
    let path = format!("/api/patients/{id}");
    call(
        &app,
        &cookie,
        Method::PUT,
        &path,
        Some(patient(secrets[3], 71.29, 40)),
    )
    .await; // ชื่อ + น้ำหนัก
    call(
        &app,
        &cookie,
        Method::PUT,
        &path,
        Some(patient(secrets[3], 71.29, 41)),
    )
    .await; // อายุอย่างเดียว

    // คนไข้ในนัดหมาย: แก้ผล VL
    let group = create_lab_group(&pool, &app, &cookie, admin_id, "2569").await;
    call(
        &app,
        &cookie,
        Method::POST,
        &format!("/api/lab-visit-groups/{group}/visits"),
        Some(json!({ "patientIds": [id] })),
    )
    .await;
    let detail = call(
        &app,
        &cookie,
        Method::GET,
        &format!("/api/lab-visit-groups/{group}"),
        None,
    )
    .await;
    let visit = detail["visits"][0]["id"].as_str().unwrap().to_string();
    call(&app, &cookie, Method::PUT, &format!("/api/lab-visits/{visit}"), Some(json!({
        "takeMedYearRange": null, "takeMedMonthRange": null, "vlResult": secrets[7], "drugRecipe": null,
    }))).await;

    let (body, text) = activities(&app, &cookie).await;
    assert_eq!(
        details_of(&body, "patient_updated"),
        [
            json!({ "changed": ["age"] }),
            json!({ "changed": ["first_name", "weight"] })
        ]
    );
    assert_eq!(details_of(&body, "patient_created"), [Value::Null]);
    assert_eq!(
        details_of(&body, "lab_visit_updated"),
        [json!({ "changed": ["vl_result"] })]
    );
    assert_eq!(details_of(&body, "lab_visit_created"), [Value::Null]);

    let stored = whole_audit_log(&pool).await;
    for secret in secrets {
        assert!(!text.contains(secret), "{secret} leaked in API: {text}");
        assert!(
            !stored.contains(secret),
            "{secret} stored in audit_log: {stored}"
        );
    }
}
