//! ห้ามให้สิทธิ์เกินตัวเอง (docs/auth-design.md ข้อ 10): คนที่จัดการผู้ใช้/role ได้แต่ไม่ใช่ admin
//! ต้องยกตัวเองหรือคนอื่นขึ้นเป็น admin ไม่ได้ และแตะบัญชี/role ที่มีสิทธิ์มากกว่าตัวเองไม่ได้
//! (รีเซ็ตรหัส admin แล้วได้ลิงก์ตั้งรหัสใหม่ = ยึดบัญชี admin) — admin (มีครบทุกสิทธิ์) ทำได้ตามเดิม

mod common;

use axum::http::{Method, StatusCode};
use axum::Router;
use common::{insert_user_with_password, login, send};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const PASSWORD: &str = "ceiling-test-password-1";
const GRANT: &str = "cannot grant permissions you do not have";
const MANAGE_USER: &str = "cannot manage a user with permissions you do not have";
const MANAGE_ROLE: &str = "cannot manage a role with permissions you do not have";

async fn create_role(pool: &PgPool, name: &str, permissions: &[&str]) -> Uuid {
    sqlx::query_scalar(
        "with r as (insert into roles (name) values ($1) returning id, uuid),
              p as (insert into role_permissions (role_id, permission)
                    select r.id, unnest($2::text[]) from r)
         select uuid from r",
    )
    .bind(name)
    .bind(permissions)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn role_uuid(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("select uuid from roles where name = $1")
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn call(
    app: &Router,
    cookie: &str,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = send(app, method, path, Some(cookie), body).await;
    (response.status, response.body)
}

fn forbidden(reason: &str) -> (StatusCode, Value) {
    (StatusCode::FORBIDDEN, json!({ "error": reason }))
}

async fn roles_of(pool: &PgPool, user: Uuid) -> Vec<String> {
    let mut names: Vec<String> = sqlx::query_scalar(
        "select r.name from user_roles ur join roles r on r.id = ur.role_id
         join users u on u.id = ur.user_id where u.uuid = $1",
    )
    .bind(user)
    .fetch_all(pool)
    .await
    .unwrap();
    names.sort();
    names
}

/// ผู้จัดการผู้ใช้ (user.* + ดูผู้ป่วย ไม่มีสิทธิ์ role) ยกตัวเอง/คนอื่นเป็น admin ไม่ได้ และแตะ admin ไม่ได้
#[sqlx::test]
async fn user_manager_cannot_escalate_or_touch_stronger_accounts(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    create_role(
        &pool,
        "จัดการผู้ใช้",
        &["user.view", "user.create", "user.update", "patient.view"],
    )
    .await;
    let viewer_role = create_role(&pool, "ดูผู้ป่วย", &["patient.view"]).await;
    let admin_role = role_uuid(&pool, "admin").await;
    let manager_role = role_uuid(&pool, "จัดการผู้ใช้").await;

    let (_, manager) = insert_user_with_password(&pool, "manager.one", PASSWORD, "จัดการผู้ใช้").await;
    let (_, admin) = insert_user_with_password(&pool, "admin.one", PASSWORD, "admin").await;
    let (_, viewer) = insert_user_with_password(&pool, "viewer.one", PASSWORD, "ดูผู้ป่วย").await;
    let (cookie, _) = login(&app, "manager.one", PASSWORD).await;

    // ยกตัวเองเป็น admin — ทั้งสองทาง
    assert_eq!(
        call(
            &app,
            &cookie,
            Method::PUT,
            &format!("/api/users/{manager}/roles"),
            Some(json!({ "roleIds": [manager_role, admin_role] }))
        )
        .await,
        forbidden(GRANT)
    );
    assert_eq!(
        call(
            &app,
            &cookie,
            Method::PUT,
            &format!("/api/users/{manager}"),
            Some(json!({ "displayName": "manager.one", "roleIds": [admin_role] }))
        )
        .await,
        forbidden(GRANT)
    );
    assert_eq!(roles_of(&pool, manager).await, ["จัดการผู้ใช้"]);

    // สร้างบัญชี admin ใหม่ (ได้ลิงก์ลงทะเบียนมาเอง) / ยกคนอื่นเป็น admin
    assert_eq!(
        call(
            &app,
            &cookie,
            Method::POST,
            "/api/users",
            Some(json!({ "username": "sneaky.admin", "displayName": "x", "roleId": admin_role }))
        )
        .await,
        forbidden(GRANT)
    );
    assert_eq!(
        call(
            &app,
            &cookie,
            Method::PUT,
            &format!("/api/users/{viewer}/roles"),
            Some(json!({ "roleIds": [admin_role] }))
        )
        .await,
        forbidden(GRANT)
    );

    // แตะบัญชี admin (สิทธิ์มากกว่าตัวเอง): รีเซ็ตรหัส, ปิดบัญชี, แก้ข้อมูล, เปลี่ยน role
    assert_eq!(
        call(
            &app,
            &cookie,
            Method::POST,
            &format!("/api/users/{admin}/reset-password"),
            None
        )
        .await,
        forbidden(MANAGE_USER)
    );
    assert_eq!(
        call(
            &app,
            &cookie,
            Method::PATCH,
            &format!("/api/users/{admin}"),
            Some(json!({ "isActive": false }))
        )
        .await,
        forbidden(MANAGE_USER)
    );
    assert_eq!(
        call(
            &app,
            &cookie,
            Method::PUT,
            &format!("/api/users/{admin}"),
            Some(json!({ "displayName": "x", "roleIds": [viewer_role] }))
        )
        .await,
        forbidden(MANAGE_USER)
    );
    assert_eq!(
        call(
            &app,
            &cookie,
            Method::PUT,
            &format!("/api/users/{admin}/roles"),
            Some(json!({ "roleIds": [viewer_role] }))
        )
        .await,
        forbidden(MANAGE_USER)
    );
    assert_eq!(roles_of(&pool, admin).await, ["admin"]);
    let active: bool = sqlx::query_scalar("select is_active from users where uuid = $1")
        .bind(admin)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(active);

    // ยังทำงานปกติได้กับบัญชีที่สิทธิ์ไม่เกินตัวเอง และให้ role ที่สิทธิ์ไม่เกินตัวเองได้
    let (status, body) = call(
        &app,
        &cookie,
        Method::POST,
        "/api/users",
        Some(json!({ "username": "new.viewer", "displayName": "ผู้ดูใหม่", "roleId": viewer_role })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, _) = call(
        &app,
        &cookie,
        Method::POST,
        &format!("/api/users/{viewer}/reset-password"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &app,
        &cookie,
        Method::PUT,
        &format!("/api/users/{viewer}/roles"),
        Some(json!({ "roleIds": [viewer_role, manager_role] })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(roles_of(&pool, viewer).await, ["จัดการผู้ใช้", "ดูผู้ป่วย"]);
}

/// ผู้จัดการสิทธิ์ (role.* + ดูผู้ป่วย) สร้าง/แก้ role ให้มีสิทธิ์ที่ตัวเองไม่มีไม่ได้ และแก้/ลบ role ที่สิทธิ์มากกว่าตัวเองไม่ได้
#[sqlx::test]
async fn role_manager_cannot_grant_beyond_own_permissions(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    create_role(
        &pool,
        "จัดการสิทธิ์",
        &[
            "role.view",
            "role.create",
            "role.update",
            "role.delete",
            "patient.view",
        ],
    )
    .await;
    insert_user_with_password(&pool, "role.manager", PASSWORD, "จัดการสิทธิ์").await;
    let (cookie, _) = login(&app, "role.manager", PASSWORD).await;
    let admin_role = role_uuid(&pool, "admin").await;

    assert_eq!(
        call(
            &app,
            &cookie,
            Method::POST,
            "/api/roles",
            Some(json!({ "name": "ลบผู้ป่วย", "permissions": ["patient.view", "patient.delete"] }))
        )
        .await,
        forbidden(GRANT)
    );
    let (status, body) = call(
        &app,
        &cookie,
        Method::POST,
        "/api/roles",
        Some(json!({ "name": "ดูอย่างเดียว", "permissions": ["patient.view"] })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let own = role_uuid(&pool, "ดูอย่างเดียว").await;

    assert_eq!(
        call(
            &app,
            &cookie,
            Method::PATCH,
            &format!("/api/roles/{own}"),
            Some(json!({ "permissions": ["patient.view", "patient.delete"] }))
        )
        .await,
        forbidden(GRANT)
    );
    // role admin มีสิทธิ์มากกว่าตัวเอง — แก้ (แม้แค่คำอธิบาย/ลดสิทธิ์) ไม่ได้
    assert_eq!(
        call(
            &app,
            &cookie,
            Method::PATCH,
            &format!("/api/roles/{admin_role}"),
            Some(json!({ "permissions": ["patient.view"] }))
        )
        .await,
        forbidden(MANAGE_ROLE)
    );
    assert_eq!(
        call(
            &app,
            &cookie,
            Method::PATCH,
            &format!("/api/roles/{admin_role}"),
            Some(json!({ "description": "ถูกแก้" }))
        )
        .await,
        forbidden(MANAGE_ROLE)
    );

    let (status, _) = call(
        &app,
        &cookie,
        Method::PATCH,
        &format!("/api/roles/{own}"),
        Some(json!({ "description": "ดูทะเบียนผู้ป่วย" })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(
        &app,
        &cookie,
        Method::DELETE,
        &format!("/api/roles/{own}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}
