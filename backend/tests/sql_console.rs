//! หน้า SQL (`/api/sql/*`, sql_console/): รัน SQL อ่านอย่างเดียวด้วย role ฐานข้อมูลแยก แล้วถอดรหัสคอลัมน์ผู้ป่วย
//!
//! แต่ละเทสสร้าง role ของตัวเอง (ชื่อสุ่ม) ด้วย `sql_console::setup` ตัวจริง แล้วลบทิ้งตอนจบ — role อยู่ระดับ cluster
//! ถ้าเทสล้มกลางทาง role จะค้าง (ไม่มีสิทธิ์อะไรเหลือเพราะฐานข้อมูลของเทสถูกลบไปแล้ว) ลบได้ด้วย DROP ROLE

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::http::{Method, StatusCode};
use axum::Router;
use common::{
    insert_prename, insert_user_with_password, login, send, test_app, test_app_with_sql_console,
    test_crypto, TestResponse,
};
use my_his_backend::sql_console::{self, SetupError, SqlConsole};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const PASSWORD: &str = "sql-console-test-password";
const HN: &str = "66471";
const NAP_NO: &str = "NAP-2026-1";

struct Reader {
    role: String,
    console: Arc<SqlConsole>,
}

/// role อ่านอย่างเดียวของเทสนี้ — timeout สั้นให้เทส pg_sleep ไม่ต้องรอนาน
async fn reader(pool: &PgPool) -> Reader {
    let role = format!("my_his_sqlt_{}", &Uuid::new_v4().simple().to_string()[..12]);
    let password = Uuid::new_v4().simple().to_string();
    let console = SqlConsole::new(pool.connect_options().as_ref(), &role, &password)
        .unwrap()
        .with_statement_timeout(Duration::from_millis(1500));
    sql_console::setup(pool, &console).await.unwrap();
    Reader {
        role,
        console: Arc::new(console),
    }
}

/// ถอนสิทธิ์ในฐานข้อมูลของเทส (DROP OWNED) แล้วลบ role — ชื่อมาจากเทสเอง (ตัวพิมพ์เล็ก/ตัวเลข) ไม่ใช่ input
async fn drop_role(pool: &PgPool, role: &str) {
    sqlx::query(&format!("drop owned by {role}"))
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(&format!("drop role {role}"))
        .execute(pool)
        .await
        .unwrap();
}

/// ผู้ใช้ที่มีเฉพาะสิทธิ์ `keys` แล้ว login คืน cookie
async fn session_with(pool: &PgPool, app: &Router, keys: &[&str]) -> String {
    let role = format!("sql-test-{}", Uuid::new_v4());
    let keys: Vec<String> = keys.iter().map(|k| k.to_string()).collect();
    sqlx::query(
        "with r as (insert into roles (name) values ($1) returning id)
         insert into role_permissions (role_id, permission) select r.id, unnest($2::text[]) from r",
    )
    .bind(&role)
    .bind(&keys)
    .execute(pool)
    .await
    .unwrap();
    let username = format!("sql.{}", &Uuid::new_v4().simple().to_string()[..10]);
    insert_user_with_password(pool, &username, PASSWORD, &role).await;
    login(app, &username, PASSWORD).await.0
}

/// คนไข้หนึ่งคนที่เข้ารหัสด้วย key ชุดเดียวกับแอปของเทส
async fn insert_encrypted_patient(pool: &PgPool) {
    let crypto = test_crypto();
    let (user_id,): (i32,) = sqlx::query_as(
        "insert into users (username, display_name, password_hash) values ('seed.user', 'Seed', 'x')
         returning id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let prename_id = insert_prename(pool, user_id, "นาย").await;
    sqlx::query(
        "insert into patient (hn, hn_idx, nap_no, nap_no_idx, prename_id, first_name, last_name,
                              sex, is_thai, created_by, updated_by)
         values ($1, $2, $3, $4, $5, $6, $7, 1, true, $8, $8)",
    )
    .bind(crypto.encrypt("patient.hn", HN).unwrap())
    .bind(crypto.blind_index("patient.hn", HN))
    .bind(crypto.encrypt("patient.nap_no", NAP_NO).unwrap())
    .bind(crypto.blind_index("patient.nap_no", NAP_NO))
    .bind(prename_id)
    .bind(crypto.encrypt("patient.first_name", "สมชาย").unwrap())
    .bind(crypto.encrypt("patient.last_name", "ใจดี").unwrap())
    .bind(user_id)
    .execute(pool)
    .await
    .unwrap();
}

async fn run(app: &Router, cookie: &str, sql: &str, decrypt: bool) -> TestResponse {
    send(
        app,
        Method::POST,
        "/api/sql/query",
        Some(cookie),
        Some(json!({ "sql": sql, "decrypt": decrypt })),
    )
    .await
}

/// ค่าแถวแรกของคอลัมน์ชื่อ `name`
fn first(body: &Value, name: &str) -> Value {
    let columns = body["columns"].as_array().unwrap();
    let index = columns
        .iter()
        .position(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no column {name}: {body}"));
    body["rows"][0][index].clone()
}

fn column<'a>(body: &'a Value, name: &str) -> &'a Value {
    body["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap()
}

#[sqlx::test]
async fn needs_sql_view_and_a_configured_console(pool: PgPool) {
    let reader = reader(&pool).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    // สิทธิ์ผู้ป่วยครบก็ไม่พอ ต้องมี sql.view
    let no_sql = session_with(&pool, &app, &["patient.view", "audit.view"]).await;
    assert_eq!(
        run(&app, &no_sql, "select 1", false).await.status,
        StatusCode::FORBIDDEN
    );
    let schema = send(&app, Method::GET, "/api/sql/schema", Some(&no_sql), None).await;
    assert_eq!(schema.status, StatusCode::FORBIDDEN);

    // ปิดหน้า SQL (ไม่ได้ตั้งรหัสของ role) = 503
    let disabled = test_app(pool.clone()).await;
    let cookie = session_with(&pool, &disabled, &["sql.view"]).await;
    let response = run(&disabled, &cookie, "select 1", false).await;
    assert_eq!(response.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response.body,
        json!({ "error": "sql console not configured" })
    );

    drop_role(&pool, &reader.role).await;
}

#[sqlx::test]
async fn patient_columns_are_decrypted_by_origin_not_by_name(pool: PgPool) {
    insert_encrypted_patient(&pool).await;
    let reader = reader(&pool).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    let cookie = session_with(&pool, &app, &["sql.view", "patient.view"]).await;

    let sql = "select p.hn as x, first_name, p.last_name as hn, hn_idx, encode(p.hn, 'hex') as e
               from patient p";
    let response = run(&app, &cookie, sql, true).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    let body = &response.body;
    assert_eq!(first(body, "x"), json!(HN));
    assert_eq!(first(body, "first_name"), json!("สมชาย"));
    // alias ชื่อ "hn" แต่มาจาก last_name → ถอดด้วย AAD ของ last_name
    assert_eq!(first(body, "hn"), json!("ใจดี"));
    assert_eq!(column(body, "x")["decrypted"], json!(true));
    // blind index และ expression ไม่ถอด
    assert!(first(body, "hn_idx").as_str().unwrap().starts_with("\\x"));
    assert_eq!(column(body, "hn_idx")["encrypted"], json!(false));
    assert_eq!(column(body, "e")["encrypted"], json!(false));
    assert_eq!(body["decryptErrors"], json!(0));

    // ไม่ขอถอด → ciphertext เป็น hex (ไบต์แรกคือเวอร์ชัน 01)
    let raw = run(&app, &cookie, sql, false).await;
    assert!(first(&raw.body, "x").as_str().unwrap().starts_with("\\x01"));
    assert_eq!(column(&raw.body, "x")["encrypted"], json!(true));
    assert_eq!(column(&raw.body, "x")["decrypted"], json!(false));

    // ขอถอดแต่ไม่มีสิทธิ์ดูผู้ป่วย → hex เหมือนกัน
    let no_patient = session_with(&pool, &app, &["sql.view"]).await;
    let denied = run(&app, &no_patient, sql, true).await;
    assert!(first(&denied.body, "x")
        .as_str()
        .unwrap()
        .starts_with("\\x01"));
    assert!(!denied.body.to_string().contains("สมชาย"));

    drop_role(&pool, &reader.role).await;
}

#[sqlx::test]
async fn helpers_search_encrypted_ids_through_the_blind_index(pool: PgPool) {
    insert_encrypted_patient(&pool).await;
    let reader = reader(&pool).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    let cookie = session_with(&pool, &app, &["sql.view", "patient.view"]).await;

    for sql in [
        "select first_name from patient where hn_idx = :hn('66471')",
        "select first_name from patient where hn_idx = :hn(' 66471 ');",
        "select first_name from patient where nap_no_idx = :nap_no('NAP-2026-1')",
    ] {
        let response = run(&app, &cookie, sql, true).await;
        assert_eq!(
            response.body["rowCount"],
            json!(1),
            "{sql}: {}",
            response.body
        );
        assert_eq!(first(&response.body, "first_name"), json!("สมชาย"));
    }
    let none = run(
        &app,
        &cookie,
        "select 1 from patient where hn_idx = :hn('99999')",
        false,
    )
    .await;
    assert_eq!(none.body["rowCount"], json!(0));

    let malformed = run(&app, &cookie, "select 1 where :hn(66471)", false).await;
    assert_eq!(malformed.status, StatusCode::BAD_REQUEST);
    assert!(!malformed.body.to_string().contains("66471"));

    drop_role(&pool, &reader.role).await;
}

#[sqlx::test]
async fn secrets_and_unlisted_tables_are_not_readable(pool: PgPool) {
    let reader = reader(&pool).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    let cookie = session_with(&pool, &app, &["sql.view"]).await;

    for sql in [
        "select password_hash from users",
        "select * from users",
        "select * from sessions",
        "select * from login_throttle",
        "select * from _sqlx_migrations",
        "select token_hash from registration_links",
    ] {
        let response = run(&app, &cookie, sql, false).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{sql}");
        assert_eq!(response.body["error"], json!("query failed"));
        assert_eq!(
            response.body["code"],
            json!("42501"),
            "{sql}: {}",
            response.body
        );
    }
    let allowed = run(
        &app,
        &cookie,
        "select username, display_name from users",
        false,
    )
    .await;
    assert_eq!(allowed.status, StatusCode::OK, "{}", allowed.body);

    drop_role(&pool, &reader.role).await;
}

#[sqlx::test]
async fn writes_and_privilege_escapes_are_rejected(pool: PgPool) {
    let reader = reader(&pool).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    let cookie = session_with(&pool, &app, &["sql.view"]).await;
    let audit_before: i64 =
        sqlx::query_scalar("select count(*) from audit_log where action <> 'sql_executed'")
            .fetch_one(&pool)
            .await
            .unwrap();

    // คำสั่งที่ไม่ใช่การอ่าน: ตรวจจากคำแรกก่อนรัน หรือ Postgres ตอบว่าแก้ข้อมูลใน read only ไม่ได้ → 400 พร้อมเหตุผล
    // ให้หน้าเว็บเตือน (ไม่ใช่ syntax error จากการครอบ LIMIT) — ส่วนที่เป็นการอ่านแต่ไม่มีสิทธิ์ยังเป็น error ของ Postgres
    let read_only = |keyword: &str| json!({ "error": "statement not allowed", "reason": "read_only", "keyword": keyword });
    let read_only_cte = json!({ "error": "statement not allowed", "reason": "read_only" });
    let several = json!({ "error": "statement not allowed", "reason": "multiple_statements" });
    let cases = [
        (
            "insert into prename (name, created_by, updated_by) values ('x', 1, 1)",
            read_only("INSERT"),
        ),
        ("update users set display_name = 'x'", read_only("UPDATE")),
        ("  -- ลบ\n delete from audit_log;", read_only("DELETE")),
        ("create table hacked (id int)", read_only("CREATE")),
        ("copy (select 1) to program 'id'", read_only("COPY")),
        ("set role postgres", read_only("SET")),
        ("do $$ begin end $$", read_only("DO")),
        // ผ่านคำแรกแต่ Postgres บอกว่าแก้ข้อมูล
        (
            "with gone as (delete from audit_log returning 1) select * from gone",
            read_only_cte,
        ),
        // explain รันโดยไม่ครอบ LIMIT — analyze ทำคำสั่งจริง ต้องถูก read only ตัด
        ("explain analyze delete from audit_log", read_only("DELETE")),
        (
            "select * from prename for update",
            read_only("SELECT FOR UPDATE"),
        ),
        (
            "select * into hacked from prename",
            read_only("SELECT INTO"),
        ),
        ("explain select 1; delete from audit_log", several.clone()),
        ("select 1; select 2", several),
    ];
    for (sql, expected) in cases {
        let response = run(&app, &cookie, sql, false).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{sql}");
        assert_eq!(response.body, expected, "{sql}");
    }
    // อ่านอย่างเดียวแต่ไม่มีสิทธิ์ = error ของ Postgres ตามเดิม (สลับ role: 22023 ไม่มี role นั้น / 42501 ไม่มีสิทธิ์)
    for (sql, codes) in [
        ("select pg_read_file('/etc/passwd')", &["42501"][..]),
        (
            "select set_config('role', 'postgres', false)",
            &["22023", "42501"][..],
        ),
    ] {
        let response = run(&app, &cookie, sql, false).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST, "{sql}");
        assert_eq!(response.body["error"], json!("query failed"), "{sql}");
        let code = response.body["code"].as_str().unwrap_or_default();
        assert!(codes.contains(&code), "{sql}: {code}");
    }

    // ตัว role เอง (ไม่ขึ้นกับ wrapper ของ backend): ไม่ใช่ superuser และไม่ได้เป็นสมาชิก role ใด
    let (superuser, member): (bool, bool) = sqlx::query_as(
        "select r.rolsuper, exists(select 1 from pg_auth_members m where m.member = r.oid)
         from pg_roles r where r.rolname = $1",
    )
    .bind(&reader.role)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!superuser && !member);

    let audit_after: i64 =
        sqlx::query_scalar("select count(*) from audit_log where action <> 'sql_executed'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audit_before, audit_after);
    let prenames: i64 = sqlx::query_scalar("select count(*) from prename")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(prenames, 0);
    let hacked: Option<String> = sqlx::query_scalar("select to_regclass('public.hacked')::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(hacked, None);

    drop_role(&pool, &reader.role).await;
}

#[sqlx::test]
async fn slow_queries_are_cancelled_and_large_results_capped(pool: PgPool) {
    let reader = reader(&pool).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    let cookie = session_with(&pool, &app, &["sql.view"]).await;

    let slow = run(&app, &cookie, "select pg_sleep(5)", false).await;
    assert_eq!(slow.status, StatusCode::BAD_REQUEST);
    assert_eq!(slow.body["code"], json!("57014"), "{}", slow.body);

    let many = run(
        &app,
        &cookie,
        "select g from generate_series(1, 1000) g order by g",
        false,
    )
    .await;
    assert_eq!(many.body["rowCount"], json!(sql_console::MAX_ROWS));
    assert_eq!(many.body["truncated"], json!(true));
    assert_eq!(many.body["rows"][0][0], json!(1));

    let few = run(&app, &cookie, "select 1 as a", false).await;
    assert_eq!(few.body["truncated"], json!(false));

    drop_role(&pool, &reader.role).await;
}

#[sqlx::test]
async fn values_decode_to_readable_json(pool: PgPool) {
    let reader = reader(&pool).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    let cookie = session_with(&pool, &app, &["sql.view"]).await;

    let sql = "select 123.45::numeric as a, -0.001::numeric as b, 1e20::numeric as c,
                      'NaN'::numeric as d, 0.50::numeric(5,2) as w, '2026-10-12'::date as day,
                      '2026-10-12 09:00:00+00'::timestamptz as at, '{\"k\": 1}'::jsonb as j,
                      array['x', 'y'] as arr, 9007199254740993::int8 as big, true as t,
                      '1 year 2 mons'::interval as span, null::text as nothing,
                      '127.0.0.1'::inet as addr";
    let response = run(&app, &cookie, sql, false).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    let body = &response.body;
    assert_eq!(first(body, "a"), json!("123.45"));
    assert_eq!(first(body, "b"), json!("-0.001"));
    assert_eq!(first(body, "c"), json!("100000000000000000000"));
    assert_eq!(first(body, "d"), json!("NaN"));
    assert_eq!(first(body, "w"), json!("0.50"));
    assert_eq!(first(body, "day"), json!("2026-10-12"));
    assert_eq!(first(body, "at"), json!("2026-10-12T09:00:00Z"));
    assert_eq!(first(body, "j"), json!({ "k": 1 }));
    assert_eq!(first(body, "arr"), json!(["x", "y"]));
    assert_eq!(first(body, "big"), json!("9007199254740993"));
    assert_eq!(first(body, "t"), json!(true));
    assert_eq!(first(body, "span"), json!("1 year 2 mons"));
    assert_eq!(first(body, "nothing"), Value::Null);
    // ชนิดที่ยังไม่รองรับ: ค่า null + คอลัมน์ทำเครื่องหมายไว้ให้หน้าเว็บแนะนำ ::text
    assert_eq!(first(body, "addr"), Value::Null);
    assert_eq!(column(body, "addr")["supported"], json!(false));

    let explain = run(&app, &cookie, "explain select 1", false).await;
    assert_eq!(explain.status, StatusCode::OK, "{}", explain.body);
    assert_eq!(explain.body["columns"][0]["name"], json!("QUERY PLAN"));

    let error = run(&app, &cookie, "select nope from users", false).await;
    assert_eq!(error.body["code"], json!("42703"));
    assert_eq!(error.body["position"], json!(8), "{}", error.body);

    drop_role(&pool, &reader.role).await;
}

#[sqlx::test]
async fn every_run_is_audited_without_the_sql_text(pool: PgPool) {
    insert_encrypted_patient(&pool).await;
    let reader = reader(&pool).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    let cookie = session_with(&pool, &app, &["sql.view", "patient.view", "audit.view"]).await;

    let ok = run(
        &app,
        &cookie,
        "select first_name from patient where hn_idx = :hn('66471')",
        true,
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.body);
    let failed = run(
        &app,
        &cookie,
        "select * from sessions where token_hash = '66471'",
        false,
    )
    .await;
    assert_eq!(failed.status, StatusCode::BAD_REQUEST);
    let blocked = run(&app, &cookie, "delete from patient", false).await;
    assert_eq!(blocked.status, StatusCode::BAD_REQUEST);

    let rows: Vec<Value> = sqlx::query_scalar(
        "select metadata from audit_log where action = 'sql_executed' and target_type = 'sql' order by id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["status"], json!("ok"));
    assert_eq!(rows[0]["rows"], json!(1));
    assert_eq!(rows[0]["tables"], json!(["patient"]));
    assert_eq!(rows[0]["decryptedCells"], json!(1));
    assert_eq!(rows[0]["sqlSha256"].as_str().unwrap().len(), 64);
    assert_eq!(rows[1]["status"], json!("error"));
    assert_eq!(rows[1]["errorCode"], json!("42501"));
    // ไม่ได้ส่งไป Postgres แต่ยังบันทึกว่ามีคนพยายาม (เก็บแค่ชื่อคำสั่ง)
    assert_eq!(rows[2]["status"], json!("blocked"));
    assert_eq!(rows[2]["reason"], json!("read_only"));
    assert_eq!(rows[2]["keyword"], json!("DELETE"));
    for metadata in &rows {
        let text = metadata.to_string();
        assert!(!text.contains(HN), "{text}");
        assert!(!text.contains("สมชาย"), "{text}");
        assert!(!text.to_lowercase().contains("select"), "{text}");
    }

    // หน้าประวัติการใช้งานสรุปจาก metadata
    let today = chrono::Utc::now().date_naive();
    let activities = send(
        &app,
        Method::GET,
        &format!("/api/audit/activities?from={today}&to={today}"),
        Some(&cookie),
        None,
    )
    .await;
    let labels: Vec<Value> = activities.body["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["action"] == "sql_executed")
        .map(|e| e["targetLabel"].clone())
        .collect();
    assert!(labels.contains(&json!("1 แถว · patient")), "{labels:?}");
    assert!(labels.contains(&json!("ล้มเหลว (42501)")), "{labels:?}");
    assert!(labels.contains(&json!("ไม่อนุญาต (DELETE)")), "{labels:?}");

    drop_role(&pool, &reader.role).await;
}

#[sqlx::test]
async fn schema_lists_only_columns_the_role_can_read(pool: PgPool) {
    let reader = reader(&pool).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    let cookie = session_with(&pool, &app, &["sql.view"]).await;

    let response = send(&app, Method::GET, "/api/sql/schema", Some(&cookie), None).await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    let tables = response.body["tables"].as_array().unwrap();
    let find = |name: &str| tables.iter().find(|t| t["name"] == name);
    let column_names = |name: &str| -> Vec<String> {
        find(name).unwrap()["columns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["name"].as_str().unwrap().to_string())
            .collect()
    };

    let users = column_names("users");
    assert!(users.contains(&"username".to_string()));
    assert!(!users.contains(&"password_hash".to_string()));
    assert!(!column_names("registration_links").contains(&"token_hash".to_string()));
    assert!(find("sessions").is_none());
    assert!(find("login_throttle").is_none());
    let hn = find("patient").unwrap()["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "hn")
        .unwrap()
        .clone();
    assert_eq!(hn["encrypted"], json!(true));
    assert_eq!(response.body["maxRows"], json!(sql_console::MAX_ROWS));

    drop_role(&pool, &reader.role).await;
}

#[sqlx::test]
async fn setup_is_idempotent_and_refuses_unsafe_roles(pool: PgPool) {
    let reader = reader(&pool).await;
    // ใครไปให้สิทธิ์คอลัมน์ลับเพิ่ม — setup รอบถัดไปถอนคืน
    sqlx::query(&format!(
        "grant select (password_hash) on users to {}",
        reader.role
    ))
    .execute(&pool)
    .await
    .unwrap();
    sql_console::setup(&pool, &reader.console).await.unwrap();
    let visible: bool = sqlx::query_scalar(
        "select has_column_privilege($1, 'public.users', 'password_hash', 'select')",
    )
    .bind(&reader.role)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!visible);

    // เป็นสมาชิก role ที่อ่านได้ทุกอย่าง → ไม่ยอมเปิดหน้า SQL
    sqlx::query(&format!("grant pg_read_all_data to {}", reader.role))
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        sql_console::setup(&pool, &reader.console).await,
        Err(SetupError::Unsafe(_))
    ));
    sqlx::query(&format!("revoke pg_read_all_data from {}", reader.role))
        .execute(&pool)
        .await
        .unwrap();

    // ชื่อชนกับ superuser อื่น → ไม่แตะ role นั้นเลย
    let superuser = format!(
        "my_his_sqlt_su_{}",
        &Uuid::new_v4().simple().to_string()[..8]
    );
    sqlx::query(&format!("create role {superuser} superuser"))
        .execute(&pool)
        .await
        .unwrap();
    let options = pool.connect_options();
    let clash = SqlConsole::new(options.as_ref(), &superuser, "irrelevant-password-1").unwrap();
    assert!(matches!(
        sql_console::setup(&pool, &clash).await,
        Err(SetupError::Unsafe(_))
    ));
    let still_superuser: bool =
        sqlx::query_scalar("select rolsuper from pg_roles where rolname = $1")
            .bind(&superuser)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(still_superuser);
    sqlx::query(&format!("drop role {superuser}"))
        .execute(&pool)
        .await
        .unwrap();

    // role ของแอปเอง
    let app_role: String = sqlx::query_scalar("select current_user::text")
        .fetch_one(&pool)
        .await
        .unwrap();
    let own = SqlConsole::new(options.as_ref(), &app_role, "irrelevant-password-1").unwrap();
    assert!(matches!(
        sql_console::setup(&pool, &own).await,
        Err(SetupError::SameAsAppRole)
    ));

    drop_role(&pool, &reader.role).await;
}
