//! ตัวนับ login ผิด (docs/auth-design.md ข้อ 9): ผิดติดกัน 5 ครั้ง → ล็อก 60 วินาที, 401 บอก `remainingAttempts`
//! ให้หน้า login แสดง "เหลือ X/5", ล็อกหมดอายุแล้วนับใหม่ — **ชื่อที่ไม่มีในระบบต้องได้ผลเหมือนกันทุกอย่าง**
//! (กันเดาว่าชื่อไหนมีจริง) และตารางของชื่อที่ไม่มีเก็บแค่ hash ไม่เก็บชื่อดิบ

mod common;

use axum::http::{Method, StatusCode};
use axum::Router;
use common::{insert_user_with_password, send, test_app, TestResponse};
use serde_json::{json, Value};
use sqlx::PgPool;

const PASSWORD: &str = "nurse-test-password-1";
const WRONG: &str = "not-the-password-123";

async fn attempt(app: &Router, username: &str, password: &str) -> TestResponse {
    send(
        app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": username, "password": password })),
    )
    .await
}

/// (status, body, มี Retry-After ไหม) ของ login ผิด 6 ครั้งติดกัน
async fn six_wrong_attempts(app: &Router, username: &str) -> Vec<(StatusCode, Value, bool)> {
    let mut out = Vec::new();
    for _ in 0..6 {
        let response = attempt(app, username, WRONG).await;
        if let Some(seconds) = response.retry_after {
            assert!((55..=60).contains(&seconds), "Retry-After {seconds}");
        }
        out.push((
            response.status,
            response.body,
            response.retry_after.is_some(),
        ));
    }
    out
}

fn rejected(remaining: i32) -> (StatusCode, Value, bool) {
    (
        StatusCode::UNAUTHORIZED,
        json!({ "error": "invalid credentials", "remainingAttempts": remaining }),
        false,
    )
}

fn locked() -> (StatusCode, Value, bool) {
    (
        StatusCode::TOO_MANY_REQUESTS,
        json!({ "error": "account temporarily locked" }),
        true,
    )
}

#[sqlx::test]
async fn five_wrong_attempts_lock_for_sixty_seconds_and_unknown_names_match(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;

    let expected = vec![
        rejected(4),
        rejected(3),
        rejected(2),
        rejected(1),
        locked(), // ครั้งที่ 5 ล็อกทันที
        locked(), // ระหว่างล็อก
    ];
    assert_eq!(six_wrong_attempts(&app, "nurse.one").await, expected);
    // ชื่อที่ไม่มีในระบบ: status + body + header ลำดับเดียวกันเป๊ะ
    assert_eq!(six_wrong_attempts(&app, "ghost.user").await, expected);

    // ระหว่างล็อก รหัสถูกก็ยังเข้าไม่ได้
    let right = attempt(&app, "nurse.one", PASSWORD).await;
    assert_eq!(right.status, StatusCode::TOO_MANY_REQUESTS);

    // ชื่อที่ไม่มีเก็บแค่ SHA-256 — ไม่มีชื่อดิบในตาราง (ชื่อที่พิมพ์มาอาจเป็นรหัสผ่าน)
    let rows: Vec<(String, i32)> =
        sqlx::query_as("select username_hash, failed_attempts from login_throttle")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0.len(), 64);
    assert!(!rows[0].0.contains("ghost"));
    assert_eq!(rows[0].1, 5);

    // บันทึกประวัติ login_failed ครบทุกครั้งที่ไม่ได้ถูกปัดตกเพราะล็อก (4 + ครั้งที่ล็อก = 5 ต่อชื่อ)
    let failures: i64 =
        sqlx::query_scalar("select count(*) from audit_log where action = 'login_failed'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(failures, 10);
}

#[sqlx::test]
async fn expired_lock_starts_a_fresh_count_and_success_resets(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;
    for username in ["nurse.one", "ghost.user"] {
        for _ in 0..5 {
            attempt(&app, username, WRONG).await;
        }
    }
    // เหมือนเวลาผ่านไปเกิน 60 วินาที
    sqlx::query(
        "update users set locked_until = now() - interval '1 second' where username = 'nurse.one'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("update login_throttle set locked_until = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();

    for username in ["nurse.one", "ghost.user"] {
        let again = attempt(&app, username, WRONG).await;
        assert_eq!(
            (again.status, again.body),
            (
                StatusCode::UNAUTHORIZED,
                json!({ "error": "invalid credentials", "remainingAttempts": 4 })
            ),
            "{username}"
        );
    }

    let ok = attempt(&app, "nurse.one", PASSWORD).await;
    assert_eq!(ok.status, StatusCode::OK);
    let (failed, locked_until): (i32, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "select failed_attempts, locked_until from users where username = 'nurse.one'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((failed, locked_until), (0, None));
    // นับใหม่เต็ม 5 ครั้งหลัง login สำเร็จ
    let next = attempt(&app, "nurse.one", WRONG).await;
    assert_eq!(next.body["remainingAttempts"], 4);
}

#[sqlx::test]
async fn deactivated_account_counts_like_a_wrong_password(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;
    sqlx::query("update users set is_active = false where username = 'nurse.one'")
        .execute(&pool)
        .await
        .unwrap();

    let first = attempt(&app, "nurse.one", PASSWORD).await;
    assert_eq!(
        (first.status, first.body),
        (
            StatusCode::UNAUTHORIZED,
            json!({ "error": "invalid credentials", "remainingAttempts": 4 })
        )
    );
}

#[sqlx::test]
async fn wrong_current_password_on_change_has_no_attempt_count(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;
    let (cookie, _) = common::login(&app, "nurse.one", PASSWORD).await;

    let wrong = send(
        &app,
        Method::POST,
        "/api/auth/password",
        Some(&cookie),
        Some(json!({ "currentPassword": WRONG, "newPassword": "a-brand-new-password" })),
    )
    .await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong.body, json!({ "error": "invalid credentials" }));
}

/// login ด้วยชื่อที่ไม่มีในระบบ: ประวัติ (audit_log.target_id) เก็บแค่ SHA-256 ของชื่อ ไม่เก็บชื่อดิบ
/// (อาจเป็นรหัสผ่านที่พิมพ์ผิดช่อง) — บัญชีที่มีจริงยังเก็บ username ตามเดิม
#[sqlx::test]
async fn failed_login_audit_never_stores_an_unknown_typed_name(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;
    attempt(&app, "  My-Secret-Pass-Typo ", WRONG).await;
    attempt(&app, "nurse.one", WRONG).await;

    let targets: Vec<Option<String>> = sqlx::query_scalar(
        "select target_id from audit_log where action = 'login_failed' order by id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(targets.len(), 2);
    let unknown = targets[0].clone().unwrap();
    assert_eq!(unknown.len(), 64);
    assert!(unknown.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(!unknown.to_lowercase().contains("secret"));
    assert_eq!(targets[1].as_deref(), Some("nurse.one"));
}

/// login สำเร็จแล้วลบ session ที่หมดอายุทิ้ง (ของทุกคน) — session ที่ยังใช้ได้อยู่ไม่ถูกแตะ
#[sqlx::test]
async fn successful_login_cleans_up_expired_sessions(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    let (user_id, _) = insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;
    for (hash, idle, absolute) in [
        (
            "expired-idle",
            "now() - interval '1 minute'",
            "now() + interval '1 day'",
        ),
        (
            "expired-absolute",
            "now() + interval '1 hour'",
            "now() - interval '1 minute'",
        ),
        (
            "still-valid",
            "now() + interval '1 hour'",
            "now() + interval '1 day'",
        ),
    ] {
        sqlx::query(&format!(
            "insert into sessions (token_hash, user_id, idle_expires_at, absolute_expires_at)
             values ($1, $2, {idle}, {absolute})"
        ))
        .bind(hash)
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    common::login(&app, "nurse.one", PASSWORD).await;

    let mut left: Vec<String> = sqlx::query_scalar("select token_hash from sessions")
        .fetch_all(&pool)
        .await
        .unwrap();
    left.sort();
    assert_eq!(left.len(), 2, "{left:?}"); // still-valid + session ใหม่ของ login นี้
    assert!(left.contains(&"still-valid".to_string()));
    assert!(!left.iter().any(|h| h.starts_with("expired")));
}
