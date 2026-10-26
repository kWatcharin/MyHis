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

// ---- อำนาจของผู้ออกลิงก์ต้องยังอยู่ตอนใช้ลิงก์ (ไม่ใช่ตรวจแค่ตอนออก) ----

/// role แบบกำหนดเอง (SQL ตรง ๆ) คืน uuid ของ role
async fn custom_role(pool: &PgPool, name: &str, permissions: &[&str]) -> Uuid {
    let id: i32 = sqlx::query_scalar("insert into roles (name) values ($1) returning id")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap();
    for permission in permissions {
        sqlx::query("insert into role_permissions (role_id, permission) values ($1, $2)")
            .bind(id)
            .bind(permission)
            .execute(pool)
            .await
            .unwrap();
    }
    sqlx::query_scalar("select uuid from roles where id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn put_role(pool: &PgPool, user_id: i32, role: &str) {
    sqlx::query(
        "update user_roles set role_id = (select id from roles where name = $2) where user_id = $1",
    )
    .bind(user_id)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
}

struct Delegated {
    manager_uuid: Uuid,
    manager_cookie: String,
    target_id: i32,
    target_uuid: Uuid,
}

/// ผู้จัดการแบบกำหนดเอง (user.view/create/update + patient.view — ไม่ใช่ admin) กับเป้าหมายที่อยู่ในอำนาจ (patient.view อย่างเดียว)
async fn delegated(pool: &PgPool, s: &Setup) -> Delegated {
    custom_role(pool, "viewer-only", &["patient.view"]).await;
    custom_role(
        pool,
        "user-manager",
        &["user.view", "user.create", "user.update", "patient.view"],
    )
    .await;
    let (manager_id, manager_uuid) =
        common::insert_user_with_password(pool, "manager.one", ADMIN_PASSWORD, "nurse").await;
    put_role(pool, manager_id, "user-manager").await;
    let (target_id, target_uuid) =
        common::insert_user_with_password(pool, "target.one", ADMIN_PASSWORD, "nurse").await;
    put_role(pool, target_id, "viewer-only").await;
    let (manager_cookie, _) = common::login(&s.app, "manager.one", ADMIN_PASSWORD).await;
    Delegated {
        manager_uuid,
        manager_cookie,
        target_id,
        target_uuid,
    }
}

async fn manager_reset(s: &Setup, d: &Delegated) -> String {
    let response = common::send(
        &s.app,
        Method::POST,
        &format!("/api/users/{}/reset-password", d.target_uuid),
        Some(&d.manager_cookie),
        None,
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    response.body["registrationToken"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn password_hash_of(pool: &PgPool, user_id: i32) -> String {
    sqlx::query_scalar("select password_hash from users where id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// ลิงก์ที่ผู้จัดการออกให้เป้าหมาย ใช้ไม่ได้อีกเมื่อ admin เพิ่มสิทธิ์ให้เป้าหมายจนเกินผู้จัดการ (เปลี่ยน role หรือเพิ่มสิทธิ์ใน role)
#[sqlx::test]
async fn link_stops_working_when_the_target_gains_permissions_the_issuer_lacks(pool: PgPool) {
    let s = setup(&pool).await;
    let d = delegated(&pool, &s).await;
    let (_, prename_uuid) = prename(&pool, s.admin_id, "นางสาว").await;

    // (1) admin เปลี่ยน role ของเป้าหมายเป็น admin
    let token = manager_reset(&s, &d).await;
    let before = password_hash_of(&pool, d.target_id).await;
    let admin_role: Uuid = sqlx::query_scalar("select uuid from roles where name = 'admin'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let replaced = common::send(
        &s.app,
        Method::PUT,
        &format!("/api/users/{}/roles", d.target_uuid),
        Some(&s.admin_cookie),
        Some(json!({ "roleIds": [admin_role] })),
    )
    .await;
    assert_eq!(replaced.status, StatusCode::NO_CONTENT);
    // ผู้จัดการออกลิงก์ใหม่ให้บัญชีนี้เองก็ไม่ได้แล้ว (กฎ ณ ตอนออก) — ลิงก์เก่าต้องใช้ไม่ได้เช่นกัน
    assert_invalid_link(&lookup(&s.app, &token).await, "lookup after promotion");
    assert_invalid_link(
        &complete(&s.app, register_body(&token, prename_uuid)).await,
        "complete after promotion",
    );
    assert_eq!(password_hash_of(&pool, d.target_id).await, before);

    // (2) เพิ่มสิทธิ์ใน role ของเป้าหมาย (เทียบเท่า PATCH /roles) หลังออกลิงก์
    put_role(&pool, d.target_id, "viewer-only").await;
    let token = manager_reset(&s, &d).await;
    sqlx::query(
        "insert into role_permissions (role_id, permission)
         select id, 'role.update' from roles where name = 'viewer-only'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_invalid_link(
        &complete(&s.app, register_body(&token, prename_uuid)).await,
        "complete after role permission edit",
    );

    // (3) ลิงก์ที่ admin ออกให้ ยังใช้ได้แม้เป้าหมายมีสิทธิ์เพิ่ม (admin ถือครบทุกสิทธิ์)
    let admin_token = reset(&s, d.target_uuid).await;
    let done = complete(&s.app, register_body(&admin_token, prename_uuid)).await;
    assert_eq!(done.status, StatusCode::NO_CONTENT);
}

/// ลิงก์ที่ผู้จัดการออกไว้ ใช้ไม่ได้ขณะผู้จัดการถูกปิดบัญชี (ใช้ได้อีกถ้าเปิดคืน) และใช้ไม่ได้ถ้าผู้จัดการเสียสิทธิ์ออกลิงก์
#[sqlx::test]
async fn link_stops_working_while_its_issuer_is_deactivated_or_loses_user_permissions(
    pool: PgPool,
) {
    let s = setup(&pool).await;
    let d = delegated(&pool, &s).await;
    let (_, prename_uuid) = prename(&pool, s.admin_id, "นาย").await;
    let token = manager_reset(&s, &d).await;

    set_active(&s, d.manager_uuid, false).await;
    assert_invalid_link(
        &complete(&s.app, register_body(&token, prename_uuid)).await,
        "issuer deactivated",
    );

    set_active(&s, d.manager_uuid, true).await;
    sqlx::query(
        "delete from role_permissions
         where permission in ('user.create', 'user.update')
           and role_id = (select id from roles where name = 'user-manager')",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert_invalid_link(
        &complete(&s.app, register_body(&token, prename_uuid)).await,
        "issuer lost user.create/user.update",
    );

    sqlx::query(
        "insert into role_permissions (role_id, permission)
         select id, 'user.update' from roles where name = 'user-manager'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let done = complete(&s.app, register_body(&token, prename_uuid)).await;
    assert_eq!(done.status, StatusCode::NO_CONTENT);
}

/// ลิงก์ผูกกับ ‘การกระทำที่ออกลิงก์’ (security audit รอบ 2): ลิงก์รีเซ็ตที่ผู้จัดการออกไว้ ใช้ไม่ได้หลังผู้จัดการเสียเฉพาะ `user.update`
/// แม้ยังถือ `user.create` (ผู้ถือ user.create อย่างเดียวรีเซ็ตใครไม่ได้ ออกรีเซ็ตใหม่ก็ 403) ทั้งตอนตรวจ (lookup) และตอนใช้ (complete)
/// รหัสผ่านของเป้าหมายต้องไม่เปลี่ยน; ลิงก์สร้างบัญชีที่ผู้จัดการออกยังใช้ได้เมื่อเหลือแค่ `user.create`
#[sqlx::test]
async fn a_reset_link_needs_user_update_from_its_issuer_but_a_create_link_accepts_user_create(
    pool: PgPool,
) {
    let s = setup(&pool).await;
    let d = delegated(&pool, &s).await;
    let (_, prename_uuid) = prename(&pool, s.admin_id, "นาย").await;

    // ลิงก์สร้างบัญชีที่ผู้จัดการออกเอง (ใช้ user.create) กับผู้ใช้ใหม่
    let viewer_role: Uuid = sqlx::query_scalar("select uuid from roles where name = 'viewer-only'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let created = common::send(
        &s.app,
        Method::POST,
        "/api/users",
        Some(&d.manager_cookie),
        Some(json!({ "username": "fresh.one", "displayName": "ผู้ใช้ใหม่", "roleId": viewer_role })),
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let create_token = created.body["registrationToken"]
        .as_str()
        .unwrap()
        .to_string();

    // ลิงก์รีเซ็ตที่ผู้จัดการออก (ใช้ user.update) ให้เป้าหมายที่ลงทะเบียนแล้ว
    let reset_token = manager_reset(&s, &d).await;
    let purposes: Vec<(String, String)> = sqlx::query_as(
        "select u.username, l.purpose from registration_links l join users u on u.id = l.user_id
         where l.used_at is null and l.revoked_at is null order by u.username",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        purposes,
        vec![
            ("fresh.one".to_string(), "create".to_string()),
            ("target.one".to_string(), "reset".to_string()),
        ]
    );
    let before = password_hash_of(&pool, d.target_id).await;

    // ถอดเฉพาะ user.update ออกจาก role ของผู้จัดการ (ยังเหลือ user.create)
    sqlx::query(
        "delete from role_permissions
         where permission = 'user.update'
           and role_id = (select id from roles where name = 'user-manager')",
    )
    .execute(&pool)
    .await
    .unwrap();
    // ออกรีเซ็ตใหม่เองไม่ได้แล้ว
    let denied = common::send(
        &s.app,
        Method::POST,
        &format!("/api/users/{}/reset-password", d.target_uuid),
        Some(&d.manager_cookie),
        None,
    )
    .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    // ลิงก์รีเซ็ตเก่าต้องใช้ไม่ได้เช่นกัน และรหัสผ่านไม่เปลี่ยน
    assert_invalid_link(
        &lookup(&s.app, &reset_token).await,
        "lookup of reset link without user.update",
    );
    assert_invalid_link(
        &complete(
            &s.app,
            json!({ "token": reset_token, "newPassword": "new-password-67890" }),
        )
        .await,
        "complete of reset link without user.update",
    );
    assert_eq!(password_hash_of(&pool, d.target_id).await, before);

    // ลิงก์สร้างบัญชียังใช้ได้ (user.create พอ)
    assert_eq!(lookup(&s.app, &create_token).await.status, StatusCode::OK);
    let done = complete(&s.app, register_body(&create_token, prename_uuid)).await;
    assert_eq!(done.status, StatusCode::NO_CONTENT);
}
