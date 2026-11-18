//! แก้ role ผ่าน API: role ของระบบ (admin/nurse) แก้คำอธิบาย/สิทธิ์ได้ แม้ฟอร์มส่งชื่อเดิมมาด้วย แต่เปลี่ยนชื่อไม่ได้
//! และแอดมินตัดสิทธิ์จัดการ role (`role.view` + `role.update`) ของตัวเองจนไม่เหลือไม่ได้ (กันล็อกตัวเองออก)

mod common;

use axum::http::{Method, StatusCode};
use axum::Router;
use common::{insert_user_with_password, login, send, test_app};
use serde_json::{json, Value};
use sqlx::PgPool;

const ADMIN_PASSWORD: &str = "admin-test-password-1";

struct Admin {
    app: Router,
    cookie: String,
    user_id: i32,
}

impl Admin {
    async fn new(pool: &PgPool) -> Self {
        let app = test_app(pool.clone()).await;
        let (user_id, _) =
            insert_user_with_password(pool, "admin.one", ADMIN_PASSWORD, "admin").await;
        let (cookie, _) = login(&app, "admin.one", ADMIN_PASSWORD).await;
        Self {
            app,
            cookie,
            user_id,
        }
    }

    async fn role(&self, name: &str) -> Value {
        let response = send(
            &self.app,
            Method::GET,
            "/api/roles",
            Some(&self.cookie),
            None,
        )
        .await;
        assert_eq!(response.status, StatusCode::OK);
        response
            .body
            .as_array()
            .unwrap()
            .iter()
            .find(|role| role["name"] == name)
            .unwrap_or_else(|| panic!("role {name} not found"))
            .clone()
    }

    async fn patch_role(&self, name: &str, body: Value) -> (StatusCode, Value) {
        let id = self.role(name).await["id"].as_str().unwrap().to_string();
        let response = send(
            &self.app,
            Method::PATCH,
            &format!("/api/roles/{id}"),
            Some(&self.cookie),
            Some(body),
        )
        .await;
        (response.status, response.body)
    }
}

/// เคสที่เจอจริง: ฟอร์มแก้ role ส่งชื่อเดิม (ช่องชื่อถูกล็อก) มาพร้อมสิทธิ์ใหม่
#[sqlx::test]
async fn system_role_permissions_change_when_the_name_is_unchanged(pool: PgPool) {
    let admin = Admin::new(&pool).await;

    let (status, body) = admin
        .patch_role(
            "nurse",
            json!({ "name": "nurse", "description": "พยาบาลวอร์ด", "permissions": ["patient.view"] }),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let nurse = admin.role("nurse").await;
    assert_eq!(nurse["permissions"], json!(["patient.view"]));
    assert_eq!(nurse["description"], "พยาบาลวอร์ด");

    // ไม่ส่งชื่อเลยก็ได้
    let (status, _) = admin
        .patch_role(
            "nurse",
            json!({ "permissions": ["patient.view", "patient.update"] }),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        admin.role("nurse").await["permissions"],
        json!(["patient.update", "patient.view"])
    );

    // คีย์เก่า (ก่อนแยกสิทธิ์รายเพจ) ไม่รับแล้ว
    let (status, _) = admin
        .patch_role("nurse", json!({ "permissions": ["patient.read"] }))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[sqlx::test]
async fn system_role_cannot_be_renamed(pool: PgPool) {
    let admin = Admin::new(&pool).await;
    let (status, body) = admin
        .patch_role("nurse", json!({ "name": "nurse2", "permissions": [] }))
        .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("cannot rename a system role"))
    );
    // ทั้ง request ถูกปฏิเสธ สิทธิ์เดิมไม่ถูกแตะ
    assert_ne!(admin.role("nurse").await["permissions"], json!([]));
}

#[sqlx::test]
async fn admin_cannot_remove_their_last_role_management(pool: PgPool) {
    let admin = Admin::new(&pool).await;
    let before = admin.role("admin").await["permissions"].clone();

    // ตัดตัวใดตัวหนึ่งของ role.view / role.update ก็ไม่ได้
    for permissions in [
        json!(["patient.view"]),
        json!(["role.view", "patient.view"]),
        json!(["role.update", "patient.view"]),
    ] {
        let (status, body) = admin
            .patch_role(
                "admin",
                json!({ "name": "admin", "permissions": permissions }),
            )
            .await;
        assert_eq!(
            (status, body["error"].as_str()),
            (
                StatusCode::BAD_REQUEST,
                Some("cannot remove your own role management permission")
            ),
            "{permissions}"
        );
    }
    assert_eq!(admin.role("admin").await["permissions"], before);

    // ยังเก็บ role.view + role.update ไว้ = ตัดสิทธิ์อื่นของ role admin ได้ (role.create ไว้สร้าง role ข้างล่าง)
    let (status, _) = admin
        .patch_role(
            "admin",
            json!({ "permissions": ["role.view", "role.update", "role.create", "patient.view"] }),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // มี role อื่นที่ให้สิทธิ์จัดการ role กับตัวเองแล้ว → ตัดออกจาก admin ได้
    let response = send(
        &admin.app,
        Method::POST,
        "/api/roles",
        Some(&admin.cookie),
        Some(json!({ "name": "manager", "permissions": ["role.view", "role.update"] })),
    )
    .await;
    assert_eq!(response.status, StatusCode::CREATED);
    sqlx::query(
        "insert into user_roles (user_id, role_id) select $1, id from roles where name = 'manager'",
    )
    .bind(admin.user_id)
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = admin
        .patch_role("admin", json!({ "permissions": ["patient.view"] }))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    assert_eq!(
        admin.role("admin").await["permissions"],
        json!(["patient.view"])
    );
}

/// รายการ permission ที่ยาวเกินจำนวน permission ทั้งหมดในระบบ (ย่อมมีค่าซ้ำ) ถูกปฏิเสธก่อนประมวลผล — ทั้งสร้างและแก้ role
#[sqlx::test]
async fn permission_list_longer_than_all_permissions_is_rejected(pool: PgPool) {
    let admin = Admin::new(&pool).await;
    let too_many = vec!["patient.view"; 1000];

    let response = send(
        &admin.app,
        Method::POST,
        "/api/roles",
        Some(&admin.cookie),
        Some(json!({ "name": "flood", "permissions": too_many })),
    )
    .await;
    assert_eq!(
        (response.status, response.body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("too many permissions"))
    );

    let (status, body) = admin
        .patch_role("nurse", json!({ "permissions": too_many }))
        .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("too many permissions"))
    );
}
