//! role ฐานข้อมูลของหน้า SQL: สร้าง/อัปเดตตอน backend เริ่ม (หลัง migration), ตรวจว่าไม่มีสิทธิ์เกินอ่านอย่างเดียว
//! แล้ว sync สิทธิ์ SELECT ระดับคอลัมน์ตาม allowlist — grant ผูกกับฐานข้อมูล จึงทำใหม่ทุกครั้งที่เริ่ม ตารางที่
//! migration เพิ่มมาใหม่จะมองไม่เห็นจนกว่าจะเพิ่มเข้า `READABLE_TABLES` และคอลัมน์ลับใหม่ต้องเพิ่มใน `HIDDEN_COLUMNS`
//!
//! สร้าง role ได้เพราะตอนนี้แอปต่อ DB ด้วย superuser — ถ้าวันหนึ่งเปลี่ยนเป็นบัญชีธรรมดา (docs/infrastructure.md)
//! ขั้นสร้าง role ต้องย้ายไปทำด้วยมือ ส่วน sync grant ยังทำได้เพราะแอปเป็นเจ้าของตาราง
//!
//! ข้อยกเว้นของกฎ "ห้ามประกอบ SQL" (docs/rust-guidelines.md ข้อ 11): CREATE/ALTER ROLE และ GRANT รับชื่อ role/ตาราง
//! และรหัสผ่านเป็น bind parameter ไม่ได้ — ให้ Postgres ประกอบคำสั่งเองด้วย `format('%I', '%L')` จากค่าที่ bind
//! เข้าไป (quote ถูกต้องเสมอ) แล้วรันข้อความที่ได้ ไม่ต่อ string ใน Rust

use std::time::Duration;

use sqlx::postgres::{PgConnectOptions, PgConnection};
use sqlx::{ConnectOptions, Connection, PgPool};

/// ชื่อ role ของหน้า SQL บนเครื่องจริง (host/ฐานข้อมูลเดียวกับ DATABASE_URL รหัสจาก SQL_CONSOLE_DB_PASSWORD)
pub const READER_ROLE: &str = "my_his_sql_reader";

/// ตารางที่หน้า SQL อ่านได้ — ตารางที่ไม่อยู่ในรายการมองไม่เห็นเลย: `sessions` (token_hash ของ session),
/// `login_throttle` (ชื่อที่พิมพ์ตอน login ผิด อาจเป็นรหัสผ่านที่พิมพ์ผิดช่อง), `_sqlx_migrations`, ตารางใหม่ ๆ
const READABLE_TABLES: [&str; 19] = [
    "audit_log",
    "budget_year",
    "disease",
    "lab",
    "lab_visit",
    "lab_visit_group",
    "medical_condition",
    "nationality",
    "page_permissions",
    "pages",
    "patient",
    "patient_group",
    "prename",
    "registration_links",
    "role_permissions",
    "roles",
    "user_roles",
    "users",
    "visit_group",
];

/// คอลัมน์ลับในตารางที่อ่านได้ (`<ตาราง>.<คอลัมน์>`) — ไม่ grant
const HIDDEN_COLUMNS: [&str; 2] = ["users.password_hash", "registration_links.token_hash"];

/// connection พร้อมกันสูงสุดของ role (แต่ละการรันเปิด connection ใหม่หนึ่งตัวแล้วปิด)
const CONNECTION_LIMIT: i32 = 4;

/// ต้องน้อยกว่า TimeoutLayer 10 วินาทีของทั้ง request (app.rs) ให้หน้าเว็บได้ข้อความ error ของ Postgres ก่อน
const DEFAULT_STATEMENT_TIMEOUT: Duration = Duration::from_secs(5);

const MAX_ROLE_NAME_LEN: usize = 63;

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("invalid sql console role name")]
    InvalidRoleName,
    #[error("sql console role must differ from the application's own database role")]
    SameAsAppRole,
    #[error("sql console role {0} has privileges beyond read-only access")]
    Unsafe(String),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// ที่อยู่และตัวตนของ role หน้า SQL — ไม่มี pool (แต่ละการรันเปิด connection ใหม่ กัน SET ค้างข้าม request)
pub struct SqlConsole {
    role: String,
    password: String,
    options: PgConnectOptions,
    statement_timeout: Duration,
}

/// เขียน Debug เอง — options และ password มีรหัสของ role
impl std::fmt::Debug for SqlConsole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlConsole")
            .field("role", &self.role)
            .field("password", &"[redacted]")
            .field("statement_timeout", &self.statement_timeout)
            .finish()
    }
}

impl SqlConsole {
    /// `base` = connect options ของ pool หลัก (host/port/ฐานข้อมูลเดิม) — เปลี่ยนแค่ผู้ใช้/รหัสเป็นของ role หน้า SQL
    /// ปิดการ log statement (SQL ของผู้ใช้อาจมี HN/blind index) และ statement cache (connection ใช้ครั้งเดียว)
    pub fn new(base: &PgConnectOptions, role: &str, password: &str) -> Result<Self, SetupError> {
        if !is_valid_role_name(role) {
            return Err(SetupError::InvalidRoleName);
        }
        let options = base
            .clone()
            .username(role)
            .password(password)
            .application_name("my-his sql console")
            .statement_cache_capacity(0)
            .disable_statement_logging();
        Ok(SqlConsole {
            role: role.to_string(),
            password: password.to_string(),
            options,
            statement_timeout: DEFAULT_STATEMENT_TIMEOUT,
        })
    }

    /// เทสย่น timeout ได้ (เทส pg_sleep ไม่ต้องรอ 5 วินาที)
    pub fn with_statement_timeout(mut self, timeout: Duration) -> Self {
        self.statement_timeout = timeout;
        self
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub(crate) fn options(&self) -> &PgConnectOptions {
        &self.options
    }

    pub(crate) fn statement_timeout(&self) -> Duration {
        self.statement_timeout
    }
}

/// ตัวพิมพ์เล็ก/ตัวเลข/ขีดล่าง ขึ้นต้นด้วยตัวอักษรหรือขีดล่าง ไม่เกิน 63 ตัว (ชื่อที่ไม่ต้อง quote ใน Postgres)
fn is_valid_role_name(name: &str) -> bool {
    let mut chars = name.chars();
    let valid_first = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c == '_');
    valid_first
        && name.len() <= MAX_ROLE_NAME_LEN
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// สร้าง/อัปเดต role, ตรวจความปลอดภัย, sync grant บนฐานข้อมูลของ `pool` — รันตอนเริ่ม (main.rs) และใน fixture ของเทส
pub async fn setup(pool: &PgPool, console: &SqlConsole) -> Result<(), SetupError> {
    // connection แยกที่ปิดการ log statement — คำสั่ง CREATE/ALTER ROLE มีรหัสผ่านอยู่ในข้อความ
    let options = pool
        .connect_options()
        .as_ref()
        .clone()
        .disable_statement_logging();
    let mut conn = PgConnection::connect_with(&options).await?;
    let result = setup_on(&mut conn, console).await;
    // ปิดไม่สำเร็จก็ไม่เป็นไร connection ถูกทิ้งอยู่ดี — ผลที่สำคัญคือ `result`
    let _ = conn.close().await;
    result
}

async fn setup_on(conn: &mut PgConnection, console: &SqlConsole) -> Result<(), SetupError> {
    let role = console.role.as_str();
    let app_role: String = sqlx::query_scalar!(r#"select current_user::text as "name!""#)
        .fetch_one(&mut *conn)
        .await?;
    if app_role == role {
        return Err(SetupError::SameAsAppRole);
    }

    let existing = sqlx::query_scalar!(
        r#"select rolsuper as "rolsuper!" from pg_roles where rolname = $1"#,
        role
    )
    .fetch_optional(&mut *conn)
    .await?;
    // role ชื่อนี้เป็น superuser อยู่แล้ว = ตั้งชื่อชนกับบัญชีอื่น ไม่แตะ (ALTER ROLE NOSUPERUSER จะไปปลดสิทธิ์คนอื่น)
    if existing == Some(true) {
        return Err(SetupError::Unsafe(role.to_string()));
    }

    // ตั้งคุณสมบัติทุกครั้ง (แม้มีอยู่แล้ว) — รหัสผ่านตาม .env เสมอ และดึงสิทธิ์ที่ใครไปเพิ่มให้ทีหลังกลับ
    let template = if existing.is_some() {
        ALTER_ROLE
    } else {
        CREATE_ROLE
    };
    let statement: String = sqlx::query_scalar(template)
        .bind(role)
        .bind(CONNECTION_LIMIT)
        .bind(&console.password)
        .fetch_one(&mut *conn)
        .await?;
    execute_built(conn, &statement).await?;

    // ค่าเริ่มต้นของทุก session ของ role — กันซ้ำนอกเหนือจาก SET LOCAL ที่ทำทุกครั้งที่รัน (repository.rs)
    let timeout = format!("{}ms", console.statement_timeout.as_millis());
    let defaults: Vec<String> = sqlx::query_scalar(ROLE_DEFAULTS)
        .bind(role)
        .bind(timeout)
        .fetch_all(&mut *conn)
        .await?;
    for statement in &defaults {
        execute_built(conn, statement).await?;
    }

    ensure_read_only(conn, role).await?;
    sync_grants(conn, role).await
}

// SQL ที่ "ประกอบคำสั่ง" ด้วย format() ใช้ sqlx::query_scalar แบบ runtime ไม่ใช่ query! — macro ตรวจตอนคอมไพล์ด้วยการ
// EXPLAIN โดยใส่ NULL แทนพารามิเตอร์ และ format('%I', NULL) ล้มตั้งแต่ขั้น plan ข้อความ SQL ทั้งหมดคงที่ ค่าทุกตัว bind

const CREATE_ROLE: &str = "select format('create role %I with login nosuperuser nocreatedb nocreaterole noinherit
                                          noreplication nobypassrls connection limit %s password %L',
                                         $1::text, $2::int4, $3::text)";

const ALTER_ROLE: &str =
    "select format('alter role %I with login nosuperuser nocreatedb nocreaterole noinherit
                                         noreplication nobypassrls connection limit %s password %L',
                                        $1::text, $2::int4, $3::text)";

const ROLE_DEFAULTS: &str = "select unnest(array[
                                 format('alter role %I set default_transaction_read_only = on', $1::text),
                                 format('alter role %I set statement_timeout = %L', $1::text, $2::text)
                             ])";

const BASE_GRANTS: &str = "select unnest(array[
                               format('revoke all on all tables in schema public from %I', $1::text),
                               format('revoke all on all sequences in schema public from %I', $1::text),
                               format('grant connect on database %I to %I', current_database(), $1::text),
                               format('grant usage on schema public to %I', $1::text)
                           ])";

/// grant ทีละตาราง: คอลัมน์ทั้งหมดของตารางใน allowlist ($1) ยกเว้นคอลัมน์ลับ ($2) ให้ role ($3)
const COLUMN_GRANTS: &str = "select format('grant select (%s) on public.%I to %I',
                                           string_agg(quote_ident(c.column_name::text), ', '
                                                      order by c.ordinal_position),
                                           c.table_name::text, $3::text)
                             from information_schema.columns c
                             where c.table_schema = 'public'
                               and c.table_name::text = any($1::text[])
                               and not ((c.table_name::text || '.' || c.column_name::text) = any($2::text[]))
                             group by c.table_name
                             order by c.table_name";

/// ไม่ใช่ superuser/createrole/createdb/bypassrls/replication และไม่ได้เป็นสมาชิก role ใดเลย (pg_read_server_files,
/// pg_execute_server_program, pg_read_all_data ฯลฯ ล้วนมาทาง membership) — ไม่ผ่าน = ไม่เปิดหน้า SQL
async fn ensure_read_only(conn: &mut PgConnection, role: &str) -> Result<(), SetupError> {
    let row = sqlx::query!(
        r#"select r.rolsuper as "rolsuper!", r.rolcreaterole as "rolcreaterole!",
                  r.rolcreatedb as "rolcreatedb!", r.rolbypassrls as "rolbypassrls!",
                  r.rolreplication as "rolreplication!",
                  exists(select 1 from pg_auth_members m where m.member = r.oid) as "is_member!"
           from pg_roles r
           where r.rolname = $1"#,
        role
    )
    .fetch_one(&mut *conn)
    .await?;
    let elevated = row.rolsuper
        || row.rolcreaterole
        || row.rolcreatedb
        || row.rolbypassrls
        || row.rolreplication
        || row.is_member;
    if elevated {
        return Err(SetupError::Unsafe(role.to_string()));
    }
    Ok(())
}

/// ถอนสิทธิ์ทุกตาราง/sequence ใน public แล้ว grant SELECT เฉพาะคอลัมน์ตาม allowlist ใหม่ทั้งชุด (transaction เดียว)
/// การถอนสิทธิ์ระดับตารางถอนสิทธิ์ระดับคอลัมน์ไปด้วย คอลัมน์ที่เคยให้แล้วเอาออกจากรายการจะหายจริง
async fn sync_grants(conn: &mut PgConnection, role: &str) -> Result<(), SetupError> {
    let mut tx = conn.begin().await?;

    let base: Vec<String> = sqlx::query_scalar(BASE_GRANTS)
        .bind(role)
        .fetch_all(&mut *tx)
        .await?;
    for statement in &base {
        execute_built(&mut tx, statement).await?;
    }

    let tables: Vec<String> = READABLE_TABLES.iter().map(|t| t.to_string()).collect();
    let hidden: Vec<String> = HIDDEN_COLUMNS.iter().map(|c| c.to_string()).collect();
    let grants: Vec<String> = sqlx::query_scalar(COLUMN_GRANTS)
        .bind(&tables)
        .bind(&hidden)
        .bind(role)
        .fetch_all(&mut *tx)
        .await?;
    if grants.len() != READABLE_TABLES.len() {
        tracing::warn!(
            expected = READABLE_TABLES.len(),
            granted = grants.len(),
            "some sql console tables do not exist; check READABLE_TABLES"
        );
    }
    for statement in &grants {
        execute_built(&mut tx, statement).await?;
    }

    tx.commit().await?;
    Ok(())
}

/// รันคำสั่งที่ Postgres ประกอบเองด้วย format('%I', '%L') (ดูหัวไฟล์) — ไม่มีค่าจากผู้ใช้เว็บปนอยู่
async fn execute_built(conn: &mut PgConnection, statement: &str) -> Result<(), sqlx::Error> {
    sqlx::query(statement).execute(&mut *conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_names_must_be_plain_lowercase_identifiers() {
        assert!(is_valid_role_name(READER_ROLE));
        assert!(is_valid_role_name("_reader2"));
        assert!(!is_valid_role_name(""));
        assert!(!is_valid_role_name("2reader"));
        assert!(!is_valid_role_name("Reader"));
        assert!(!is_valid_role_name("reader; drop"));
        assert!(!is_valid_role_name(&"a".repeat(64)));
    }

    #[test]
    fn debug_hides_password() {
        let base = PgConnectOptions::new().host("db").database("my_his");
        let console = SqlConsole::new(&base, READER_ROLE, "reader-secret-345").unwrap();
        let printed = format!("{console:?}");
        assert!(!printed.contains("reader-secret-345"), "{printed}");
        assert!(printed.contains(READER_ROLE));
    }

    #[test]
    fn hidden_columns_belong_to_readable_tables() {
        for column in HIDDEN_COLUMNS {
            let table = column.split('.').next().unwrap();
            assert!(READABLE_TABLES.contains(&table), "{column}");
        }
    }
}
