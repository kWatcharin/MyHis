//! ขอบเขตของหน้า SQL ที่เกิดจาก security audit รอบ 2 (docs/security-audit.md ข้อ 8):
//! - คำสั่งที่ถูกยกเลิกกลางทาง (client ปิด connection) ยังมีแถว audit ครบหนึ่งแถว — การรัน + บันทึก audit ทำใน task ที่ไม่ผูกกับ handler
//! - ‘keyword’ ของคำสั่งที่ถูกบล็อกเป็นชื่อคำสั่งจากชุดคงที่หรือ `OTHER` เสมอ ไม่ใช่ข้อความที่ผู้ใช้พิมพ์ (ขนาดแถว audit คงที่)
//! - ผลลัพธ์ที่ใหญ่เกินเพดาน (ไบต์บนสาย, numeric/bytea/array ที่ขยายเมื่อแปลง, จำนวนคอลัมน์) ถูกปฏิเสธด้วย 400 ที่ยังบันทึก audit
//!   โดยไม่จองหน่วยความจำตามที่ผู้ใช้ขอ
//!
//! แต่ละเทสสร้าง role ของตัวเอง (ชื่อสุ่ม) แล้วลบตอนจบ — เหมือน tests/sql_console.rs

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::{Method, StatusCode};
use axum::Router;
use common::{insert_user_with_password, login, send, test_app_with_sql_console, TestResponse};
use my_his_backend::conn_guard::{GuardedListener, IDLE_TIMEOUT, MAX_CONNECTIONS};
use my_his_backend::sql_console::{self, SqlConsole};
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

const PASSWORD: &str = "sql-console-test-password";

struct Reader {
    role: String,
    console: Arc<SqlConsole>,
}

async fn reader(pool: &PgPool, timeout: Option<Duration>) -> Reader {
    let role = format!("my_his_sqlt_{}", &Uuid::new_v4().simple().to_string()[..12]);
    let password = Uuid::new_v4().simple().to_string();
    let mut console = SqlConsole::new(pool.connect_options().as_ref(), &role, &password)
        .unwrap()
        .with_decrypt_attempts(20_000);
    if let Some(timeout) = timeout {
        console = console.with_statement_timeout(timeout);
    }
    sql_console::setup(pool, &console).await.unwrap();
    Reader {
        role,
        console: Arc::new(console),
    }
}

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

async fn run(app: &Router, cookie: &str, sql: &str) -> TestResponse {
    send(
        app,
        Method::POST,
        "/api/sql/query",
        Some(cookie),
        Some(json!({ "sql": sql, "decrypt": false })),
    )
    .await
}

async fn audit_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("select count(*) from audit_log where action = 'sql_executed'")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn last_audit(pool: &PgPool) -> Value {
    sqlx::query_scalar(
        "select metadata from audit_log where action = 'sql_executed' order by id desc limit 1",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

// ---- keyword ของคำสั่งที่ถูกบล็อก ----

#[sqlx::test]
async fn a_blocked_keyword_is_a_fixed_command_name_or_other(pool: PgPool) {
    let reader = reader(&pool, None).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    let cookie = session_with(&pool, &app, &["sql.view", "audit.view"]).await;

    // คำแรกยาว 5,000 ตัวอักษร (ตัวอักษรล้วน) — เดิมถูกเก็บทั้งก้อนใน audit_log, body 400 และ label ของหน้าประวัติ
    let long_word = "z".repeat(5_000);
    let response = run(&app, &cookie, &long_word).await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(response.body["error"], "statement not allowed");
    assert_eq!(response.body["keyword"], "OTHER");
    let audit = last_audit(&pool).await;
    assert_eq!(audit["keyword"], "OTHER");
    assert_eq!(audit["status"], "blocked");
    let stored: i64 = sqlx::query_scalar(
        "select pg_column_size(metadata)::int8 from audit_log
         where action = 'sql_executed' order by id desc limit 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        stored < 400,
        "audit row must stay small, got {stored} bytes"
    );

    // ป้ายที่หน้าประวัติแสดงก็สั้นเช่นกัน
    let today = chrono::Utc::now().date_naive();
    let (from, to) = (
        today - chrono::Duration::days(1),
        today + chrono::Duration::days(1),
    );
    let activities = send(
        &app,
        Method::GET,
        &format!("/api/audit/activities?from={from}&to={to}"),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(activities.status, StatusCode::OK, "{}", activities.body);
    let longest = activities.body["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|event| event["targetLabel"].as_str())
        .map(|label| label.chars().count())
        .max()
        .unwrap_or(0);
    assert!(longest < 200, "activity label too long: {longest}");

    // ชื่อคำสั่งจริงยังเห็นตามเดิม (ตัวพิมพ์ไม่สำคัญ)
    for (sql, keyword) in [
        ("insert into prename values (1)", "INSERT"),
        ("DeLeTe from audit_log", "DELETE"),
        ("grant all on prename to public", "GRANT"),
        ("vacuum", "VACUUM"),
        ("frobnicate the database", "OTHER"),
    ] {
        let response = run(&app, &cookie, sql).await;
        assert_eq!(response.body["keyword"], keyword, "{sql}");
    }
    drop_role(&pool, &reader.role).await;
}

// ---- ผลลัพธ์ที่ใหญ่เกินเพดาน ----

fn assert_too_large(response: &TestResponse, label: &str) {
    assert_eq!(response.status, StatusCode::BAD_REQUEST, "{label}");
    assert_eq!(response.body["error"], "query failed", "{label}");
    assert!(
        response.body["message"]
            .as_str()
            .unwrap_or_default()
            .starts_with("result too large"),
        "{label}: {}",
        response.body
    );
}

#[sqlx::test]
async fn oversized_results_are_refused_without_allocating_what_was_asked(pool: PgPool) {
    // timeout ยาวพอให้ Postgres สร้างข้อมูลก้อนใหญ่ได้ (build ของเทสไม่ optimize) แต่ต้องไม่ถึง 10 วินาทีของ TimeoutLayer
    let reader = reader(&pool, Some(Duration::from_secs(8))).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    let cookie = session_with(&pool, &app, &["sql.view"]).await;

    // ข้อความเดียว 64 MiB (SQL 40 ตัวอักษร): เกินงบไบต์บนสาย — หยุดอ่านก่อน ไม่ buffer ทั้งก้อน
    let before = audit_count(&pool).await;
    let text = run(&app, &cookie, "select repeat('x', 67108864) as v").await;
    assert_too_large(&text, "64 MiB text");
    assert_eq!(audit_count(&pool).await, before + 1);
    let audit = last_audit(&pool).await;
    assert_eq!(audit["status"], "error");
    assert_eq!(audit["rows"], 0);

    // bytea: hex ขยายสองเท่า
    let bytea = run(
        &app,
        &cookie,
        "select decode(repeat('ab', 12000000), 'hex') as v",
    )
    .await;
    assert_too_large(&bytea, "12 MB bytea");

    // numeric ที่เก็บ ~10 ไบต์แต่ขยายเป็น ~131,000 ตัวอักษรต่อช่อง: 200 แถว × 3 คอลัมน์ ≈ 78 MB ของข้อความ ถ้าไม่มีงบ
    let numeric = run(
        &app,
        &cookie,
        "select ('1' || repeat('0', 131000))::numeric as a, ('1' || repeat('0', 131000))::numeric as b,
                ('1' || repeat('0', 131000))::numeric as c
         from generate_series(1, 200)",
    )
    .await;
    assert_too_large(&numeric, "numeric expansion");

    // array หลายล้าน element
    let array = run(
        &app,
        &cookie,
        "select array_agg('x'::text) as v from generate_series(1, 3000000)",
    )
    .await;
    assert_too_large(&array, "3M element array");

    // คอลัมน์เกินเพดาน (200) — ปฏิเสธหลัง prepare ก่อนรัน
    let columns = (1..=201)
        .map(|n| format!("{n} as c{n}"))
        .collect::<Vec<_>>()
        .join(", ");
    let wide = run(&app, &cookie, &format!("select {columns}")).await;
    assert_too_large(&wide, "201 columns");

    // คำสั่งปกติยังผ่าน (เพดานไม่ได้บีบการใช้งานจริง): 200 คอลัมน์พอดี และผลลัพธ์ขนาดกลาง
    let columns = (1..=200)
        .map(|n| format!("{n} as c{n}"))
        .collect::<Vec<_>>()
        .join(", ");
    let ok = run(&app, &cookie, &format!("select {columns}")).await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.body);
    let medium = run(
        &app,
        &cookie,
        "select g, repeat('x', 1000) as filler from generate_series(1, 500) g",
    )
    .await;
    assert_eq!(medium.status, StatusCode::OK, "{}", medium.body);
    assert_eq!(medium.body["rowCount"], 500);
    assert_eq!(medium.body["rows"].as_array().unwrap().len(), 500);
    drop_role(&pool, &reader.role).await;
}

// ---- ยกเลิกกลางทางแล้ว audit ต้องไม่หาย (เซิร์ฟเวอร์จริงบนซ็อกเก็ต loopback) ----

/// เซิร์ฟเวอร์ตัวจริง (GuardedListener + router ของแอป) บนซ็อกเก็ต loopback
async fn serve(app: Router) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let guarded = GuardedListener::new(listener, MAX_CONNECTIONS, IDLE_TIMEOUT);
    tokio::spawn(async move { axum::serve(guarded, app).await.unwrap() });
    addr
}

/// ส่ง POST /api/sql/query หนึ่งครั้งแล้วคืน stream (ไม่มี Sec-Fetch-Site/Origin จึงผ่านด่าน cross-site เหมือน curl)
async fn post_query(addr: std::net::SocketAddr, cookie: &str, sql: &str) -> TcpStream {
    let body = json!({ "sql": sql, "decrypt": false }).to_string();
    let head = format!(
        "POST /api/sql/query HTTP/1.1\r\nHost: test\r\nCookie: {cookie}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let bytes = [head.as_bytes(), body.as_bytes()].concat();
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut sent = 0;
    while sent < bytes.len() {
        stream.writable().await.unwrap();
        match stream.try_write(&bytes[sent..]) {
            Ok(n) => sent += n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("write failed: {e}"),
        }
    }
    stream
}

#[sqlx::test]
async fn a_cancelled_request_still_leaves_exactly_one_audit_row(pool: PgPool) {
    let reader = reader(&pool, None).await;
    let app = test_app_with_sql_console(pool.clone(), reader.console.clone()).await;
    let cookie = session_with(&pool, &app, &["sql.view"]).await;
    let addr = serve(app.clone()).await;

    let before = audit_count(&pool).await;
    let started = Instant::now();
    let stream = post_query(addr, &cookie, "select pg_sleep(2)").await;
    // client หายไปกลางคำสั่ง (ก่อนมี response): handler ถูกยกเลิก แต่ task ‘รัน + บันทึก audit’ ต้องทำต่อ
    tokio::time::sleep(Duration::from_millis(500)).await;
    drop(stream);

    // รอจน Postgres รันคำสั่งเสร็จ (2 วินาที) แล้วเผื่อเวลาให้บันทึก audit
    let mut rows = 0;
    while started.elapsed() < Duration::from_secs(12) {
        tokio::time::sleep(Duration::from_millis(250)).await;
        rows = audit_count(&pool).await - before;
        if rows > 0 {
            break;
        }
    }
    assert_eq!(rows, 1, "the cancelled statement must still be audited");
    let audit = last_audit(&pool).await;
    assert_eq!(audit["status"], "ok");
    // และไม่มีแถวที่สองตามมาทีหลัง
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(audit_count(&pool).await - before, 1);
    drop_role(&pool, &reader.role).await;
}
