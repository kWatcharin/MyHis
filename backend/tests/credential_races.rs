//! เหตุการณ์พร้อมกันระหว่างการเพิกถอน credential (รีเซ็ตรหัสโดยแอดมิน) กับ login / เปลี่ยนรหัสผ่านที่กำลังรอคิว argon2
//! (docs/auth-design.md ข้อ 6-8) และเพดานการตรวจรหัสผ่านปัจจุบันต่อ session
//!
//! จัดลำดับด้วย lock: connection ของเทสถือ `select ... for update` ที่แถวผู้ใช้เป้าหมาย (เปลี่ยนรหัสผ่าน) หรือ `lock table sessions`
//! (login) รีเซ็ตเข้าคิวรอ แล้วค่อยเริ่มคำขออีกตัว เมื่อปล่อย lock รีเซ็ต commit ก่อนเสมอ —
//! คำขอที่ค้างต้องไม่ทิ้ง credential/session ที่ใช้ได้หลังรีเซ็ต
mod common;

use std::time::{Duration, Instant};

use axum::http::{Method, StatusCode};
use serde_json::json;
use sqlx::{Connection, PgConnection, PgPool};

const ADMIN_PASSWORD: &str = "admin-dummy-password-1";
const OLD_PASSWORD: &str = "victim-dummy-password-1";
const NEW_PASSWORD: &str = "holder-chosen-password-2";

async fn raw_conn(pool: &PgPool) -> PgConnection {
    let options = pool.connect_options().as_ref().clone();
    PgConnection::connect_with(&options).await.unwrap()
}

/// จำนวนคำสั่งที่รอ lock อยู่ตอนนี้ ที่ query ขึ้นต้นด้วย `prefix` (ตัวพิมพ์ใหญ่เล็กไม่สำคัญ)
async fn blocked_with_prefix(conn: &mut PgConnection, prefix: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from pg_stat_activity
         where datname = current_database() and pid <> pg_backend_pid()
           and wait_event_type = 'Lock' and query ilike $1",
    )
    .bind(format!("{prefix}%"))
    .fetch_one(conn)
    .await
    .unwrap()
}

async fn wait_until_blocked(conn: &mut PgConnection, prefix: &str, want: i64) {
    let started = Instant::now();
    loop {
        if blocked_with_prefix(conn, prefix).await >= want {
            return;
        }
        assert!(
            started.elapsed() < Duration::from_secs(60),
            "statement '{prefix}...' never queued behind the row lock"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn lock_user_row(
    holder: &mut PgConnection,
    user_id: i32,
) -> sqlx::Transaction<'_, sqlx::Postgres> {
    let mut tx = holder.begin().await.unwrap();
    sqlx::query("select id from users where id = $1 for update")
        .bind(user_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    tx
}

/// เปลี่ยนรหัสผ่านที่ verify รหัสเดิมแล้วแต่ยังไม่เขียน (รอคิว/ล็อก) ต้องไม่เขียนทับผลของรีเซ็ตที่ commit ก่อน
#[sqlx::test]
async fn change_password_does_not_overwrite_a_committed_admin_reset(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    common::insert_user_with_password(&pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (victim_id, victim_uuid) =
        common::insert_user_with_password(&pool, "victim.one", OLD_PASSWORD, "nurse").await;
    let (admin_cookie, _) = common::login(&app, "admin.one", ADMIN_PASSWORD).await;
    let (victim_cookie, _) = common::login(&app, "victim.one", OLD_PASSWORD).await;

    let mut holder = raw_conn(&pool).await;
    let mut poller = raw_conn(&pool).await;
    let lock = lock_user_row(&mut holder, victim_id).await;

    // รีเซ็ตเข้าคิวก่อน (update ของรีเซ็ตมี failed_attempts) แล้วค่อยเริ่มเปลี่ยนรหัส (update ไม่มี failed_attempts)
    let reset = {
        let (app, cookie) = (app.clone(), admin_cookie.clone());
        let path = format!("/api/users/{victim_uuid}/reset-password");
        tokio::spawn(
            async move { common::send(&app, Method::POST, &path, Some(&cookie), None).await },
        )
    };
    wait_until_blocked(&mut poller, "update users%failed_attempts", 1).await;
    let change = {
        let (app, cookie) = (app.clone(), victim_cookie.clone());
        tokio::spawn(async move {
            common::send(
                &app,
                Method::POST,
                "/api/auth/password",
                Some(&cookie),
                Some(json!({ "currentPassword": OLD_PASSWORD, "newPassword": NEW_PASSWORD })),
            )
            .await
        })
    };
    wait_until_blocked(&mut poller, "update users", 2).await;
    lock.commit().await.unwrap();

    assert_eq!(reset.await.unwrap().status, StatusCode::OK);
    let change = change.await.unwrap();
    assert_eq!(
        change.status,
        StatusCode::UNAUTHORIZED,
        "stale change must not succeed"
    );

    // hash ยังเป็นค่าที่ใช้ไม่ได้ของรีเซ็ต: ทั้งรหัสเดิมและรหัสใหม่ของผู้ถือบัญชี login ไม่ได้
    let stored: String = sqlx::query_scalar("select password_hash from users where id = $1")
        .bind(victim_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(!common::verify_password(NEW_PASSWORD, &stored).await);
    assert!(!common::verify_password(OLD_PASSWORD, &stored).await);
    for password in [NEW_PASSWORD, OLD_PASSWORD] {
        let login = common::send(
            &app,
            Method::POST,
            "/api/auth/login",
            None,
            Some(json!({ "username": "victim.one", "password": password })),
        )
        .await;
        assert_eq!(login.status, StatusCode::UNAUTHORIZED);
    }
    let changed_audit: i64 =
        sqlx::query_scalar("select count(*) from audit_log where action = 'password_changed'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        changed_audit, 0,
        "a rejected change must not leave a password_changed row"
    );
}

/// login ที่ verify รหัสเดิมแล้วแต่ยังไม่ได้ insert session ต้องไม่ได้ session ที่ใช้ได้หลังรีเซ็ต commit
#[sqlx::test]
async fn login_in_flight_does_not_survive_a_committed_admin_reset(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    common::insert_user_with_password(&pool, "admin.two", ADMIN_PASSWORD, "admin").await;
    let (victim_id, victim_uuid) =
        common::insert_user_with_password(&pool, "victim.two", OLD_PASSWORD, "nurse").await;
    let (admin_cookie, _) = common::login(&app, "admin.two", ADMIN_PASSWORD).await;

    let mut holder = raw_conn(&pool).await;
    let mut poller = raw_conn(&pool).await;
    // login จองครั้งตรวจที่แถวผู้ใช้ก่อน verify จึงล็อกแถวผู้ใช้แทนไม่ได้ (login จะค้างตั้งแต่ขั้นจอง ไม่ถึงขั้นสร้าง session) —
    // ล็อกทั้งตาราง sessions แทน: INSERT/DELETE รอ ส่วน SELECT (ตรวจ session ของแอดมิน) ผ่านปกติ
    let mut gate = holder.begin().await.unwrap();
    sqlx::query("lock table sessions in share mode")
        .execute(&mut *gate)
        .await
        .unwrap();

    // login ผ่านขั้นจอง + verify รหัสเดิม (hash เดิม ยังไม่มีรีเซ็ต) แล้วค้างที่สร้าง session
    let login = {
        let app = app.clone();
        tokio::spawn(async move {
            common::send(
                &app,
                Method::POST,
                "/api/auth/login",
                None,
                Some(json!({ "username": "victim.two", "password": OLD_PASSWORD })),
            )
            .await
        })
    };
    wait_until_blocked(&mut poller, "insert into sessions", 1).await;
    // รีเซ็ตแก้แถวผู้ใช้ (ยังไม่ commit) แล้วค้างที่ลบ session ของเป้าหมาย
    let reset = {
        let (app, cookie) = (app.clone(), admin_cookie.clone());
        let path = format!("/api/users/{victim_uuid}/reset-password");
        tokio::spawn(
            async move { common::send(&app, Method::POST, &path, Some(&cookie), None).await },
        )
    };
    wait_until_blocked(&mut poller, "delete from sessions", 1).await;
    gate.commit().await.unwrap();

    assert_eq!(reset.await.unwrap().status, StatusCode::OK);
    let login = login.await.unwrap();
    assert_eq!(
        login.status,
        StatusCode::UNAUTHORIZED,
        "in-flight login must not succeed after the reset"
    );
    assert!(login.set_cookie.is_none());
    let sessions: i64 = sqlx::query_scalar("select count(*) from sessions where user_id = $1")
        .bind(victim_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sessions, 0, "no session may outlive the reset");
}

/// เดารหัสปัจจุบันผิดตอนเปลี่ยนรหัสได้ไม่เกินเพดานต่อ session — ครบแล้ว session นั้นถูกลบ (ไม่ล็อกบัญชี) และมี audit ทุกครั้ง
#[sqlx::test]
async fn wrong_current_password_is_capped_per_session_and_audited(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    let (user_id, _) =
        common::insert_user_with_password(&pool, "nurse.cap", OLD_PASSWORD, "nurse").await;
    let (cookie, _) = common::login(&app, "nurse.cap", OLD_PASSWORD).await;
    let guess = |password: &'static str| {
        let (app, cookie) = (app.clone(), cookie.clone());
        async move {
            common::send(
                &app,
                Method::POST,
                "/api/auth/password",
                Some(&cookie),
                Some(json!({ "currentPassword": password, "newPassword": NEW_PASSWORD })),
            )
            .await
        }
    };

    // 4 ครั้งแรกผิด: ยัง 401 และ session ยังใช้ได้
    for _ in 0..4 {
        assert_eq!(
            guess("wrong-current-password-x").await.status,
            StatusCode::UNAUTHORIZED
        );
    }
    let me = common::send(&app, Method::GET, "/api/auth/me", Some(&cookie), None).await;
    assert_eq!(me.status, StatusCode::OK);

    // ครั้งที่ 5 ผิด: session ถูกลบ
    assert_eq!(
        guess("wrong-current-password-x").await.status,
        StatusCode::UNAUTHORIZED
    );
    let me = common::send(&app, Method::GET, "/api/auth/me", Some(&cookie), None).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    // แม้เดาถูกต่อจากนั้นก็ใช้ไม่ได้ (ไม่มี session แล้ว)
    assert_eq!(guess(OLD_PASSWORD).await.status, StatusCode::UNAUTHORIZED);

    let failed_audit: i64 = sqlx::query_scalar(
        "select count(*) from audit_log where action = 'password_change_failed'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(failed_audit, 5);

    // เจ้าของยัง login ด้วยรหัสถูกได้ และตัวนับ/ล็อกของ login ไม่ถูกแตะ
    let (failed_attempts, locked): (i32, bool) =
        sqlx::query_as("select failed_attempts, locked_until is not null from users where id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!((failed_attempts, locked), (0, false));
    common::login(&app, "nurse.cap", OLD_PASSWORD).await;
}

/// รหัสถูกก่อนถึงเพดานยังเปลี่ยนรหัสได้ และตัวนับของ session เริ่มใหม่
#[sqlx::test]
async fn correct_current_password_before_the_cap_still_changes_it(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    common::insert_user_with_password(&pool, "nurse.ok", OLD_PASSWORD, "nurse").await;
    let (cookie, _) = common::login(&app, "nurse.ok", OLD_PASSWORD).await;
    for _ in 0..3 {
        let wrong = common::send(
            &app,
            Method::POST,
            "/api/auth/password",
            Some(&cookie),
            Some(json!({ "currentPassword": "wrong-current-password-x", "newPassword": NEW_PASSWORD })),
        )
        .await;
        assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    }
    let ok = common::send(
        &app,
        Method::POST,
        "/api/auth/password",
        Some(&cookie),
        Some(json!({ "currentPassword": OLD_PASSWORD, "newPassword": NEW_PASSWORD })),
    )
    .await;
    assert_eq!(ok.status, StatusCode::NO_CONTENT);
    let count: i32 = sqlx::query_scalar("select max(password_check_count) from sessions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    common::login(&app, "nurse.ok", NEW_PASSWORD).await;
}
