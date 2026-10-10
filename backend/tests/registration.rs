//! เทสการลงทะเบียนผ่านลิงก์ (โมดูล registration — endpoint สาธารณะ) ยิงผ่าน `app::router` จริงบน DB สะอาด:
//! ลิงก์ที่ใช้ไม่ได้ทุกแบบได้ข้อความกลางเดียว, โหมดลงทะเบียนต้องกรอกข้อมูลตัวครบและไม่เขียนอะไรถ้าผิด,
//! โหมดตั้งรหัสใหม่ไม่แตะข้อมูลตัว, ใช้ซ้ำไม่ได้, audit ไม่มี token/รหัสผ่าน
//!
//! รหัสผ่านทั้งหมดเป็นค่าทดสอบ ไม่ใช่ของจริง

mod common;

use axum::http::{Method, StatusCode};
use axum::Router;
use chrono::{DateTime, Utc};
use common::TestResponse;
use my_his_backend::token;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const ADMIN_PASSWORD: &str = "admin-test-password-1";
const INVALID_LINK: &str = "invalid or expired registration link";

struct Setup {
    app: Router,
    admin_id: i32,
    admin_cookie: String,
}

async fn setup(pool: &PgPool) -> Setup {
    let app = common::test_app(pool.clone()).await;
    let (admin_id, _) =
        common::insert_user_with_password(pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (admin_cookie, _) = common::login(&app, "admin.one", ADMIN_PASSWORD).await;
    Setup {
        app,
        admin_id,
        admin_cookie,
    }
}

/// แอดมินสร้างผู้ใช้ผ่าน API — คืน (uuid, token ของลิงก์ลงทะเบียน)
async fn create_user(pool: &PgPool, s: &Setup, username: &str) -> (Uuid, String) {
    let nurse_role: Uuid = sqlx::query_scalar("select uuid from roles where name = 'nurse'")
        .fetch_one(pool)
        .await
        .unwrap();
    let response = common::send(
        &s.app,
        Method::POST,
        "/api/users",
        Some(&s.admin_cookie),
        Some(json!({ "username": username, "displayName": "ผู้ใช้ทดสอบ", "roleId": nurse_role })),
    )
    .await;
    assert_eq!(response.status, StatusCode::CREATED);
    let id = Uuid::parse_str(response.body["user"]["id"].as_str().unwrap()).unwrap();
    let link_token = response.body["registrationToken"]
        .as_str()
        .unwrap()
        .to_string();
    (id, link_token)
}

async fn reset(s: &Setup, user: Uuid) -> String {
    let response = common::send(
        &s.app,
        Method::POST,
        &format!("/api/users/{user}/reset-password"),
        Some(&s.admin_cookie),
        None,
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    response.body["registrationToken"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn set_active(s: &Setup, user: Uuid, active: bool) {
    let response = common::send(
        &s.app,
        Method::PATCH,
        &format!("/api/users/{user}"),
        Some(&s.admin_cookie),
        Some(json!({ "isActive": active })),
    )
    .await;
    assert_eq!(response.status, StatusCode::NO_CONTENT);
}

async fn lookup(app: &Router, link_token: &str) -> TestResponse {
    common::send(
        app,
        Method::POST,
        "/api/registration/lookup",
        None,
        Some(json!({ "token": link_token })),
    )
    .await
}

async fn complete(app: &Router, body: Value) -> TestResponse {
    common::send(
        app,
        Method::POST,
        "/api/registration/complete",
        None,
        Some(body),
    )
    .await
}

async fn prename(pool: &PgPool, admin_id: i32, name: &str) -> (i32, Uuid) {
    let id = common::insert_prename(pool, admin_id, name).await;
    let uuid = sqlx::query_scalar("select uuid from prename where id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    (id, uuid)
}

fn register_body(link_token: &str, prename: Uuid) -> Value {
    json!({
        "token": link_token,
        "newPassword": "my-own-password-123",
        "prenameId": prename,
        "firstName": "สมศรี",
        "lastName": "ใจดี",
    })
}

fn assert_invalid_link(response: &TestResponse, label: &str) {
    assert_eq!(response.status, StatusCode::BAD_REQUEST, "{label}");
    assert_eq!(response.body["error"], INVALID_LINK, "{label}");
}

#[derive(sqlx::FromRow)]
struct RegisteredUser {
    prename_id: Option<i32>,
    first_name: Option<String>,
    last_name: Option<String>,
    position: Option<String>,
    must_change_password: bool,
    failed_attempts: i32,
    registered_at: Option<DateTime<Utc>>,
}

async fn registered_at(pool: &PgPool, user: Uuid) -> Option<DateTime<Utc>> {
    sqlx::query_scalar("select registered_at from users where uuid = $1")
        .bind(user)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[sqlx::test]
async fn unusable_links_all_get_the_same_generic_error(pool: PgPool) {
    let s = setup(&pool).await;
    let (_, prename_uuid) = prename(&pool, s.admin_id, "นาง").await;

    // token ที่ไม่มีอยู่จริง
    let random = token::generate_token();
    assert_invalid_link(&lookup(&s.app, &random).await, "random lookup");
    assert_invalid_link(
        &complete(&s.app, register_body(&random, prename_uuid)).await,
        "random complete",
    );

    // หมดอายุ
    let (_, expired) = create_user(&pool, &s, "expired.user").await;
    sqlx::query(
        "update registration_links set expires_at = now() - interval '1 minute' where token_hash = $1",
    )
    .bind(token::hash_token(&expired))
    .execute(&pool)
    .await
    .unwrap();
    assert_invalid_link(&lookup(&s.app, &expired).await, "expired lookup");
    assert_invalid_link(
        &complete(&s.app, register_body(&expired, prename_uuid)).await,
        "expired complete",
    );

    // ถูกแทนด้วยลิงก์ใหม่ (รีเซ็ต/ออกลิงก์ใหม่) → ลิงก์เดิมใช้ไม่ได้ ลิงก์ใหม่ใช้ได้
    let (user, first) = create_user(&pool, &s, "relinked.user").await;
    let second = reset(&s, user).await;
    assert_invalid_link(&lookup(&s.app, &first).await, "revoked lookup");
    assert_eq!(lookup(&s.app, &second).await.status, StatusCode::OK);

    // บัญชีถูกปิด → ลิงก์ใช้ไม่ได้ เปิดกลับแล้วลิงก์เดิมที่ยังไม่หมดอายุใช้ได้อีก
    set_active(&s, user, false).await;
    assert_invalid_link(&lookup(&s.app, &second).await, "inactive lookup");
    assert_invalid_link(
        &complete(&s.app, register_body(&second, prename_uuid)).await,
        "inactive complete",
    );
    assert!(registered_at(&pool, user).await.is_none());
    set_active(&s, user, true).await;
    let found = lookup(&s.app, &second).await;
    assert_eq!(found.status, StatusCode::OK);
    assert_eq!(found.body["mode"], "register");

    // ใช้แล้ว → ใช้ซ้ำไม่ได้
    assert_eq!(
        complete(&s.app, register_body(&second, prename_uuid))
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_invalid_link(&lookup(&s.app, &second).await, "used lookup");
    assert_invalid_link(
        &complete(&s.app, register_body(&second, prename_uuid)).await,
        "used complete",
    );
}

#[sqlx::test]
async fn register_mode_requires_profile_and_writes_nothing_on_error(pool: PgPool) {
    let s = setup(&pool).await;
    let (prename_id, prename_uuid) = prename(&pool, s.admin_id, "นางสาว").await;
    let (deleted_id, deleted_uuid) = prename(&pool, s.admin_id, "ลบแล้ว").await;
    common::soft_delete_row(&pool, "prename", deleted_id, s.admin_id).await;
    let (user, link_token) = create_user(&pool, &s, "new.nurse").await;

    let cases: Vec<(Value, &str)> = vec![
        (
            json!({ "token": link_token, "newPassword": "my-own-password-123", "firstName": "a", "lastName": "b" }),
            "prename is required",
        ),
        (
            json!({ "token": link_token, "newPassword": "my-own-password-123", "prenameId": Uuid::new_v4(), "firstName": "a", "lastName": "b" }),
            "unknown prename",
        ),
        (
            json!({ "token": link_token, "newPassword": "my-own-password-123", "prenameId": deleted_uuid, "firstName": "a", "lastName": "b" }),
            "unknown prename",
        ),
        (
            json!({ "token": link_token, "newPassword": "my-own-password-123", "prenameId": prename_uuid, "firstName": "   ", "lastName": "b" }),
            "first name is required",
        ),
        (
            json!({ "token": link_token, "newPassword": "my-own-password-123", "prenameId": prename_uuid, "firstName": "a" }),
            "last name is required",
        ),
        (
            json!({ "token": link_token, "newPassword": "my-own-password-123", "prenameId": prename_uuid, "firstName": "a", "lastName": "b", "position": "ก".repeat(101) }),
            "position must be at most 100 characters",
        ),
    ];
    for (body, expected) in cases {
        let response = complete(&s.app, body).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{expected}");
        assert_eq!(response.body["error"], expected);
    }
    let short = complete(
        &s.app,
        json!({ "token": link_token, "newPassword": "short", "prenameId": prename_uuid, "firstName": "a", "lastName": "b" }),
    )
    .await;
    assert_eq!(short.status, StatusCode::BAD_REQUEST);
    assert!(short.body["error"]
        .as_str()
        .unwrap()
        .contains("at least 12 characters"));

    // ทุกคำขอที่ผิดไม่เขียนอะไร — ลิงก์ยังใช้ได้ ยังไม่ลงทะเบียน
    assert_eq!(lookup(&s.app, &link_token).await.status, StatusCode::OK);
    assert!(registered_at(&pool, user).await.is_none());

    // บัญชีที่ยังไม่ลงทะเบียนถูกล็อกได้จากการเดารหัส — ลงทะเบียนสำเร็จแล้วปลดล็อกให้ (เข้าได้ทันที)
    for _ in 0..5 {
        common::send(
            &s.app,
            Method::POST,
            "/api/auth/login",
            None,
            Some(json!({ "username": "new.nurse", "password": "guessing-the-password" })),
        )
        .await;
    }
    let locked: Option<DateTime<Utc>> =
        sqlx::query_scalar("select locked_until from users where uuid = $1")
            .bind(user)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(locked.is_some());

    let done = complete(
        &s.app,
        json!({
            "token": link_token,
            "newPassword": "my-own-password-123",
            "prenameId": prename_uuid,
            "firstName": "  สมศรี ",
            "lastName": "ใจดี",
        }),
    )
    .await;
    assert_eq!(done.status, StatusCode::NO_CONTENT);
    common::login(&s.app, "new.nurse", "my-own-password-123").await;

    let row: RegisteredUser = sqlx::query_as(
        "select prename_id, first_name, last_name, position, must_change_password, failed_attempts,
                registered_at
         from users where uuid = $1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row.prename_id, Some(prename_id));
    assert_eq!(row.first_name.as_deref(), Some("สมศรี"));
    assert_eq!(row.last_name.as_deref(), Some("ใจดี"));
    assert_eq!(row.position, None); // ตำแหน่งไม่บังคับ
    assert!(!row.must_change_password);
    // ลงทะเบียนล้างตัวนับที่เดารหัสผิดไว้ (login ที่ผ่านหลังจากนั้นก็ล้างอีกรอบ)
    assert_eq!(row.failed_attempts, 0);
    assert!(row.registered_at.is_some());

    let used_at: Option<DateTime<Utc>> =
        sqlx::query_scalar("select used_at from registration_links where token_hash = $1")
            .bind(token::hash_token(&link_token))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(used_at.is_some());
}

#[sqlx::test]
async fn reset_mode_sets_password_only_and_audits_without_secrets(pool: PgPool) {
    let s = setup(&pool).await;
    let (prename_id, other_prename) = prename(&pool, s.admin_id, "นาย").await;
    let (user_id, user_uuid) =
        common::insert_user_with_password(&pool, "old.nurse", "old-password-12345", "nurse").await;
    sqlx::query(
        "update users set prename_id = $2, first_name = 'เดิม', last_name = 'นามสกุลเดิม', position = 'หัวหน้า'
         where id = $1",
    )
    .bind(user_id)
    .bind(prename_id)
    .execute(&pool)
    .await
    .unwrap();

    let link_token = reset(&s, user_uuid).await;
    let found = lookup(&s.app, &link_token).await;
    assert_eq!(found.status, StatusCode::OK);
    assert_eq!(found.body["mode"], "reset");
    assert_eq!(found.body["firstName"], "เดิม");

    // โหมดตั้งรหัสใหม่ไม่แตะข้อมูลตัว แม้ส่งมา
    let done = complete(
        &s.app,
        json!({
            "token": link_token,
            "newPassword": "new-password-67890",
            "prenameId": other_prename,
            "firstName": "ชื่ออื่น",
            "lastName": "สกุลอื่น",
        }),
    )
    .await;
    assert_eq!(done.status, StatusCode::NO_CONTENT);
    common::login(&s.app, "old.nurse", "new-password-67890").await;
    let names: (Option<String>, Option<String>, Option<String>) =
        sqlx::query_as("select first_name, last_name, position from users where id = $1")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        names,
        (
            Some("เดิม".to_string()),
            Some("นามสกุลเดิม".to_string()),
            Some("หัวหน้า".to_string())
        )
    );

    // audit: ผู้กระทำคือเจ้าของบัญชีเอง ไม่มี metadata และไม่มี token/รหัสผ่านอยู่ในแถวใดเลย
    let (actor, metadata): (Option<i32>, Option<Value>) = sqlx::query_as(
        "select actor_user_id, metadata from audit_log
         where action = 'user_password_set' and target_type = 'user' and target_id = $1",
    )
    .bind(user_uuid.to_string())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actor, Some(user_id));
    assert!(metadata.is_none());
    let all_audit: Vec<String> = sqlx::query_scalar(
        "select action || '|' || coalesce(target_id, '') || '|' || coalesce(metadata::text, '') from audit_log",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let text = all_audit.join("\n");
    for secret in [
        link_token.as_str(),
        token::hash_token(&link_token).as_str(),
        "new-password-67890",
    ] {
        assert!(!text.contains(secret), "secret leaked into audit_log");
    }
}
