//! helper ที่ใช้ร่วมกันในเทสบน DB สะอาด (`#[sqlx::test]`):
//! - ระดับ service — สร้างผู้กระทำ (`CurrentUser`) ตรง ๆ โดยไม่ผ่าน HTTP/cookie และแถวตัวอย่างของตารางที่
//!   master อื่นอ้างอิง
//! - ระดับ router — `test_app` ประกอบ router จริงของแอป (ทุกโมดูล + extractor/cookie) แล้วยิงด้วย
//!   `tower::ServiceExt::oneshot` (`send`, `login`) สำหรับพฤติกรรมที่ต้องผ่าน session จริง
//!
//! (แต่ละไฟล์ใน tests/ เป็นคนละ crate จึงมี helper ที่บางไฟล์ไม่ได้ใช้ — ปิด dead_code ไว้)
#![allow(dead_code)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use my_his_backend::auth::CurrentUser;
use my_his_backend::config::Config;
use my_his_backend::crypto::Crypto;
use my_his_backend::domain::{Permission, Resource, User, UserId};
use my_his_backend::state::AppState;
use my_his_backend::{app, password};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

/// key ทดสอบ (hex 32 ไบต์) — ใช้กับ Config ของ `test_app` เท่านั้น ไม่ใช่ key จริง
const TEST_DATA_KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const TEST_INDEX_KEY: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

/// argon2 ห้ามรันตรง ๆ ใน async context (CLAUDE.md) — เทสก็ห่อ spawn_blocking เหมือนโค้ดจริง
pub async fn hash_password(plain: &str) -> String {
    let plain = plain.to_string();
    tokio::task::spawn_blocking(move || password::hash(&plain))
        .await
        .unwrap()
        .unwrap()
}

pub async fn verify_password(plain: &str, hash: &str) -> bool {
    let (plain, hash) = (plain.to_string(), hash.to_string());
    tokio::task::spawn_blocking(move || password::verify(&plain, &hash))
        .await
        .unwrap()
        .unwrap()
}

/// รัน migration ทีละไฟล์ (เรียงตามเวอร์ชันในชื่อไฟล์) เฉพาะเวอร์ชันที่ `include` ตอบ true — ใช้กับ
/// `#[sqlx::test(migrations = false)]` เพื่อใส่ข้อมูลแบบที่ admin ตั้งผ่านหน้าเว็บก่อน migration ที่ทดสอบ
pub async fn run_migration_files(pool: &PgPool, include: impl Fn(i64) -> bool) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/migrations");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    files.sort();
    for path in files {
        let name = path.file_name().unwrap().to_str().unwrap();
        let version: i64 = name.split('_').next().unwrap().parse().unwrap();
        if include(version) {
            let sql = std::fs::read_to_string(&path).unwrap();
            let mut tx = pool.begin().await.unwrap();
            sqlx::raw_sql(&sql).execute(&mut *tx).await.unwrap();
            tx.commit().await.unwrap();
        }
    }
}

/// router จริงของแอปบน DB ของเทส — Config ใช้ค่าทดสอบ (ไม่อ่าน environment variable)
pub async fn test_app(pool: PgPool) -> Router {
    let config = Config {
        database_url: String::new(),
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        session_idle_ttl: chrono::Duration::hours(8),
        session_absolute_ttl: chrono::Duration::days(30),
        initial_admin_username: "unused".to_string(),
        initial_admin_password: "unused".to_string(),
        crypto: Crypto::from_hex_keys(TEST_DATA_KEY, TEST_INDEX_KEY).unwrap(),
    };
    let dummy_password_hash = tokio::task::spawn_blocking(password::dummy_hash)
        .await
        .unwrap()
        .unwrap();
    app::router(AppState {
        pool,
        config: Arc::new(config),
        dummy_password_hash: Arc::from(dummy_password_hash),
    })
}

/// ผู้ใช้จริงใน DB พร้อมรหัสผ่าน (hash จริง) และ role ที่ seed ไว้ (`admin` / `nurse`) — สำหรับเทสที่
/// login ผ่าน HTTP คืน (id ภายใน, uuid)
pub async fn insert_user_with_password(
    pool: &PgPool,
    username: &str,
    plain_password: &str,
    role: &str,
) -> (i32, Uuid) {
    let password_hash = hash_password(plain_password).await;
    let row: (i32, Uuid) = sqlx::query_as(
        "insert into users (username, display_name, password_hash)
         values ($1, $1, $2) returning id, uuid",
    )
    .bind(username)
    .bind(password_hash)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into user_roles (user_id, role_id) select $1, id from roles where name = $2",
    )
    .bind(row.0)
    .bind(role)
    .execute(pool)
    .await
    .unwrap();
    row
}

pub struct TestResponse {
    pub status: StatusCode,
    pub body: Value,
    /// ค่า "ชื่อ=ค่า" ของ Set-Cookie (ตัด attribute ทิ้ง) ไว้แนบเป็น Cookie ของ request ถัดไป
    pub set_cookie: Option<String>,
    /// header Retry-After (วินาที) — มีเฉพาะตอนถูกล็อก (429)
    pub retry_after: Option<i64>,
}

/// ยิง request หนึ่งครั้งเข้า router (JSON body ถ้ามี + cookie session ถ้ามี)
pub async fn send(
    app: &Router,
    method: Method,
    path: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> TestResponse {
    send_with_headers(app, method, path, cookie, body, &[]).await
}

/// เหมือน `send` แต่แนบ header เพิ่ม เช่น X-Forwarded-For / User-Agent (ประวัติการเข้าออก)
pub async fn send_with_headers(
    app: &Router,
    method: Method,
    path: &str,
    cookie: Option<&str>,
    body: Option<Value>,
    headers: &[(&str, &str)],
) -> TestResponse {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(cookie) = cookie {
        builder = builder.header(header::COOKIE, cookie);
    }
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    let request = match body {
        Some(json) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };

    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let set_cookie = response
        .headers()
        .get(header::SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::to_string);
    let retry_after = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    TestResponse {
        status,
        body,
        set_cookie,
        retry_after,
    }
}

/// login ผ่าน POST /api/auth/login — คืน cookie session ไว้แนบ request ถัดไป และ body (`mustChangePassword` ฯลฯ)
pub async fn login(app: &Router, username: &str, plain_password: &str) -> (String, Value) {
    let response = send(
        app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": username, "password": plain_password })),
    )
    .await;
    assert_eq!(response.status, StatusCode::OK, "login {username}");
    let cookie = response
        .set_cookie
        .expect("login must set the session cookie");
    (cookie, response.body)
}

pub async fn actor_with(pool: &PgPool, permissions: Vec<Permission>) -> CurrentUser {
    let row: (i32, Uuid) = sqlx::query_as(
        "insert into users (username, display_name, password_hash)
         values ($1, 'Tester', 'x') returning id, uuid",
    )
    .bind(format!("tester-{}", Uuid::new_v4()))
    .fetch_one(pool)
    .await
    .unwrap();
    CurrentUser {
        user: User {
            id: UserId(row.0),
            public_id: row.1,
            username: "tester".to_string(),
            display_name: "Tester".to_string(),
            must_change_password: false,
        },
        permissions,
    }
}

/// permission จากคีย์ เช่น "patient.view" (คีย์ผิด = เทสพังทันที)
pub fn perm(key: &str) -> Permission {
    Permission::parse(key).unwrap_or_else(|| panic!("unknown permission {key}"))
}

/// ทุก action ของเพจหนึ่ง เช่น "patient" → patient.view/create/update/delete
pub fn all_of(resource: Resource) -> Vec<Permission> {
    Permission::all()
        .filter(|p| p.resource() == resource)
        .collect()
}

/// ทุกสิทธิ์ยกเว้นของเพจหนึ่ง — ใช้เช็คว่าสิทธิ์ของเพจอื่น (แม้ครบทุกตัว) ไม่ให้เข้าเพจนี้
pub fn all_except(resource: Resource) -> Vec<Permission> {
    Permission::all()
        .filter(|p| p.resource() != resource)
        .collect()
}

/// ผู้ใช้ที่มีทุกสิทธิ์ (เหมือน role admin หลัง migration สิทธิ์รายเพจ)
pub async fn admin(pool: &PgPool) -> CurrentUser {
    actor_with(pool, Permission::all().collect()).await
}

/// ผู้ใช้ที่ไม่มีสิทธิ์หน้าตั้งค่า/ผู้ป่วยใด ๆ (มีแค่ตารางเวร)
pub async fn nurse(pool: &PgPool) -> CurrentUser {
    actor_with(pool, vec![perm("shift.view"), perm("shift.update")]).await
}

pub async fn insert_prename(pool: &PgPool, user_id: i32, name: &str) -> i32 {
    sqlx::query_scalar(
        "insert into prename (name, created_by, updated_by) values ($1, $2, $2) returning id",
    )
    .bind(name)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

pub async fn insert_nationality(pool: &PgPool, user_id: i32, national_name: &str) -> i32 {
    sqlx::query_scalar(
        "insert into nationality (national_name, nation, created_by, updated_by)
         values ($1, $1, $2, $2) returning id",
    )
    .bind(national_name)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// คนไข้ตัวอย่าง — คอลัมน์ที่เข้ารหัสใส่ bytea ปลอม (เทสนี้ไม่เกี่ยวกับการเข้ารหัส)
pub async fn insert_patient(
    pool: &PgPool,
    user_id: i32,
    prename_id: i32,
    nationality_id: Option<i32>,
) -> i32 {
    sqlx::query_scalar(
        "insert into patient (hn, hn_idx, nap_no, nap_no_idx, prename_id, first_name, last_name,
                              sex, is_thai, nationality_id, created_by, updated_by)
         values ($1, $1, $1, $1, $2, $1, $1, 1, true, $3, $4, $4) returning id",
    )
    .bind(vec![1u8, 2, 3])
    .bind(prename_id)
    .bind(nationality_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// soft delete แถวใด ๆ ด้วย id ภายใน (ใช้ปลดการอ้างอิงก่อนทดสอบว่าลบ master ได้)
pub async fn soft_delete_row(pool: &PgPool, table: &str, id: i32, user_id: i32) {
    // table มาจากเทสเท่านั้น (ค่าคงที่) ไม่ใช่ input ผู้ใช้
    let sql = format!("update {table} set deleted_at = now(), deleted_by = $2 where id = $1");
    sqlx::query(&sql)
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
}

/// เพจที่ migration seed ไว้: (ชื่อเพจ, ชื่อเพจหลักที่อยู่ใต้ — None = เป็นเพจหลักเอง, permission)
pub async fn seeded_page(pool: &PgPool, route: &str) -> (String, Option<String>, String) {
    sqlx::query_as(
        "select p.label, m.label, pp.permission
         from pages p
         join page_permissions pp on pp.page_id = p.id
         left join pages m on m.id = p.ref_id
         where p.route = $1",
    )
    .bind(route)
    .fetch_one(pool)
    .await
    .unwrap()
}
