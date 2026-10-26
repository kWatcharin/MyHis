//! ด่าน same-origin ของคำขอที่เปลี่ยนสถานะ (app.rs `reject_cross_site`, docs/auth-design.md ข้อ 7): POST ที่ไม่มี body
//! (reset-password, logout, logout-all) ไม่ถูก Json extractor ปฏิเสธเหมือน POST อื่น จึงใช้ form จาก origin อื่นใน site
//! เดียวกันยิงได้ถ้าพึ่ง `SameSite=Lax` อย่างเดียว — เบราว์เซอร์ตั้ง Sec-Fetch-Site / Origin เอง หน้าเว็บอื่นปลอมไม่ได้
mod common;

use axum::http::{Method, StatusCode};
use common::{insert_user_with_password, login, send, send_with_headers, test_app};
use sqlx::PgPool;

const ADMIN_PASSWORD: &str = "admin-dummy-password-1";
const VICTIM_PASSWORD: &str = "victim-dummy-password-1";
const HOST: &str = "his.example.com";

async fn sessions_of(pool: &PgPool, username: &str) -> i64 {
    sqlx::query_scalar(
        "select count(*) from sessions s join users u on u.id = s.user_id where u.username = $1",
    )
    .bind(username)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[sqlx::test]
async fn cross_origin_requests_without_a_body_change_nothing(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (_, victim_uuid) =
        insert_user_with_password(&pool, "victim.one", VICTIM_PASSWORD, "nurse").await;
    let (admin_cookie, _) = login(&app, "admin.one", ADMIN_PASSWORD).await;
    login(&app, "victim.one", VICTIM_PASSWORD).await;
    let reset_path = format!("/api/users/{victim_uuid}/reset-password");

    let hostile: [&[(&str, &str)]; 5] = [
        // เบราว์เซอร์ใหม่: form จาก sibling subdomain (same-site) และจากเว็บอื่น (cross-site)
        &[
            ("sec-fetch-site", "same-site"),
            ("origin", "https://sibling.example.com"),
            ("host", HOST),
        ],
        &[
            ("sec-fetch-site", "cross-site"),
            ("origin", "https://evil.test"),
            ("host", HOST),
        ],
        // ไม่มี Sec-Fetch-Site: ตกไปเทียบ Origin กับ Host
        &[("origin", "https://sibling.example.com"), ("host", HOST)],
        &[("origin", "null"), ("host", HOST)],
        &[
            ("origin", "http://his.example.com"),
            ("host", "his.example.com:8443"),
        ],
    ];
    for headers in hostile {
        for path in [
            reset_path.as_str(),
            "/api/auth/logout-all",
            "/api/auth/logout",
        ] {
            let response =
                send_with_headers(&app, Method::POST, path, Some(&admin_cookie), None, headers)
                    .await;
            assert_eq!(response.status, StatusCode::FORBIDDEN, "{path} {headers:?}");
        }
    }
    // ไม่มีอะไรเปลี่ยน: session ของทั้งสองคนยังอยู่ และ session ของแอดมินยังใช้ได้
    assert_eq!(sessions_of(&pool, "victim.one").await, 1);
    assert_eq!(sessions_of(&pool, "admin.one").await, 1);
    let me = send(&app, Method::GET, "/api/auth/me", Some(&admin_cookie), None).await;
    assert_eq!(me.status, StatusCode::OK);

    // origin เดียวกัน (ไม่มี Sec-Fetch-Site: Origin ตรง Host) ผ่าน
    let reset = send_with_headers(
        &app,
        Method::POST,
        &reset_path,
        Some(&admin_cookie),
        None,
        &[("origin", "https://his.example.com"), ("host", HOST)],
    )
    .await;
    assert_eq!(reset.status, StatusCode::OK);
    assert_eq!(sessions_of(&pool, "victim.one").await, 0);

    // ผู้ใช้กดเอง (Sec-Fetch-Site: none) ผ่าน และไม่แนบ header ของเบราว์เซอร์เลย (curl/สคริปต์) ก็ผ่าน
    let logout_all = send_with_headers(
        &app,
        Method::POST,
        "/api/auth/logout-all",
        Some(&admin_cookie),
        None,
        &[("sec-fetch-site", "none")],
    )
    .await;
    assert_eq!(logout_all.status, StatusCode::OK);
    let me = send(&app, Method::GET, "/api/auth/me", Some(&admin_cookie), None).await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);

    let (cookie, _) = login(&app, "admin.one", ADMIN_PASSWORD).await;
    let plain = send(&app, Method::POST, "/api/auth/logout", Some(&cookie), None).await;
    assert_eq!(plain.status, StatusCode::OK);
}

/// คำขออ่าน (GET) ไม่ถูกด่านนี้แตะ แม้มาจาก origin อื่น — การเปลี่ยนสถานะเท่านั้นที่ตรวจ
#[sqlx::test]
async fn reads_are_not_blocked_by_the_origin_check(pool: PgPool) {
    let app = test_app(pool.clone()).await;
    insert_user_with_password(&pool, "admin.one", ADMIN_PASSWORD, "admin").await;
    let (cookie, _) = login(&app, "admin.one", ADMIN_PASSWORD).await;
    let response = send_with_headers(
        &app,
        Method::GET,
        "/api/auth/me",
        Some(&cookie),
        None,
        &[
            ("sec-fetch-site", "cross-site"),
            ("origin", "https://evil.test"),
            ("host", HOST),
        ],
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
}
