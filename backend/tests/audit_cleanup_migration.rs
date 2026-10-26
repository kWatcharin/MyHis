//! migration `20261021100300_clear_unknown_name_login_failed_target_ids.sql`: ล้าง `audit_log.target_id` ของ
//! `login_failed` ที่ไม่มีผู้กระทำ (ชื่อที่พิมพ์ไม่มีในระบบ — แถวเก่าเก็บเป็นชื่อดิบหรือ SHA-256 ของชื่อ)
//! ข้อยกเว้นครั้งเดียวของ "audit_log เขียนอย่างเดียว" (docs/auth-design.md ข้อ 4) จึงต้องแตะเฉพาะแถวกลุ่มนั้นจริง ๆ
//!
//! migrate ถึงก่อนไฟล์นี้ → ใส่แถวรูปแบบเก่า → รันไฟล์นี้ → ตรวจ (เทสแบบ pages.rs ที่ใช้ `run_migration_files`)

mod common;

use common::run_migration_files;
use serde_json::{json, Value};
use sqlx::PgPool;

const CLEANUP: i64 = 20261021100300;

async fn put_row(
    pool: &PgPool,
    action: &str,
    actor: Option<i32>,
    target: Option<&str>,
    metadata: Option<Value>,
    ip: Option<&str>,
) -> i64 {
    sqlx::query_scalar(
        "insert into audit_log (actor_user_id, action, target_type, target_id, metadata, ip)
         values ($1, $2, 'user', $3, $4, ($5::text)::inet) returning id",
    )
    .bind(actor)
    .bind(action)
    .bind(target)
    .bind(metadata)
    .bind(ip)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn targets(pool: &PgPool, ids: &[i64]) -> Vec<(i64, Option<String>)> {
    sqlx::query_as("select id, target_id from audit_log where id = any($1) order by id")
        .bind(ids)
        .fetch_all(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
async fn cleanup_touches_only_unknown_name_login_failed_rows(pool: PgPool) {
    run_migration_files(&pool, |version| version < CLEANUP).await;
    let (alice,): (i32,) = sqlx::query_as(
        "insert into users (username, display_name, password_hash) values ('alice', 'Alice', 'x')
         returning id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let meta = Some(json!({ "userAgent": "UA-1" }));
    let raw = put_row(
        &pool,
        "login_failed",
        None,
        Some("  Hunter2-Typed "),
        meta.clone(),
        Some("203.0.113.9"),
    )
    .await;
    let hashed = put_row(
        &pool,
        "login_failed",
        None,
        Some("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"),
        meta.clone(),
        Some("203.0.113.9"),
    )
    .await;
    let already_null = put_row(&pool, "login_failed", None, None, meta.clone(), None).await;
    let real_account = put_row(
        &pool,
        "login_failed",
        Some(alice),
        Some("alice"),
        meta.clone(),
        None,
    )
    .await;
    let succeeded = put_row(
        &pool,
        "login_succeeded",
        Some(alice),
        Some("alice"),
        meta.clone(),
        None,
    )
    .await;
    let other_action_no_actor =
        put_row(&pool, "login_succeeded", None, Some("ghost"), None, None).await;
    let unrelated = put_row(
        &pool,
        "patient_created",
        None,
        Some("0198-patient-uuid"),
        None,
        None,
    )
    .await;
    let ids = [
        raw,
        hashed,
        already_null,
        real_account,
        succeeded,
        other_action_no_actor,
        unrelated,
    ];

    // เฉพาะ login_failed ที่ไม่มีผู้กระทำและยังมี target_id ถูกล้าง — บัญชีจริง (target_id คือ username) และ action อื่นอยู่ครบ
    let expected = vec![
        (raw, None),
        (hashed, None),
        (already_null, None),
        (real_account, Some("alice".to_string())),
        (succeeded, Some("alice".to_string())),
        (other_action_no_actor, Some("ghost".to_string())),
        (unrelated, Some("0198-patient-uuid".to_string())),
    ];

    run_migration_files(&pool, |version| version == CLEANUP).await;
    assert_eq!(targets(&pool, &ids).await, expected);

    // ล้างแค่ target_id: metadata และ ip ของแถวที่ถูกล้างยังอยู่
    let (user_agent, ip): (Option<String>, Option<String>) =
        sqlx::query_as("select metadata ->> 'userAgent', host(ip) from audit_log where id = $1")
            .bind(raw)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(user_agent.as_deref(), Some("UA-1"));
    assert_eq!(ip.as_deref(), Some("203.0.113.9"));

    // จำนวนแถวไม่เปลี่ยน (ไม่ลบแถว)
    let (count,): (i64,) = sqlx::query_as("select count(*) from audit_log where id = any($1)")
        .bind(&ids[..])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, ids.len() as i64);

    // รันซ้ำแล้วผลเท่าเดิม
    run_migration_files(&pool, |version| version == CLEANUP).await;
    assert_eq!(targets(&pool, &ids).await, expected);
}
