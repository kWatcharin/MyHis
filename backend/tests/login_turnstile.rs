//! Cloudflare Turnstile ตอน login (docs/auth-design.md ข้อ 6): ตรวจ token กับ siteverify **ก่อนค้นผู้ใช้**
//! ไม่ผ่าน = 400 `captcha failed`, ติดต่อไม่ได้ = 503 `captcha unavailable` (fail-closed) ทั้งสองกรณีไม่แตะตัวนับ
//! login ผิด และตอบเหมือนกันทุกชื่อ — siteverify เป็น server ปลอมในเครื่อง ไม่ยิง Cloudflare จริง

mod common;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::http::{Method, StatusCode};
use axum::routing::post;
use axum::{Form, Json, Router};
use common::{
    insert_user_with_password, send, send_with_headers, test_app, test_app_with_turnstile,
    TestResponse, TEST_TURNSTILE_HOSTNAME, TEST_TURNSTILE_SECRET, TEST_TURNSTILE_SITE_KEY,
};
use serde_json::{json, Value};
use sqlx::PgPool;

const PASSWORD: &str = "nurse-test-password-1";
const WRONG: &str = "not-the-password-123";

/// form ที่ siteverify ปลอมได้รับ (secret/response/remoteip) ทุกครั้ง
type Received = Arc<Mutex<Vec<HashMap<String, String>>>>;

/// siteverify ปลอม — ตอบตาม token: `pass` ผ่าน, `wrong-action`/`wrong-host` ผ่านแต่มาจาก widget/โดเมนอื่น,
/// `test-key` แบบ test key ของ Cloudflare (ไม่มี action), `server-error` ตอบ 500, อื่น ๆ ไม่ผ่าน
async fn fake_siteverify() -> (String, Received) {
    let received: Received = Arc::default();
    let log = received.clone();
    let router = Router::new().route(
        "/siteverify",
        post(move |Form(form): Form<HashMap<String, String>>| {
            let log = log.clone();
            async move {
                let token = form.get("response").cloned().unwrap_or_default();
                log.lock().unwrap().push(form);
                let ok = |action: &str, hostname: &str| {
                    json!({ "success": true, "error-codes": [], "action": action, "hostname": hostname })
                };
                let body = match token.as_str() {
                    "pass" => ok("login", TEST_TURNSTILE_HOSTNAME),
                    "wrong-action" => ok("register", TEST_TURNSTILE_HOSTNAME),
                    "wrong-host" => ok("login", "evil.example"),
                    "test-key" => json!({
                        "success": true, "error-codes": [], "hostname": "example.com",
                        "metadata": { "result_with_testing_key": true }
                    }),
                    "server-error" => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({}))),
                    _ => json!({ "success": false, "error-codes": ["invalid-input-response"] }),
                };
                (StatusCode::OK, Json(body))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/siteverify", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (url, received)
}

async fn attempt(
    app: &Router,
    username: &str,
    password: &str,
    token: Option<&str>,
) -> TestResponse {
    let mut body = json!({ "username": username, "password": password });
    if let Some(token) = token {
        body["turnstileToken"] = json!(token);
    }
    send(app, Method::POST, "/api/auth/login", None, Some(body)).await
}

async fn failed_attempts(pool: &PgPool, username: &str) -> i32 {
    sqlx::query_scalar("select failed_attempts from users where username = $1")
        .bind(username)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn unknown_name_rows(pool: &PgPool) -> i64 {
    sqlx::query_scalar("select count(*) from login_throttle")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn session_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("select count(*) from sessions")
        .fetch_one(pool)
        .await
        .unwrap()
}

fn captcha_failed() -> (StatusCode, Value) {
    (
        StatusCode::BAD_REQUEST,
        json!({ "error": "captcha failed" }),
    )
}

#[sqlx::test]
async fn bad_or_missing_token_is_rejected_before_the_username_is_looked_at(pool: PgPool) {
    let (url, _) = fake_siteverify().await;
    let app = test_app_with_turnstile(pool.clone(), &url).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;

    for token in [
        None,
        Some(""),
        Some("bogus"),
        Some("wrong-action"),
        Some("wrong-host"),
    ] {
        // รหัสถูกก็ไม่ผ่าน และชื่อที่ไม่มีในระบบได้คำตอบเดียวกันเป๊ะ
        for (username, password) in [("nurse.one", PASSWORD), ("ghost.user", WRONG)] {
            let response = attempt(&app, username, password, token).await;
            assert_eq!(
                (response.status, response.body),
                captcha_failed(),
                "{username} {token:?}"
            );
            assert!(response.set_cookie.is_none());
        }
    }

    // ไม่นับเป็นครั้งที่ผิด ทั้งชื่อที่มีและไม่มี ไม่มี session และไม่บันทึกประวัติ login
    assert_eq!(failed_attempts(&pool, "nurse.one").await, 0);
    assert_eq!(unknown_name_rows(&pool).await, 0);
    assert_eq!(session_count(&pool).await, 0);
    let login_events: i64 = sqlx::query_scalar(
        "select count(*) from audit_log where action in ('login_failed', 'login_succeeded')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(login_events, 0);
}

#[sqlx::test]
async fn passing_token_continues_with_the_normal_login_rules(pool: PgPool) {
    let (url, received) = fake_siteverify().await;
    let app = test_app_with_turnstile(pool.clone(), &url).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;

    // รหัสผิด: กติกาตัวนับเดิมทุกอย่าง
    let wrong = attempt(&app, "nurse.one", WRONG, Some("pass")).await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        wrong.body,
        json!({ "error": "invalid credentials", "remainingAttempts": 4 })
    );

    let right = send_with_headers(
        &app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": "nurse.one", "password": PASSWORD, "turnstileToken": "pass" })),
        &[("x-forwarded-for", "1.1.1.1, 203.0.113.7")],
    )
    .await;
    assert_eq!(right.status, StatusCode::OK);
    assert!(right.set_cookie.is_some());
    assert_eq!(failed_attempts(&pool, "nurse.one").await, 0);

    // ส่ง secret + token + IP ของผู้ใช้ (ค่าขวาสุดของ X-Forwarded-For) ให้ Cloudflare
    let forms = received.lock().unwrap().clone();
    assert_eq!(forms.len(), 2);
    assert_eq!(forms[1]["secret"], TEST_TURNSTILE_SECRET);
    assert_eq!(forms[1]["response"], "pass");
    assert_eq!(forms[1]["remoteip"], "203.0.113.7");
    assert!(!forms[0].contains_key("remoteip"));
}

#[sqlx::test]
async fn cloudflare_test_key_result_is_accepted_without_action(pool: PgPool) {
    let (url, _) = fake_siteverify().await;
    let app = test_app_with_turnstile(pool.clone(), &url).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;

    let response = attempt(&app, "nurse.one", PASSWORD, Some("test-key")).await;
    assert_eq!(response.status, StatusCode::OK);
}

#[sqlx::test]
async fn unreachable_siteverify_refuses_login_without_counting(pool: PgPool) {
    // port ที่ปิดแล้ว (bind แล้วปล่อยทิ้ง) → ต่อไม่ได้ทันที
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/siteverify", closed.local_addr().unwrap());
    drop(closed);
    let (fake_url, _) = fake_siteverify().await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;

    let unavailable = (
        StatusCode::SERVICE_UNAVAILABLE,
        json!({ "error": "captcha unavailable" }),
    );
    let down = test_app_with_turnstile(pool.clone(), &url).await;
    let response = attempt(&down, "nurse.one", PASSWORD, Some("pass")).await;
    assert_eq!((response.status, response.body), unavailable);

    // siteverify ตอบ 500 ก็ปฏิเสธเหมือนกัน
    let broken = test_app_with_turnstile(pool.clone(), &fake_url).await;
    let response = attempt(&broken, "nurse.one", PASSWORD, Some("server-error")).await;
    assert_eq!((response.status, response.body), unavailable);

    assert_eq!(failed_attempts(&pool, "nurse.one").await, 0);
    assert_eq!(session_count(&pool).await, 0);
}

#[sqlx::test]
async fn login_config_tells_the_page_whether_to_show_the_widget(pool: PgPool) {
    let (url, _) = fake_siteverify().await;
    let enabled = test_app_with_turnstile(pool.clone(), &url).await;
    let response = send(&enabled, Method::GET, "/api/auth/login-config", None, None).await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(
        response.body,
        json!({ "turnstileSiteKey": TEST_TURNSTILE_SITE_KEY })
    );

    // ปิดฟีเจอร์: ไม่มี widget และ login แบบเดิม (ไม่ส่ง token) ได้ตามปกติ
    let disabled = test_app(pool.clone()).await;
    let response = send(&disabled, Method::GET, "/api/auth/login-config", None, None).await;
    assert_eq!(response.body, json!({ "turnstileSiteKey": null }));
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;
    assert_eq!(
        attempt(&disabled, "nurse.one", PASSWORD, None).await.status,
        StatusCode::OK
    );
}

#[sqlx::test]
async fn oversized_token_is_a_validation_error(pool: PgPool) {
    let (url, received) = fake_siteverify().await;
    let app = test_app_with_turnstile(pool.clone(), &url).await;
    let response = attempt(&app, "nurse.one", PASSWORD, Some(&"x".repeat(2049))).await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert!(received.lock().unwrap().is_empty());
}
