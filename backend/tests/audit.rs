//! ประวัติการเข้าออก: login/logout บันทึก IP (ค่าขวาสุดของ X-Forwarded-For) + User-Agent ลง audit_log และ
//! `GET /api/audit/auth-events` (สิทธิ์ audit.view) คืนเฉพาะเหตุการณ์เข้า/ออก ตามช่วงวันเวลาไทย โดยไม่ส่งชื่อที่พิมพ์มา
//! ของ login ที่ไม่พบผู้ใช้

mod common;

use axum::http::{Method, StatusCode};
use common::{insert_user_with_password, login, send, send_with_headers, test_app};
use serde_json::{json, Value};
use sqlx::PgPool;

const ADMIN_PASSWORD: &str = "admin-test-password-1";
const NURSE_PASSWORD: &str = "nurse-test-password-1";
const CLIENT_HEADERS: [(&str, &str); 2] = [
    // ค่าซ้ายผู้ใช้ปลอมได้ ค่าขวาสุดคือที่ proxy เติม
    ("x-forwarded-for", "198.51.100.1, 203.0.113.9"),
    ("user-agent", "TestAgent/1.0 (Windows NT 10.0)"),
];

/// (action, host(ip), userAgent, username ของ actor)
type AuthRow = (String, Option<String>, Option<String>, Option<String>);

/// เหตุการณ์เข้า/ออกใน audit_log เรียงตามลำดับที่เขียน
async fn auth_rows(pool: &PgPool) -> Vec<AuthRow> {
    sqlx::query_as(
        "select a.action, host(a.ip), a.metadata ->> 'userAgent', u.username
         from audit_log a left join users u on u.id = a.actor_user_id
         where a.action in ('login_succeeded', 'login_failed', 'logout', 'logout_all')
         order by a.id",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn login_and_logout_record_ip_and_user_agent(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "nurse.one", NURSE_PASSWORD, "nurse").await;

    let response = send_with_headers(
        &app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": "nurse.one", "password": NURSE_PASSWORD })),
        &CLIENT_HEADERS,
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    let cookie = response.set_cookie.unwrap();

    let wrong = send_with_headers(
        &app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": "nurse.one", "password": "wrong-password-123" })),
        &CLIENT_HEADERS,
    )
    .await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);

    let logout = send_with_headers(
        &app,
        Method::POST,
        "/api/auth/logout",
        Some(&cookie),
        None,
        &CLIENT_HEADERS,
    )
    .await;
    assert!(logout.status.is_success(), "{:?}", logout.status);

    let expected = |action: &str| {
        (
            action.to_string(),
            Some("203.0.113.9".to_string()),
            Some("TestAgent/1.0 (Windows NT 10.0)".to_string()),
            Some("nurse.one".to_string()),
        )
    };
    assert_eq!(
        auth_rows(&pool).await,
        [
            expected("login_succeeded"),
            expected("login_failed"),
            expected("logout")
        ]
    );

    // logout ซ้ำด้วย cookie เดิม (session ถูกลบแล้ว) และ logout โดยไม่มี cookie = ไม่มีใครให้ผูก ไม่บันทึก
    send(&app, Method::POST, "/api/auth/logout", Some(&cookie), None).await;
    send(&app, Method::POST, "/api/auth/logout", None, None).await;
    assert_eq!(auth_rows(&pool).await.len(), 3);
}

#[sqlx::test]
async fn login_without_proxy_headers_still_works(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "nurse.one", NURSE_PASSWORD, "nurse").await;
    login(&app, "nurse.one", NURSE_PASSWORD).await;
    assert_eq!(
        auth_rows(&pool).await,
        [(
            "login_succeeded".to_string(),
            None,
            None,
            Some("nurse.one".to_string())
        )]
    );
}

async fn insert_event(pool: &PgPool, action: &str, actor: Option<&str>, target: &str, at: &str) {
    sqlx::query(
        "insert into audit_log (actor_user_id, action, target_type, target_id, ip, metadata, created_at)
         values ((select id from users where username = $1), $2, 'user', $3, '192.0.2.5',
                 '{\"userAgent\": \"Agent/2\"}', $4::timestamptz)",
    )
    .bind(actor)
    .bind(action)
    .bind(target)
    .bind(at)
    .execute(pool)
    .await
    .unwrap();
}

async fn events(app: &axum::Router, cookie: &str, query: &str) -> (StatusCode, Value) {
    let response = send(
        app,
        Method::GET,
        &format!("/api/audit/auth-events?{query}"),
        Some(cookie),
        None,
    )
    .await;
    (response.status, response.body)
}

#[sqlx::test]
async fn auth_events_api_filters_by_bangkok_day_and_hides_typed_names(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (cookie, _) = login(&app, "admin.one", ADMIN_PASSWORD).await;

    // 15 ม.ค. 2026 ตามเวลาไทย = 2026-01-14T17:00Z ถึงก่อน 2026-01-15T17:00Z
    insert_event(
        &pool,
        "login_succeeded",
        Some("admin.one"),
        "admin.one",
        "2026-01-14T17:00:00Z",
    )
    .await;
    insert_event(
        &pool,
        "login_failed",
        None,
        "secret-typed-name",
        "2026-01-15T10:00:00Z",
    )
    .await;
    insert_event(
        &pool,
        "logout",
        Some("admin.one"),
        "",
        "2026-01-15T16:30:00Z",
    )
    .await; // 23:30 น.
    insert_event(
        &pool,
        "user_created",
        Some("admin.one"),
        "x",
        "2026-01-15T11:00:00Z",
    )
    .await; // ไม่ใช่เข้า/ออก
    insert_event(
        &pool,
        "logout",
        Some("admin.one"),
        "",
        "2026-01-15T17:00:00Z",
    )
    .await; // 00:00 น. วันถัดไป
    insert_event(
        &pool,
        "logout",
        Some("admin.one"),
        "",
        "2026-01-14T16:59:59Z",
    )
    .await; // 23:59 น. วันก่อน

    let (status, body) = events(&app, &cookie, "from=2026-01-15&to=2026-01-15").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["truncated"], false);
    let list = body["events"].as_array().unwrap();
    let summary: Vec<(&str, &str)> = list
        .iter()
        .map(|e| {
            (
                e["event"].as_str().unwrap(),
                e["occurredAt"].as_str().unwrap(),
            )
        })
        .collect();
    // ใหม่→เก่า เฉพาะ 3 เหตุการณ์ในวันนั้น
    assert_eq!(summary.len(), 3, "{summary:?}");
    assert_eq!(
        summary.iter().map(|(e, _)| *e).collect::<Vec<_>>(),
        ["logout", "login_failed", "login_succeeded"]
    );
    assert_eq!(list[0]["ip"], "192.0.2.5");
    assert_eq!(list[0]["userAgent"], "Agent/2");
    assert_eq!(list[0]["user"]["username"], "admin.one");
    assert!(list[0]["user"]["id"].is_string());

    // login ด้วยชื่อที่ไม่มีในระบบ: ไม่รู้ว่าใคร และชื่อที่พิมพ์มาไม่ออกจาก backend
    assert_eq!(list[1]["user"], Value::Null);
    assert!(!body.to_string().contains("secret-typed-name"));
}

#[sqlx::test]
async fn auth_events_api_requires_audit_view_and_a_sane_range(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    insert_user_with_password(&pool, "nurse.one", NURSE_PASSWORD, "nurse").await;
    let (admin, _) = login(&app, "admin.one", ADMIN_PASSWORD).await;
    let (nurse, _) = login(&app, "nurse.one", NURSE_PASSWORD).await;

    let (status, _) = events(&app, &nurse, "from=2026-01-01&to=2026-01-31").await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    for (query, expected) in [
        ("from=2026-01-31&to=2026-01-01", StatusCode::BAD_REQUEST),
        ("from=2025-01-01&to=2026-01-02", StatusCode::BAD_REQUEST), // 367 วัน
        ("from=2025-01-02&to=2026-01-02", StatusCode::OK),          // 366 วัน
        ("from=2026-01-01", StatusCode::BAD_REQUEST),
        ("from=bad&to=2026-01-01", StatusCode::BAD_REQUEST),
    ] {
        let (status, body) = events(&app, &admin, query).await;
        assert_eq!(status, expected, "{query}: {body}");
    }
}
