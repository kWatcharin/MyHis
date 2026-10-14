//! ประวัติการใช้งาน: ทุกหน้าบันทึก เพิ่ม/แก้ไข/ลบ ลง audit_log (หน้าตั้งค่า master, role, เพจ — นอกจากที่มีอยู่เดิม)
//! การส่งออกเอกสารนัดหมายแล็ปบันทึกผ่าน `POST /api/lab-visit-groups/{id}/exports` (ตรวจ `lab_visit.export`)
//! และ `GET /api/audit/activities` (สิทธิ์ audit.view) คืนทุก action ยกเว้นเข้า/ออก พร้อมชื่อรายการ —
//! **แถวผู้ป่วยไม่มีชื่อรายการ และ HN/ชื่อคนไข้ไม่อยู่ใน response เลย**

mod common;

use axum::http::{Method, StatusCode};
use axum::Router;
use chrono::{Duration, Utc};
use common::{insert_patient, insert_prename, insert_user_with_password, login, send, test_app};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const ADMIN_PASSWORD: &str = "admin-test-password-1";
const VIEWER_PASSWORD: &str = "viewer-test-password-1";

/// admin ที่ login แล้ว — คืน (cookie, id ภายใน, uuid)
async fn admin_session(pool: &PgPool, app: &Router) -> (String, i32, Uuid) {
    let (id, uuid) = insert_user_with_password(pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (cookie, _) = login(app, "admin.one", ADMIN_PASSWORD).await;
    (cookie, id, uuid)
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

/// uuid ของรายการในหน้า list ที่ `field` ตรงกับ `value`
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

/// ประวัติการใช้งานรอบ "วันนี้" ตามเวลาไทย (เผื่อวันก่อน/หลังกันเทสรันคร่อมเที่ยงคืน)
async fn activities_around_today(app: &Router, cookie: &str) -> (Value, String) {
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

/// (action, targetLabel) ของแถวที่ targetType ตรงกัน เรียงตาม response (ใหม่→เก่า)
fn events_of(body: &Value, target_type: &str) -> Vec<(String, Option<String>)> {
    body["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["targetType"] == target_type)
        .map(|e| {
            (
                e["action"].as_str().unwrap().to_string(),
                e["targetLabel"].as_str().map(str::to_string),
            )
        })
        .collect()
}

fn owned(rows: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
    rows.iter()
        .map(|(a, l)| (a.to_string(), l.map(str::to_string)))
        .collect()
}

#[sqlx::test]
async fn master_pages_record_create_update_delete(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (cookie, _, _) = admin_session(&pool, &app).await;

    // (path, target_type, ฟิลด์ชื่อใน list, ชื่อ, body ตอนสร้าง)
    let masters = [
        (
            "/api/budget-years",
            "budget_year",
            "year",
            "2570",
            json!({ "year": "2570", "startDate": "2026-10-01", "endDate": "2027-09-30" }),
        ),
        (
            "/api/diseases",
            "disease",
            "name",
            "โรคทดสอบ",
            json!({ "name": "โรคทดสอบ", "abbreviation": "TT" }),
        ),
        (
            "/api/prenames",
            "prename",
            "name",
            "คำนำหน้าทดสอบ",
            json!({ "name": "คำนำหน้าทดสอบ" }),
        ),
        (
            "/api/nationalities",
            "nationality",
            "nationalName",
            "สัญชาติทดสอบ",
            json!({ "nationalName": "สัญชาติทดสอบ", "nation": "ทดสอบ" }),
        ),
        (
            "/api/patient-groups",
            "patient_group",
            "name",
            "กลุ่มทดสอบ",
            json!({ "name": "กลุ่มทดสอบ" }),
        ),
        (
            "/api/labs",
            "lab",
            "name",
            "แลปทดสอบ",
            json!({ "name": "แลปทดสอบ" }),
        ),
    ];

    for (path, target_type, field, name, body) in &masters {
        call(&app, &cookie, Method::POST, path, Some(body.clone())).await;
        let id = find_id(&app, &cookie, path, field, name).await;
        let item = format!("{path}/{id}");
        call(
            &app,
            &cookie,
            Method::PATCH,
            &item,
            Some(json!({ "sortOrder": 9 })),
        )
        .await;
        call(&app, &cookie, Method::DELETE, &item, None).await;

        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            "select action, target_id from audit_log where target_type = $1 order by id",
        )
        .bind(target_type)
        .fetch_all(&pool)
        .await
        .unwrap();
        let expected: Vec<(String, Option<String>)> = ["created", "updated", "deleted"]
            .iter()
            .map(|verb| (format!("{target_type}_{verb}"), Some(id.clone())))
            .collect();
        assert_eq!(rows, expected, "{target_type}");
    }

    // ชื่อรายการอ่านจาก join — แถวที่ถูก soft delete แล้วยังได้ชื่อ
    let (body, _) = activities_around_today(&app, &cookie).await;
    for (_, target_type, _, name, _) in &masters {
        let expected = ["deleted", "updated", "created"]
            .iter()
            .map(|verb| (format!("{target_type}_{verb}"), Some(name.to_string())))
            .collect::<Vec<_>>();
        assert_eq!(events_of(&body, target_type), expected, "{target_type}");
    }
}

#[sqlx::test]
async fn roles_and_pages_keep_their_name_after_delete(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (cookie, _, _) = admin_session(&pool, &app).await;

    call(
        &app,
        &cookie,
        Method::POST,
        "/api/roles",
        Some(json!({ "name": "ผู้ช่วยทดสอบ", "permissions": ["patient.view"] })),
    )
    .await;
    let role = find_id(&app, &cookie, "/api/roles", "name", "ผู้ช่วยทดสอบ").await;
    let role_path = format!("/api/roles/{role}");
    call(
        &app,
        &cookie,
        Method::PATCH,
        &role_path,
        Some(json!({ "name": "ผู้ช่วยชื่อใหม่" })),
    )
    .await;
    call(&app, &cookie, Method::DELETE, &role_path, None).await;

    call(
        &app,
        &cookie,
        Method::POST,
        "/api/pages",
        Some(json!({ "label": "หัวข้อทดสอบ", "permissions": [] })),
    )
    .await;
    let page = find_id(&app, &cookie, "/api/pages", "label", "หัวข้อทดสอบ").await;
    let page_path = format!("/api/pages/{page}");
    call(
        &app,
        &cookie,
        Method::PATCH,
        &page_path,
        Some(json!({ "label": "หัวข้อชื่อใหม่" })),
    )
    .await;
    call(&app, &cookie, Method::DELETE, &page_path, None).await;

    let targets: Vec<(String, Option<String>)> = sqlx::query_as(
        "select action, target_id from audit_log where target_type in ('role', 'page') order by id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(targets.len(), 6);
    assert!(targets[..3]
        .iter()
        .all(|(_, id)| id.as_deref() == Some(role.as_str())));
    assert!(targets[3..]
        .iter()
        .all(|(_, id)| id.as_deref() == Some(page.as_str())));

    let (body, _) = activities_around_today(&app, &cookie).await;
    assert_eq!(
        events_of(&body, "role"),
        owned(&[
            ("role_deleted", Some("ผู้ช่วยชื่อใหม่")),
            ("role_updated", Some("ผู้ช่วยชื่อใหม่")),
            ("role_created", Some("ผู้ช่วยทดสอบ")),
        ])
    );
    assert_eq!(
        events_of(&body, "page"),
        owned(&[
            ("page_deleted", Some("หัวข้อชื่อใหม่")),
            ("page_updated", Some("หัวข้อชื่อใหม่")),
            ("page_created", Some("หัวข้อทดสอบ")),
        ])
    );
}

/// ปีงบ 2570 + กลุ่ม "กลุ่มทดสอบนัด" แล้วสร้างนัดหมายผ่าน API — คืน uuid ของนัดหมาย
async fn create_lab_group(pool: &PgPool, app: &Router, cookie: &str, user_id: i32) -> String {
    let year: Uuid = sqlx::query_scalar(
        "insert into budget_year (year, start_date, end_date, created_by, updated_by)
         values ('2570', '2026-10-01', '2027-09-30', $1, $1) returning uuid",
    )
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
            "budgetYearId": year,
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
async fn export_requires_permission_and_is_recorded(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (cookie, admin_id, _) = admin_session(&pool, &app).await;
    let group = create_lab_group(&pool, &app, &cookie, admin_id).await;
    let export = |id: &str| format!("/api/lab-visit-groups/{id}/exports");
    let cd4_excel = json!({ "document": "cd4", "format": "excel" });

    // ดูนัดหมาย + ผู้ป่วยได้ แต่ไม่มีสิทธิ์ส่งออก
    sqlx::query(
        "with r as (insert into roles (name) values ('ดูอย่างเดียว') returning id)
         insert into role_permissions (role_id, permission)
         select r.id, unnest(array['lab_visit.view', 'patient.view']) from r",
    )
    .execute(&pool)
    .await
    .unwrap();
    insert_user_with_password(&pool, "viewer.one", VIEWER_PASSWORD, "ดูอย่างเดียว").await;
    let (viewer, _) = login(&app, "viewer.one", VIEWER_PASSWORD).await;
    let denied = send(
        &app,
        Method::POST,
        &export(&group),
        Some(&viewer),
        Some(cd4_excel.clone()),
    )
    .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);

    for bad in [
        json!({ "document": "other", "format": "excel" }),
        json!({ "document": "cd4", "format": "csv" }),
    ] {
        let response = send(
            &app,
            Method::POST,
            &export(&group),
            Some(&cookie),
            Some(bad),
        )
        .await;
        assert_eq!(
            response.status,
            StatusCode::BAD_REQUEST,
            "{}",
            response.body
        );
    }
    let unknown = send(
        &app,
        Method::POST,
        &export(&Uuid::new_v4().to_string()),
        Some(&cookie),
        Some(cd4_excel.clone()),
    )
    .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);

    let ok = send(
        &app,
        Method::POST,
        &export(&group),
        Some(&cookie),
        Some(cd4_excel.clone()),
    )
    .await;
    assert_eq!(ok.status, StatusCode::NO_CONTENT, "{}", ok.body);
    let roster_pdf = json!({ "document": "roster", "format": "pdf" });
    call(
        &app,
        &cookie,
        Method::POST,
        &export(&group),
        Some(roster_pdf.clone()),
    )
    .await;

    let rows: Vec<(Option<String>, Option<Value>)> = sqlx::query_as(
        "select target_id, metadata from audit_log
         where action = 'lab_visit_group_exported' order by id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        [
            (Some(group.clone()), Some(cd4_excel)),
            (Some(group.clone()), Some(roster_pdf))
        ]
    );

    let (body, _) = activities_around_today(&app, &cookie).await;
    let exports: Vec<&Value> = body["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["action"] == "lab_visit_group_exported")
        .collect();
    assert_eq!(exports.len(), 2);
    assert_eq!(exports[0]["exportDocument"], "roster");
    assert_eq!(exports[0]["exportFormat"], "pdf");
    assert_eq!(exports[1]["exportDocument"], "cd4");
    assert_eq!(exports[1]["exportFormat"], "excel");
    assert_eq!(exports[0]["targetLabel"], "ปีงบ 2570 · กลุ่มทดสอบนัด");
    assert_eq!(exports[0]["user"]["username"], "admin.one");
    // แถวอื่นไม่มีช่องส่งออก
    let created = body["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["action"] == "lab_visit_group_created")
        .unwrap();
    assert_eq!(created["exportDocument"], Value::Null);

    // นัดหมายที่ถูกลบแล้วส่งออกไม่ได้
    call(
        &app,
        &cookie,
        Method::DELETE,
        &format!("/api/lab-visit-groups/{group}"),
        None,
    )
    .await;
    let gone = send(
        &app,
        Method::POST,
        &export(&group),
        Some(&cookie),
        Some(json!({ "document": "cd4", "format": "pdf" })),
    )
    .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
}

#[sqlx::test]
async fn activities_hide_patient_identity_and_exclude_auth_events(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (cookie, admin_id, _) = admin_session(&pool, &app).await;
    let prename: Uuid = sqlx::query_scalar(
        "insert into prename (name, created_by, updated_by) values ('นาย', $1, $1) returning uuid",
    )
    .bind(admin_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // ข้อมูลสมมติ — ค่าที่ห้ามโผล่ในประวัติ
    let secrets = ["HN-ACT-4242", "NAP-ACT-4242", "ชื่อลับทดสอบ", "สกุลลับทดสอบ"];
    let patient = json!({
        "hn": secrets[0],
        "napNo": secrets[1],
        "prenameId": prename,
        "firstName": secrets[2],
        "lastName": secrets[3],
        "sex": 1,
        "isThai": true,
    });
    call(
        &app,
        &cookie,
        Method::POST,
        "/api/patients",
        Some(patient.clone()),
    )
    .await;
    let id = find_id(&app, &cookie, "/api/patients", "hn", secrets[0]).await;
    let mut updated = patient;
    updated["age"] = json!(41);
    call(
        &app,
        &cookie,
        Method::PUT,
        &format!("/api/patients/{id}"),
        Some(updated),
    )
    .await;
    call(
        &app,
        &cookie,
        Method::DELETE,
        &format!("/api/patients/{id}"),
        None,
    )
    .await;

    // คนไข้ในนัดหมาย: ชื่อรายการเป็นปีงบ + กลุ่ม ไม่ใช่ตัวคนไข้
    let group = create_lab_group(&pool, &app, &cookie, admin_id).await;
    let prename_id = insert_prename(&pool, admin_id, "นางสาว").await;
    let in_group = insert_patient(&pool, admin_id, prename_id, None).await;
    let in_group_uuid: Uuid = sqlx::query_scalar("select uuid from patient where id = $1")
        .bind(in_group)
        .fetch_one(&pool)
        .await
        .unwrap();
    call(
        &app,
        &cookie,
        Method::POST,
        &format!("/api/lab-visit-groups/{group}/visits"),
        Some(json!({ "patientIds": [in_group_uuid] })),
    )
    .await;

    let (body, text) = activities_around_today(&app, &cookie).await;
    assert_eq!(
        events_of(&body, "patient"),
        owned(&[
            ("patient_deleted", None),
            ("patient_updated", None),
            ("patient_created", None),
        ])
    );
    for secret in secrets {
        assert!(!text.contains(secret), "{secret} leaked: {text}");
    }
    // uuid ของรายการไม่ถูกส่งออก (มีแค่ชนิด + ชื่อรายการ)
    assert!(!text.contains(&id), "patient uuid leaked");
    assert_eq!(
        events_of(&body, "lab_visit"),
        owned(&[("lab_visit_created", Some("ปีงบ 2570 · กลุ่มทดสอบนัด"))])
    );
    let actions: Vec<&str> = body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["action"].as_str().unwrap())
        .collect();
    assert!(!actions.contains(&"login_succeeded"), "{actions:?}");
}

async fn insert_event(pool: &PgPool, action: &str, target_type: &str, target: &str, at: &str) {
    sqlx::query(
        "insert into audit_log (actor_user_id, action, target_type, target_id, created_at)
         values ((select id from users where username = 'admin.one'), $1, $2, $3, $4::timestamptz)",
    )
    .bind(action)
    .bind(target_type)
    .bind(target)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
async fn activities_exclude_auth_events_and_filter_by_bangkok_day(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (cookie, _, admin_uuid) = admin_session(&pool, &app).await;
    let admin = admin_uuid.to_string();
    let missing = Uuid::new_v4().to_string();

    // 15 ม.ค. 2026 ตามเวลาไทย = 2026-01-14T17:00Z ถึงก่อน 2026-01-15T17:00Z
    insert_event(
        &pool,
        "user_updated",
        "user",
        &admin,
        "2026-01-14T17:00:00Z",
    )
    .await;
    // target_id ที่ไม่ใช่ uuid (admin คนแรกที่สร้างตอนเริ่มระบบ) ต้องไม่ทำให้ query พัง
    insert_event(
        &pool,
        "user_created",
        "user",
        "admin.one",
        "2026-01-15T03:00:00Z",
    )
    .await;
    for auth in ["login_succeeded", "login_failed", "logout", "logout_all"] {
        insert_event(&pool, auth, "user", &admin, "2026-01-15T04:00:00Z").await;
    }
    insert_event(
        &pool,
        "disease_created",
        "disease",
        &missing,
        "2026-01-15T16:59:59Z",
    )
    .await; // 23:59 น.
    insert_event(
        &pool,
        "disease_updated",
        "disease",
        &missing,
        "2026-01-15T17:00:00Z",
    )
    .await; // วันถัดไป
    insert_event(
        &pool,
        "disease_deleted",
        "disease",
        &missing,
        "2026-01-14T16:59:59Z",
    )
    .await; // วันก่อน

    let response = send(
        &app,
        Method::GET,
        "/api/audit/activities?from=2026-01-15&to=2026-01-15",
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    assert_eq!(response.body["truncated"], false);
    let events: Vec<(&str, Option<&str>)> = response.body["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["action"].as_str().unwrap(), e["targetLabel"].as_str()))
        .collect();
    assert_eq!(
        events,
        [
            ("disease_created", None), // ไม่พบรายการ = ไม่มีชื่อ
            ("user_created", None),
            ("user_updated", Some("admin.one (admin.one)")),
        ]
    );

    // ไม่มี audit.view → 403, ช่วงวันผิด → 400
    insert_user_with_password(&pool, "nurse.one", VIEWER_PASSWORD, "nurse").await;
    let (nurse, _) = login(&app, "nurse.one", VIEWER_PASSWORD).await;
    let forbidden = send(
        &app,
        Method::GET,
        "/api/audit/activities?from=2026-01-15&to=2026-01-15",
        Some(&nurse),
        None,
    )
    .await;
    assert_eq!(forbidden.status, StatusCode::FORBIDDEN);
    for query in [
        "from=2026-01-16&to=2026-01-15",
        "from=2025-01-01&to=2026-01-02",
        "from=2026-01-15",
    ] {
        let response = send(
            &app,
            Method::GET,
            &format!("/api/audit/activities?{query}"),
            Some(&cookie),
            None,
        )
        .await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{query}");
    }
}
