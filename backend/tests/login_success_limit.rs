//! เพดาน login ที่ **สำเร็จ** ต่อผู้ใช้ (docs/auth-design.md ข้อ 9.3) + ตัวป้องกันข้างเคียงของการ login (security audit รอบ 2):
//! - ผู้ถือรหัสผ่านที่ถูกต้องวนล็อกอินรัวไม่ได้ — ครบเพดาน → 429 `too many attempts` ไม่สร้าง session ไม่เขียน audit `login_succeeded`
//! - ผู้ใช้อื่นไม่กระทบ; คำขอที่ถูกปัดตกไม่สะสมตัวนับผิดของบัญชี (ไม่ล็อกบัญชีเอง)
//! - username ที่ไม่ตรงรูปแบบ (NUL, ยาวเกิน) ไม่ทำให้ Postgres error → 401/400 ไม่ใช่ 500
//! - sweep session หมดอายุใช้ index (ไม่สแกนทั้งตาราง)

mod common;

use axum::http::{Method, StatusCode};
use chrono::Duration;
use common::{
    insert_user_with_password, send_with_headers, test_app_with_login_success_budget, TestResponse,
};
use my_his_backend::auth::UserBudget;
use serde_json::json;
use sqlx::PgPool;

const PASSWORD: &str = "nurse-test-password-1";
const WRONG: &str = "not-the-password-123";

fn budget(max: u32) -> UserBudget {
    UserBudget::new(max, Duration::minutes(10), 100)
}

async fn attempt(app: &axum::Router, username: &str, password: &str) -> TestResponse {
    send_with_headers(
        app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": username, "password": password })),
        &[],
    )
    .await
}

async fn count(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[sqlx::test]
async fn successful_logins_stop_at_the_per_user_limit(pool: PgPool) {
    let app = test_app_with_login_success_budget(pool.clone(), budget(3)).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;
    insert_user_with_password(&pool, "nurse.two", PASSWORD, "nurse").await;

    for _ in 0..3 {
        let response = attempt(&app, "nurse.one", PASSWORD).await;
        assert_eq!(response.status, StatusCode::OK);
        assert!(response.set_cookie.is_some());
    }
    assert_eq!(count(&pool, "select count(*) from sessions").await, 3);

    // ครบเพดาน: รหัสถูกก็ได้ 429 + Retry-After ไม่มี cookie ไม่มี session/audit เพิ่ม
    let blocked = attempt(&app, "nurse.one", PASSWORD).await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(blocked.body["error"], "too many attempts");
    // นับถอยหลังจาก 600 วินาที: ขอบล่างหลวมไว้ เพราะ argon2 แบบ debug ช้าได้หลายวินาทีต่อครั้งเมื่อเครื่องโหลดหนัก
    let seconds = blocked.retry_after.expect("Retry-After");
    assert!((300..=600).contains(&seconds), "Retry-After {seconds}");
    assert!(blocked.set_cookie.is_none());
    assert_eq!(count(&pool, "select count(*) from sessions").await, 3);
    assert_eq!(
        count(
            &pool,
            "select count(*) from audit_log where action = 'login_succeeded'"
        )
        .await,
        3
    );

    // ผู้ใช้อื่นไม่กระทบ
    assert_eq!(
        attempt(&app, "nurse.two", PASSWORD).await.status,
        StatusCode::OK
    );
}

/// คำขอที่ถูกปัดตกด้วยเพดานนี้ผ่านการจองตัวนับผิดของบัญชีมาแล้ว (จองก่อน verify) ต้องล้างทิ้ง — ไม่งั้นยิงซ้ำ 5 ครั้งก็ล็อกบัญชีเอง
#[sqlx::test]
async fn blocked_logins_do_not_accumulate_towards_the_account_lock(pool: PgPool) {
    let app = test_app_with_login_success_budget(pool.clone(), budget(1)).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;

    assert_eq!(
        attempt(&app, "nurse.one", PASSWORD).await.status,
        StatusCode::OK
    );
    for _ in 0..8 {
        let blocked = attempt(&app, "nurse.one", PASSWORD).await;
        assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(blocked.body["error"], "too many attempts");
    }
    let (failed, locked): (i32, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "select failed_attempts, locked_until from users where username = 'nurse.one'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failed, 0);
    assert!(locked.is_none());
}

/// รหัสผิดไม่กินโควตาความสำเร็จ (เพดานนี้นับเฉพาะรหัสที่ถูก — รหัสผิดมีเพดานต่อชื่อ/IP ของมันเอง)
#[sqlx::test]
async fn wrong_passwords_do_not_use_the_success_quota(pool: PgPool) {
    let app = test_app_with_login_success_budget(pool.clone(), budget(2)).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;

    for _ in 0..2 {
        assert_eq!(
            attempt(&app, "nurse.one", WRONG).await.status,
            StatusCode::UNAUTHORIZED
        );
    }
    for _ in 0..2 {
        assert_eq!(
            attempt(&app, "nurse.one", PASSWORD).await.status,
            StatusCode::OK
        );
    }
}

/// username ที่ไม่ตรงรูปแบบที่เก็บได้ (NUL, ยาว, อักขระแปลก) ต้องตอบเหมือนชื่อที่ไม่มีในระบบ (401 invalid credentials) ไม่ใช่ 500 —
/// Postgres ปฏิเสธ NUL ใน text parameter ด้วย error ซึ่งเดิมกลายเป็น 500 ที่คืนโควตา IP (ยิงได้ไม่จำกัด)
#[sqlx::test]
async fn malformed_usernames_are_ordinary_failures_not_server_errors(pool: PgPool) {
    let app = test_app_with_login_success_budget(pool.clone(), budget(30)).await;
    insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;

    let nul = attempt(&app, "nurse\u{0}.one", PASSWORD).await;
    let ghost = attempt(&app, "ghost.one", PASSWORD).await;
    assert_eq!(nul.status, StatusCode::UNAUTHORIZED);
    assert_eq!(nul.body["error"], "invalid credentials");
    // รูปแบบ response เหมือนชื่อที่ไม่มีในระบบทุกประการ (ไม่เป็น oracle)
    assert_eq!(nul.status, ghost.status);
    assert_eq!(nul.body["error"], ghost.body["error"]);
    assert_eq!(
        nul.body["remainingAttempts"],
        ghost.body["remainingAttempts"]
    );

    for bad in [
        "UPPER CASE".to_string(),
        "ชื่อไทย".to_string(),
        "a".repeat(33),
        "a/b".to_string(),
    ] {
        let response = attempt(&app, &bad, PASSWORD).await;
        assert_eq!(response.status, StatusCode::UNAUTHORIZED, "{bad:?}");
    }
    // ไม่มีใครล็อกอินสำเร็จ ไม่มี session
    assert_eq!(count(&pool, "select count(*) from sessions").await, 0);
}

#[sqlx::test]
async fn oversized_login_fields_are_rejected_before_any_work(pool: PgPool) {
    let app = test_app_with_login_success_budget(pool.clone(), budget(30)).await;
    for (username, password) in [
        ("a".repeat(65), PASSWORD.to_string()),
        ("nurse.one".to_string(), "p".repeat(1025)),
    ] {
        let response = attempt(&app, &username, &password).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST);
    }
    // ไม่ถึงขั้นสร้างตัวนับ/audit
    assert_eq!(count(&pool, "select count(*) from login_throttle").await, 0);
    assert_eq!(count(&pool, "select count(*) from audit_log").await, 0);
}

/// sweep session หมดอายุ (`delete … where idle_expires_at < $1 or absolute_expires_at < $1`) ต้องใช้ index ของสองคอลัมน์ ไม่สแกนทั้งตาราง
#[sqlx::test]
async fn the_expired_session_sweep_uses_the_expiry_indexes(pool: PgPool) {
    let (user_id, _) = insert_user_with_password(&pool, "nurse.one", PASSWORD, "nurse").await;
    sqlx::query(
        "insert into sessions (token_hash, user_id, idle_expires_at, absolute_expires_at)
         select 'h' || g, $1, now() + interval '8 hours', now() + interval '30 days'
         from generate_series(1, 20000) g",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("analyze sessions")
        .execute(&pool)
        .await
        .unwrap();

    let plan: Vec<(String,)> = sqlx::query_as(
        "explain delete from sessions where idle_expires_at < now() or absolute_expires_at < now()",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let plan = plan
        .into_iter()
        .map(|(line,)| line)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !plan.contains("Seq Scan on sessions"),
        "sweep should use the expiry indexes:\n{plan}"
    );
    assert!(plan.contains("sessions_idle_expires_at_idx"), "{plan}");
    assert!(plan.contains("sessions_absolute_expires_at_idx"), "{plan}");
}
