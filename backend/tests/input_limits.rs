//! ขอบเขตของคำขอ: body ใหญ่เกินเพดานถูกปฏิเสธ (413) และอาร์เรย์ id ที่ยาวเกินเพดานถูกปฏิเสธก่อนทำงานใด ๆ ที่โตตามความยาว
//! (เดิม body ปริยาย 2 MiB ใส่ uuid ได้หลายหมื่นตัว และการตัดซ้ำด้วย Vec::contains เป็น O(n²) บน async worker)
mod common;

use axum::http::{Method, StatusCode};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

const PASSWORD: &str = "limits-test-password-1";

#[sqlx::test]
async fn oversized_bodies_are_rejected_with_413(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    // 300 KiB ของข้อความในฟิลด์เดียว (เพดานคือ 256 KiB) — ไม่ต้อง login เพราะ body ถูกปฏิเสธก่อนถึง handler
    let response = common::send(
        &app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": "x".repeat(300 * 1024), "password": "y" })),
    )
    .await;
    assert_eq!(response.status, StatusCode::PAYLOAD_TOO_LARGE);

    // ขนาดปกติของคำขอจริงยังผ่าน (ถึง handler แล้วตอบ 401 เพราะไม่มีบัญชีนี้)
    let normal = common::send(
        &app,
        Method::POST,
        "/api/auth/login",
        None,
        Some(json!({ "username": "nobody", "password": "y".repeat(1000) })),
    )
    .await;
    assert_eq!(normal.status, StatusCode::UNAUTHORIZED);
}

fn ids(count: usize) -> Vec<Uuid> {
    (0..count).map(|_| Uuid::new_v4()).collect()
}

#[sqlx::test]
async fn id_arrays_over_the_cap_are_rejected_before_any_lookup(pool: PgPool) {
    let app = common::test_app(pool.clone()).await;
    common::insert_user_with_password(&pool, "admin.limits", PASSWORD, "admin").await;
    let (cookie, _) = common::login(&app, "admin.limits", PASSWORD).await;

    // นัดหมาย: 501 id (เพดาน 500) → 400 โดยไม่ต้องมีกลุ่มนัดหมายอยู่จริง
    let visits = common::send(
        &app,
        Method::POST,
        &format!("/api/lab-visit-groups/{}/visits", Uuid::new_v4()),
        Some(&cookie),
        Some(json!({ "patientIds": ids(501) })),
    )
    .await;
    assert_eq!(visits.status, StatusCode::BAD_REQUEST, "{}", visits.body);
    assert_eq!(visits.body["error"], json!("too many patients"));

    // role ของผู้ใช้: 101 id (เพดาน 100) → 400 โดยไม่ต้องมีผู้ใช้อยู่จริง
    let roles = common::send(
        &app,
        Method::PUT,
        &format!("/api/users/{}/roles", Uuid::new_v4()),
        Some(&cookie),
        Some(json!({ "roleIds": ids(101) })),
    )
    .await;
    assert_eq!(roles.status, StatusCode::BAD_REQUEST, "{}", roles.body);
    assert_eq!(roles.body["error"], json!("too many roles"));
}
