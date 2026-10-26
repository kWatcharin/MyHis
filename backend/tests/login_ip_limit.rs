//! เพดาน login ที่ไม่สำเร็จต่อ IP (docs/auth-design.md ข้อ 9.2): ชนเพดาน → 429 `too many attempts` ก่อนเรียก Turnstile/argon2,
//! นับตาม IP อย่างเดียว (ชื่อมี/ไม่มีได้ผลเหมือนกัน), login สำเร็จและ 503 คืนโควตา, คำขอพร้อมกันผ่านได้ไม่เกินเพดาน
//! IP ของผู้ใช้อ่านจาก X-Forwarded-For ค่าขวาสุด (เหมือนที่ Caddy เติมให้) — ไม่มี header = ไม่จำกัด

mod common;

use axum::http::{Method, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use chrono::Duration;
use common::{
    insert_user_with_password, send_with_headers, test_app_with_login_ip_budget,
    test_app_with_turnstile_and_login_ip_budget, TestResponse,
};
use my_his_backend::auth::IpBudget;
use serde_json::{json, Value};
use sqlx::PgPool;

const PASSWORD: &str = "nurse-test-password-1";
const WRONG: &str = "not-the-password-123";
const IP_A: &str = "203.0.113.9";
const IP_B: &str = "203.0.113.10";

fn budget(max: u32) -> IpBudget {
    IpBudget::new(max, Duration::minutes(10), 100)
}

async fn attempt_with(
    app: &Router,
    ip: Option<&str>,
    username: &str,
    password: &str,
    token: Option<&str>,
) -> TestResponse {
    let headers: Vec<(&str, &str)> = ip.map(|ip| ("x-forwarded-for", ip)).into_iter().collect();
    let mut body = json!({ "username": username, "password": password });
    if let Some(token) = token {
        body["turnstileToken"] = json!(token);
    }
    send_with_headers(
        app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(body),
        &headers,
    )
    .await
}

async fn attempt(app: &Router, ip: Option<&str>, username: &str, password: &str) -> TestResponse {
    attempt_with(app, ip, username, password, None).await
}

fn blocked() -> (StatusCode, Value) {
    (
        StatusCode::TOO_MANY_REQUESTS,
        json!({ "error": "too many attempts" }),
    )
}

async fn session_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("select count(*) from sessions")
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn one_address_is_blocked_after_the_limit_whatever_the_names(pool: PgPool) {
    let app = test_app_with_login_ip_budget(pool.clone(), budget(4)).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;

    // ชื่อสุ่มคนละชื่อ: ตัวนับต่อชื่อไม่เคยถึง 5 แต่ IP เดียวกันนับรวม
    for i in 0..4 {
        let response = attempt(&app, Some(IP_A), &format!("ghost.{i}"), WRONG).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
        assert_eq!(response.body["remainingAttempts"], 4);
    }

    // ครบเพดานแล้ว: แม้เป็นบัญชีจริงและรหัสถูกก็ถูกปฏิเสธก่อนถึงขั้น verify ไม่ออก session
    let right = attempt(&app, Some(IP_A), "nurse.one", PASSWORD).await;
    assert_eq!((right.status, right.body.clone()), blocked());
    // นับถอยหลังจาก 600 วินาที: ขอบล่างหลวมไว้ เพราะ argon2 แบบ debug ช้าได้หลายวินาทีต่อครั้งเมื่อเครื่องโหลดหนัก
    let seconds = right.retry_after.expect("Retry-After");
    assert!((300..=600).contains(&seconds), "Retry-After {seconds}");
    assert!(right.set_cookie.is_none());
    assert_eq!(session_count(&pool).await, 0);

    // ชื่อที่ไม่มีในระบบ ได้ผลเหมือนกันทุกอย่าง
    let ghost = attempt(&app, Some(IP_A), "ghost.new", PASSWORD).await;
    assert_eq!((ghost.status, ghost.body), blocked());

    // ที่อยู่อื่นไม่กระทบ
    let other = attempt(&app, Some(IP_B), "nurse.one", PASSWORD).await;
    assert_eq!(other.status, StatusCode::OK);

    // ไม่มี X-Forwarded-For (ไม่ผ่าน proxy) = ไม่จำกัด
    for i in 0..6 {
        let response = attempt(&app, None, &format!("direct.{i}"), WRONG).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }
}

#[sqlx::test]
async fn known_and_unknown_names_are_counted_and_blocked_the_same(pool: PgPool) {
    let app = test_app_with_login_ip_budget(pool.clone(), budget(3)).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;

    assert_eq!(
        attempt(&app, Some(IP_A), "nurse.one", WRONG).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        attempt(&app, Some(IP_A), "ghost.one", WRONG).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        attempt(&app, Some(IP_A), "nurse.one", WRONG).await.status,
        StatusCode::UNAUTHORIZED
    );

    let known = attempt(&app, Some(IP_A), "nurse.one", WRONG).await;
    let unknown = attempt(&app, Some(IP_A), "ghost.two", WRONG).await;
    assert_eq!((known.status, known.body.clone()), blocked());
    assert_eq!((unknown.status, unknown.body.clone()), blocked());
    assert_eq!(known.retry_after.is_some(), unknown.retry_after.is_some());

    // คำขอที่ถูกปัดตกที่ด่าน IP ไม่แตะตัวนับต่อชื่อ (ไม่ถึงขั้นจอง) และไม่เขียน audit เพิ่ม
    let failed: i32 =
        sqlx::query_scalar("select failed_attempts from users where username = 'nurse.one'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(failed, 2);
    let counters: i64 = sqlx::query_scalar("select count(*) from login_throttle")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(counters, 1);
    let audit_rows: i64 =
        sqlx::query_scalar("select count(*) from audit_log where action = 'login_failed'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audit_rows, 3);
}

#[sqlx::test]
async fn successful_logins_do_not_use_the_quota(pool: PgPool) {
    let app = test_app_with_login_ip_budget(pool.clone(), budget(2)).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;

    for _ in 0..5 {
        let response = attempt(&app, Some(IP_A), "nurse.one", PASSWORD).await;
        assert_eq!(response.status, StatusCode::OK);
    }
    for i in 0..2 {
        let response = attempt(&app, Some(IP_A), &format!("ghost.{i}"), WRONG).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    }
    let third = attempt(&app, Some(IP_A), "ghost.2", WRONG).await;
    assert_eq!((third.status, third.body), blocked());
}

/// คำขอพร้อมกันจาก IP เดียว: จองก่อนทำงานจึงผ่านด่านได้ไม่เกินเพดานพอดี ที่เหลือถูกปฏิเสธโดยไม่ verify
#[sqlx::test]
async fn concurrent_attempts_pass_the_gate_at_most_the_limit(pool: PgPool) {
    let app = test_app_with_login_ip_budget(pool.clone(), budget(5)).await;

    let jobs: Vec<_> = (0..12)
        .map(|i| {
            let app = app.clone();
            tokio::spawn(async move {
                attempt(&app, Some(IP_A), &format!("ghost.{i}"), WRONG)
                    .await
                    .status
            })
        })
        .collect();
    let mut statuses = Vec::new();
    for job in jobs {
        statuses.push(job.await.unwrap());
    }
    let rejected = statuses
        .iter()
        .filter(|s| **s == StatusCode::UNAUTHORIZED)
        .count();
    let blocked_count = statuses
        .iter()
        .filter(|s| **s == StatusCode::TOO_MANY_REQUESTS)
        .count();
    assert_eq!((rejected, blocked_count), (5, 7), "{statuses:?}");
}

/// ตารางที่อยู่เต็ม → ที่อยู่ใหม่แบ่งถังรวมถังเดียว (ไม่ใช่ไม่จำกัดเหมือนเดิม — ผู้ที่ดันตารางให้เต็มไม่ได้ใบอนุญาตไม่จำกัด) และที่อยู่
/// ที่ติดตามอยู่แล้วนับต่อตามปกติ; IPv6 นับทั้ง /56
#[sqlx::test]
async fn a_full_address_table_shares_one_overflow_budget(pool: PgPool) {
    // ตาราง 1 key, ถังรวม 3 ครั้ง
    let app = test_app_with_login_ip_budget(
        pool.clone(),
        IpBudget::with_overflow(10, Duration::minutes(10), 1, 3),
    )
    .await;
    let status = |response: TestResponse| response.status;

    assert_eq!(
        status(attempt(&app, Some("203.0.113.1"), "ghost.0", WRONG).await),
        StatusCode::UNAUTHORIZED
    );
    for i in 2..5 {
        let ip = format!("203.0.113.{i}");
        assert_eq!(
            status(attempt(&app, Some(&ip), "ghost.new", WRONG).await),
            StatusCode::UNAUTHORIZED,
            "{ip}"
        );
    }
    let over = attempt(&app, Some("203.0.113.9"), "ghost.new", WRONG).await;
    assert_eq!((over.status, over.body), blocked());
    // ที่อยู่ที่ติดตามอยู่แล้วยังนับต่อตามปกติ (ไม่ใช่ถังรวม)
    assert_eq!(
        status(attempt(&app, Some("203.0.113.1"), "ghost.0", WRONG).await),
        StatusCode::UNAUTHORIZED
    );
}

/// IPv6 ที่อยู่ใน /56 เดียวกันแบ่งโควตาเดียวกัน (ผู้ถือ prefix ใหญ่เปลี่ยน /64 เพื่อเลี่ยงเพดานไม่ได้)
#[sqlx::test]
async fn ipv6_addresses_in_one_slash_56_share_a_quota(pool: PgPool) {
    let app = test_app_with_login_ip_budget(pool.clone(), budget(2)).await;
    for subnet in ["2001:db8:4800:100::1", "2001:db8:4800:1ff::7"] {
        let response = attempt(&app, Some(subnet), "ghost.one", WRONG).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED, "{subnet}");
    }
    let third = attempt(&app, Some("2001:db8:4800:142::3"), "ghost.one", WRONG).await;
    assert_eq!((third.status, third.body), blocked());
    // /56 ถัดไปเป็นอีกคน
    let other = attempt(&app, Some("2001:db8:4800:200::1"), "ghost.one", WRONG).await;
    assert_eq!(other.status, StatusCode::UNAUTHORIZED);
}

/// siteverify ปลอมที่ไม่ผ่านทุก token
async fn rejecting_siteverify() -> String {
    let router = Router::new().route(
        "/siteverify",
        post(|| async {
            Json(json!({ "success": false, "error-codes": ["invalid-input-response"] }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/siteverify", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    url
}

/// token Turnstile ที่ไม่ผ่านนับเป็นความพยายามที่ไม่สำเร็จ (บอทที่ไม่มี token ต้องถูกตัดด้วย); Cloudflare ไม่ตอบ (503)
/// เป็นความผิดของฝั่งเรา ไม่นับ
#[sqlx::test]
async fn rejected_captchas_count_but_an_unreachable_siteverify_does_not(pool: PgPool) {
    let rejecting = rejecting_siteverify().await;
    let app =
        test_app_with_turnstile_and_login_ip_budget(pool.clone(), &rejecting, budget(2)).await;
    for _ in 0..2 {
        let response = attempt_with(&app, Some(IP_A), "ghost.one", WRONG, Some("bad")).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST);
        assert_eq!(response.body["error"], "captcha failed");
    }
    let third = attempt_with(&app, Some(IP_A), "ghost.one", WRONG, Some("bad")).await;
    assert_eq!((third.status, third.body), blocked());

    // port ที่ปิดแล้ว → ต่อ Cloudflare ไม่ได้: 503 ทุกครั้ง ไม่เคยกลายเป็น 429 แม้เกินเพดาน
    let closed = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let closed_url = format!("http://{}/siteverify", closed.local_addr().unwrap());
    drop(closed);
    let down =
        test_app_with_turnstile_and_login_ip_budget(pool.clone(), &closed_url, budget(1)).await;
    for _ in 0..4 {
        let response = attempt_with(&down, Some(IP_A), "ghost.one", WRONG, Some("pass")).await;
        assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.body["error"], "captcha unavailable");
    }
}
