//! กฎของหน้า SQL: สิทธิ์ (`sql.view`; ถอดรหัสต้องมี `patient.view` ด้วย), ขนาด SQL, รับเฉพาะคำสั่งอ่านทีละคำสั่ง
//! (คำสั่งอื่นตอบ `Blocked` ให้หน้าเว็บเตือน), ตัวช่วย `:hn()`/`:nap_no()`,
//! จำกัดจำนวนแถว, ถอดรหัสคอลัมน์ผู้ป่วยตามต้นทางจริงของคอลัมน์, บันทึก audit_log ทุกครั้ง — ห้าม import ชนิดของ Axum
//!
//! **ห้าม log** ตัว SQL, ข้อความ error ของ Postgres หรือค่าในผลลัพธ์ (อาจมีข้อมูลผู้ป่วย) — audit_log เก็บแค่ SHA-256
//! ของคำสั่งที่รันจริง (หลังแทนตัวช่วยแล้ว จึงไม่มี HN ดิบ), จำนวนแถว, ชื่อตาราง, จำนวนช่องที่ถอดรหัส

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgDatabaseError, PgErrorPosition};
use sqlx::{Column, PgPool, Row, TypeInfo, ValueRef};

use crate::auth::CurrentUser;
use crate::crypto::Crypto;
use crate::domain::patient_crypto;
use crate::domain::{Action, Resource, UserId};
use crate::sql_console::embedded;
use crate::sql_console::repository::{self, EncryptedColumn, Executed, RepositoryError};
use crate::sql_console::setup::SqlConsole;
use crate::sql_console::sql_text;
use crate::sql_console::value::{self, ValueKind};

pub const MAX_SQL_CHARS: usize = 20_000;
/// ข้อความ error (400) เมื่อใช้ตัวช่วย :hn() / :nap_no() โดยไม่มี patient.view — หน้าเว็บแปลเป็นไทย (sqlConsoleApi.ts)
pub const HELPERS_NEED_PATIENT_VIEW: &str = "helpers need patient.view";
/// แถวสูงสุดที่ส่งกลับ — มีมากกว่านี้ตอบ truncated (ครอบคำสั่งด้วย LIMIT MAX_ROWS + 1 เพื่อรู้ว่ามีเกิน)
pub const MAX_ROWS: usize = 500;
/// ขนาดผลลัพธ์สูงสุดต่อคำขอ: ทั้ง (ก) ไบต์ที่รับจาก Postgres (หยุดอ่านทันทีเมื่อเกิน — repository::execute) และ (ข) งบหน่วยความจำของ JSON
/// ที่แปลงแล้ว (value::Budget — numeric/bytea/array ขยายใหญ่กว่าที่ส่งมา) แถวถูกจำกัดแล้ว (MAX_ROWS) แต่ความกว้างของแถวไม่ถูกจำกัด
/// → query สั้น ๆ เช่น `select repeat('x', 67108864)` ทำให้ backend จองหน่วยความจำ ~237 MB (วัดใน security audit รอบ 2)
pub const MAX_RESULT_BYTES: usize = 16 * 1024 * 1024;
/// คอลัมน์สูงสุดในผลลัพธ์ (Postgres ให้ถึง 1,664) — เกินแล้วปฏิเสธหลัง prepare ก่อนรัน
pub const MAX_COLUMNS: usize = 200;
/// ข้อความ error (400) เมื่อผลลัพธ์ใหญ่เกินเพดาน — หน้าเว็บแปลเป็นไทย (sqlConsoleApi.ts) อย่าแก้โดยไม่แก้ตัวแปลด้วย
pub const RESULT_TOO_LARGE: &str =
    "result too large: select fewer columns or rows, or shorten the values (cast to ::text and use left())";

/// ชื่อคำสั่งที่ยอมให้บันทึกใน audit / ส่งกลับ เมื่อคำแรกของคำสั่งไม่ใช่การอ่าน — ที่เหลือบันทึกเป็น `OTHER`
const BLOCKED_COMMANDS: &[&str] = &[
    "INSERT", "UPDATE", "DELETE", "MERGE", "CREATE", "ALTER", "DROP", "TRUNCATE", "COPY", "SET",
    "RESET", "DO", "CALL", "GRANT", "REVOKE", "VACUUM", "ANALYZE", "LOCK", "BEGIN", "COMMIT",
    "ROLLBACK", "PREPARE", "EXECUTE", "LISTEN", "NOTIFY",
];

/// คำแรกที่ผู้ใช้พิมพ์ (ตัวอักษรล้วน ยาวได้ถึง `MAX_SQL_CHARS`) → ชื่อคำสั่งจากชุดคงที่ หรือ `OTHER` — audit_log เก็บแค่ SHA-256
/// ของคำสั่ง ไม่เก็บข้อความที่พิมพ์ (ดูหัวไฟล์) ผู้ใช้เลือกขนาด/เนื้อหาของแถว audit ผ่านคำแรกไม่ได้
fn audit_keyword(word: &str) -> String {
    let upper = word.to_ascii_uppercase();
    if BLOCKED_COMMANDS.contains(&upper.as_str()) {
        upper
    } else {
        "OTHER".to_string()
    }
}
/// ข้อความที่ครอบหน้าคำสั่งของผู้ใช้ — ตำแหน่ง error ของ Postgres ต้องลบความยาวนี้ออกก่อนส่งให้หน้าเว็บ
const WRAP_PREFIX: &str = "select * from (\n";

#[derive(Debug, thiserror::Error)]
pub enum SqlConsoleError {
    #[error("forbidden")]
    Forbidden,
    #[error("sql console not configured")]
    NotConfigured,
    #[error("{0}")]
    Validation(String),
    /// Postgres ปฏิเสธคำสั่ง — ส่งข้อความให้ผู้ใช้ (ผู้ใช้พิมพ์ SQL เอง) แต่ไม่ log
    #[error("query failed")]
    QueryFailed(QueryFailure),
    /// คำสั่งที่หน้า SQL ไม่รัน (แก้ข้อมูล / หลายคำสั่ง) — หน้าเว็บแสดงคำเตือนตามเหตุผลแทน error ของ Postgres
    #[error("statement not allowed")]
    Blocked(Blocked),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// error ของ Postgres ที่ส่งกลับหน้าเว็บ — `position` = ตำแหน่งตัวอักษร (เริ่ม 1) ในข้อความที่ผู้ใช้พิมพ์
pub struct QueryFailure {
    pub message: String,
    pub code: Option<String>,
    pub position: Option<usize>,
}

/// ข้อความ error อาจมีค่าจากคำสั่งของผู้ใช้ — Debug ไม่พิมพ์ข้อความ
impl std::fmt::Debug for QueryFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryFailure")
            .field("code", &self.code)
            .field("message", &"[redacted]")
            .finish()
    }
}

/// เหตุที่หน้า SQL ไม่รันคำสั่ง — `keyword` คือชื่อคำสั่ง SQL จากชุดคงที่ (INSERT, DELETE, SELECT FOR UPDATE, … หรือ `OTHER` สำหรับคำแรก
/// ที่ไม่อยู่ในชุด — `audit_keyword`) ไม่ใช่ข้อความที่ผู้ใช้พิมพ์ จึงส่งกลับหน้าเว็บและเก็บใน audit ได้ (ก่อนมี allowlist คำแรกที่เป็นตัวอักษร
/// ล้วนยาวถึง 20,000 ตัวถูกเก็บลง audit_log, body 400 และ label ของหน้าประวัติ — security audit รอบ 2)
#[derive(Debug, Clone)]
pub enum Blocked {
    /// ไม่ใช่การอ่านข้อมูล — ตรวจจากคำแรกก่อนรัน หรือ Postgres ตอบว่าแก้ข้อมูลใน read only ไม่ได้
    /// (`keyword` None = รู้แค่ว่ามีการแก้ข้อมูล เช่น WITH ที่มี DELETE ข้างใน)
    ReadOnly { keyword: Option<String> },
    /// มี `;` คั่นหลายคำสั่ง (รันได้ทีละคำสั่ง)
    MultipleStatements,
}

impl Blocked {
    pub fn reason(&self) -> &'static str {
        match self {
            Blocked::ReadOnly { .. } => "read_only",
            Blocked::MultipleStatements => "multiple_statements",
        }
    }

    pub fn keyword(&self) -> Option<&str> {
        match self {
            Blocked::ReadOnly { keyword } => keyword.as_deref(),
            Blocked::MultipleStatements => None,
        }
    }
}

pub struct ResultColumn {
    pub name: String,
    pub type_name: String,
    /// false = ชนิดที่ยังแปลงเป็น JSON ไม่ได้ (ค่าเป็น null — หน้าเว็บแนะนำให้ cast เป็น ::text)
    pub supported: bool,
    /// มาจากคอลัมน์ที่เข้ารหัสของตาราง patient (ตาม OID/attnum ไม่ใช่ชื่อ)
    pub encrypted: bool,
    pub decrypted: bool,
}

/// ผลลัพธ์ที่อาจมีข้อมูลผู้ป่วยที่ถอดรหัสแล้ว — ไม่ derive Debug กันหลุดลง log
pub struct QueryResult {
    pub columns: Vec<ResultColumn>,
    pub rows: Vec<Vec<Value>>,
    pub truncated: bool,
    pub duration_ms: u64,
    /// ช่องที่ควรถอดรหัสได้แต่ถอดไม่ได้ (ค่าเป็น null — ไม่มีทางได้ข้อความผิดออกไป)
    pub decrypt_errors: usize,
}

#[derive(Debug)]
pub struct SchemaTable {
    pub name: String,
    pub columns: Vec<SchemaColumn>,
}

#[derive(Debug)]
pub struct SchemaColumn {
    pub name: String,
    pub type_name: String,
    pub encrypted: bool,
}

/// ตรวจสิทธิ์ก่อน (ไม่บอกผู้ไม่มีสิทธิ์ว่าตั้งค่าหรือยัง) แล้วจึงดูว่าเปิดหน้า SQL ไว้ไหม
fn authorize<'a>(
    console: Option<&'a SqlConsole>,
    actor: &CurrentUser,
) -> Result<&'a SqlConsole, SqlConsoleError> {
    if !actor.can(Resource::Sql, Action::View) {
        return Err(SqlConsoleError::Forbidden);
    }
    console.ok_or(SqlConsoleError::NotConfigured)
}

fn is_encrypted_column(table: &str, column: &str) -> bool {
    table == patient_crypto::TABLE
        && patient_crypto::ENCRYPTED_COLUMNS
            .iter()
            .any(|(name, _)| *name == column)
}

/// ตาราง/คอลัมน์ที่ role ของหน้า SQL อ่านได้ (ใช้ทำ auto complete และรายการตารางด้านข้าง)
pub async fn schema(
    console: Option<&SqlConsole>,
    actor: &CurrentUser,
) -> Result<Vec<SchemaTable>, SqlConsoleError> {
    let console = authorize(console, actor)?;
    let mut conn = repository::connect(console).await?;
    let columns = repository::readable_columns(&mut conn).await;
    repository::close(conn).await;

    let mut tables: Vec<SchemaTable> = Vec::new();
    for column in columns? {
        let encrypted = is_encrypted_column(&column.table, &column.column);
        let entry = SchemaColumn {
            name: column.column,
            type_name: column.data_type,
            encrypted,
        };
        match tables.last_mut() {
            Some(table) if table.name == column.table => table.columns.push(entry),
            _ => tables.push(SchemaTable {
                name: column.table,
                columns: vec![entry],
            }),
        }
    }
    Ok(tables)
}

/// คำสั่งที่ส่งไป Postgres จริง
struct Prepared {
    sql: String,
    /// ความยาว (ตัวอักษร) ของข้อความที่ครอบไว้หน้าคำสั่งของผู้ใช้
    prefix_chars: usize,
    body_chars: usize,
    helpers_replaced: usize,
    /// Some = ไม่ส่งไป Postgres (ยังบันทึก audit ว่าถูกปฏิเสธ)
    blocked: Option<Blocked>,
}

/// `helpers_allowed` = ผู้เรียกถือ patient.view: `:hn()` / `:nap_no()` แปลงข้อความเป็น blind index ที่คำนวณด้วย key ลับ แล้วเทียบกับ
/// `hn_idx` / `nap_no_idx` (reader role อ่านได้) จึงเดา/ยืนยันเลขระบุตัวผู้ป่วยได้ — ต้องอยู่ใต้เกต patient.view เดียวกับการถอดรหัส
fn prepare(sql: &str, crypto: &Crypto, helpers_allowed: bool) -> Result<Prepared, SqlConsoleError> {
    if sql.trim().is_empty() {
        return Err(SqlConsoleError::Validation("sql is required".to_string()));
    }
    if sql.chars().count() > MAX_SQL_CHARS {
        return Err(SqlConsoleError::Validation(format!(
            "sql must be at most {MAX_SQL_CHARS} characters"
        )));
    }
    let expanded = sql_text::expand_helpers(sql, |aad, value| crypto.blind_index(aad, value))
        .map_err(|err| SqlConsoleError::Validation(err.to_string()))?;
    if expanded.replaced > 0 && !helpers_allowed {
        return Err(SqlConsoleError::Validation(
            HELPERS_NEED_PATIENT_VIEW.to_string(),
        ));
    }
    let body = sql_text::trim_trailing_semicolons(&expanded.sql);
    let body_chars = body.chars().count();
    let keyword = sql_text::leading_keyword(body);
    // ตรวจก่อนส่ง: ปล่อยไปถึง Postgres ก็ถูกตัดอยู่ดี แต่หลังครอบ LIMIT จะกลายเป็น syntax error ที่อ่านไม่รู้เรื่อง
    let blocked = if !sql_text::is_read_keyword(&keyword) {
        Some(Blocked::ReadOnly {
            keyword: Some(audit_keyword(&keyword)),
        })
    } else if sql_text::has_multiple_statements(body) {
        Some(Blocked::MultipleStatements)
    } else {
        None
    };
    let (sql, prefix_chars) = if sql_text::runs_unwrapped(&keyword) {
        (body.to_string(), 0)
    } else {
        // ขึ้นบรรทัดใหม่ก่อนวงเล็บปิด เผื่อคำสั่งจบด้วย comment `--`
        let wrapped = format!("{WRAP_PREFIX}{body}\n) as q limit {}", MAX_ROWS + 1);
        (wrapped, WRAP_PREFIX.chars().count())
    };
    Ok(Prepared {
        sql,
        prefix_chars,
        body_chars,
        helpers_replaced: expanded.replaced,
        blocked,
    })
}

/// สิ่งที่ได้จากการรันสำเร็จ — ชื่อตารางไว้บันทึก audit
struct Outcome {
    result: QueryResult,
    tables: Vec<String>,
    decrypted_cells: usize,
}

/// รัน SQL ของผู้ใช้ด้วย role อ่านอย่างเดียว แล้วบันทึก audit_log (ทั้งสำเร็จและล้มเหลว) — บันทึกไม่ได้ = ไม่ส่งผลให้
///
/// ส่วน ‘รัน + บันทึก audit’ ทำใน `tokio::spawn` แล้ว handler แค่รอผล: ถ้า handler ถูกยกเลิก (client ปิดแท็บ/ตัด connection หรือครบ
/// 10 วินาทีของ TimeoutLayer) task ยังทำต่อจนบันทึก audit — ไม่งั้นคำสั่งที่ Postgres ยังรันต่ออยู่ไม่มีแถว audit เลย (security audit
/// รอบ 2) ผู้ที่ถูกยกเลิกไม่ได้ข้อมูลกลับ จึงยังไม่มีผลลัพธ์ที่ส่งโดยไม่มี audit; ไม่ครอบคลุมกรณี process ตายกลางทาง
/// รับค่าที่ own เอง (`PgPool`/`Arc`/`Crypto` โคลนถูก) เพราะ task ต้องอยู่ได้นานกว่า request
pub async fn run(
    pool: PgPool,
    console: Option<Arc<SqlConsole>>,
    crypto: Crypto,
    actor: &CurrentUser,
    sql: &str,
    decrypt: bool,
) -> Result<QueryResult, SqlConsoleError> {
    authorize(console.as_deref(), actor)?;
    let Some(console) = console else {
        return Err(SqlConsoleError::NotConfigured);
    };
    // ถอดรหัสและตัวช่วย :hn()/:nap_no() ใช้ได้เฉพาะผู้ที่ดูข้อมูลผู้ป่วยได้อยู่แล้ว (ถอดรหัสไม่มีสิทธิ์ = เห็นเป็น hex
    // เหมือนไม่ได้ขอถอด; ตัวช่วยไม่มีสิทธิ์ = 400)
    let may_view_patients = actor.can(Resource::Patient, Action::View);
    let prepared = prepare(sql, &crypto, may_view_patients)?;
    let may_decrypt = decrypt && may_view_patients;

    let job = Job {
        pool,
        console,
        crypto,
        actor: actor.user.id,
        sql_chars: sql.chars().count(),
        prepared,
        may_decrypt,
    };
    match tokio::spawn(run_and_audit(job)).await {
        Ok(result) => result,
        // task ล้ม (panic ที่ไม่ใช่ในงานแปลงผลซึ่งมี JoinError ของตัวเอง) — error ธรรมดา ไม่ใช่การข้ามการตอบ
        Err(_) => Err(SqlConsoleError::QueryFailed(QueryFailure {
            message: "the query could not be completed".to_string(),
            code: None,
            position: None,
        })),
    }
}

/// ทุกอย่างที่ task ‘รัน + บันทึก audit’ ต้องใช้ (own ทั้งหมด)
struct Job {
    pool: PgPool,
    console: Arc<SqlConsole>,
    crypto: Crypto,
    actor: UserId,
    /// ความยาวของ SQL ที่ผู้ใช้พิมพ์ (ตัวอักษร) — เก็บใน audit
    sql_chars: usize,
    prepared: Prepared,
    may_decrypt: bool,
}

async fn run_and_audit(job: Job) -> Result<QueryResult, SqlConsoleError> {
    let Job {
        pool,
        console,
        crypto,
        actor,
        sql_chars,
        prepared,
        may_decrypt,
    } = job;

    let started = Instant::now();
    let outcome = match &prepared.blocked {
        Some(blocked) => Err(SqlConsoleError::Blocked(blocked.clone())),
        None => execute(&console, &crypto, &prepared, may_decrypt).await,
    };
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);

    let sql_sha256: String = Sha256::digest(prepared.sql.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let metadata = match &outcome {
        Ok(outcome) => json!({
            "sqlSha256": sql_sha256,
            "sqlLength": sql_chars,
            "status": "ok",
            "rows": outcome.result.rows.len(),
            "truncated": outcome.result.truncated,
            "tables": outcome.tables,
            "decryptedCells": outcome.decrypted_cells,
            "durationMs": duration_ms,
        }),
        Err(SqlConsoleError::Blocked(blocked)) => json!({
            "sqlSha256": sql_sha256,
            "sqlLength": sql_chars,
            "status": "blocked",
            "reason": blocked.reason(),
            "keyword": blocked.keyword(),
            "rows": 0,
            "durationMs": duration_ms,
        }),
        Err(err) => {
            let code = match err {
                SqlConsoleError::QueryFailed(failure) => failure.code.clone(),
                _ => None,
            };
            if let Some(code) = &code {
                tracing::info!(sqlstate = %code, "sql console query rejected by database");
            }
            json!({
                "sqlSha256": sql_sha256,
                "sqlLength": sql_chars,
                "status": "error",
                "errorCode": code,
                "rows": 0,
                "durationMs": duration_ms,
            })
        }
    };
    repository::insert_audit(&pool, actor, metadata).await?;

    let mut outcome = outcome?;
    outcome.result.duration_ms = duration_ms;
    Ok(outcome.result)
}

async fn execute(
    console: &SqlConsole,
    crypto: &Crypto,
    prepared: &Prepared,
    may_decrypt: bool,
) -> Result<Outcome, SqlConsoleError> {
    let mut conn = repository::connect(console)
        .await
        .map_err(|err| to_failure(err, prepared))?;
    let result = execute_on(&mut conn, console, crypto, prepared, may_decrypt).await;
    repository::close(conn).await;
    result
}

async fn execute_on(
    conn: &mut sqlx::PgConnection,
    console: &SqlConsole,
    crypto: &Crypto,
    prepared: &Prepared,
    may_decrypt: bool,
) -> Result<Outcome, SqlConsoleError> {
    let names: Vec<String> = patient_crypto::ENCRYPTED_COLUMNS
        .iter()
        .map(|(name, _)| name.to_string())
        .collect();
    let encrypted = repository::encrypted_columns(conn, patient_crypto::TABLE, &names).await?;
    let limits = repository::ResultLimits {
        max_bytes: MAX_RESULT_BYTES,
        max_columns: MAX_COLUMNS,
    };
    let executed = repository::execute(conn, &prepared.sql, console.statement_timeout(), limits)
        .await
        .map_err(|err| to_failure(err, prepared))?;

    let oids: Vec<i64> = executed
        .columns
        .iter()
        .filter_map(|column| column.relation_id())
        .map(|oid| i64::from(oid.0))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let tables = repository::table_names(conn, &oids).await?;

    // แปลงผลลัพธ์/ถอดรหัสเป็นงาน CPU ต่อเนื่อง (ไม่มี await) — รันบน blocking thread ไม่ยึด async worker ที่ใช้ร่วมกัน
    // และถ้า panic (JoinError) ก็เป็น error ธรรมดาที่ยังถูกบันทึก audit ไม่ใช่ข้ามการบันทึก
    let crypto = crypto.clone();
    let decrypt_attempts = console.decrypt_attempts();
    let (result, decrypted_cells) = tokio::task::spawn_blocking(move || {
        convert(executed, &encrypted, &crypto, may_decrypt, decrypt_attempts)
    })
    .await
    .map_err(|_| {
        SqlConsoleError::QueryFailed(QueryFailure {
            message: "cannot read the result — cast the columns to ::text".to_string(),
            code: None,
            position: None,
        })
    })??;
    Ok(Outcome {
        result,
        tables,
        decrypted_cells,
    })
}

/// แปลงแถวเป็น JSON และถอดรหัส (เมื่อ `may_decrypt`) คืนผลลัพธ์ + จำนวนช่องที่ถอดได้
/// - คอลัมน์ที่มาจากคอลัมน์เข้ารหัสจริง: จับคู่ OID ของตาราง + attnum (alias ไม่มีผล) ถอดด้วย AAD ของคอลัมน์นั้น
///   ถอดไม่ได้ = null + นับใน decrypt_errors
/// - ค่าอื่น (expression เช่น concat/cast/coalesce/union/json ที่ไม่มีต้นทาง): หา ciphertext ที่ฝังอยู่ (embedded.rs)
fn convert(
    executed: Executed,
    encrypted: &[EncryptedColumn],
    crypto: &Crypto,
    may_decrypt: bool,
    decrypt_attempts: usize,
) -> Result<(QueryResult, usize), SqlConsoleError> {
    let aad_by_origin: HashMap<(u32, i16), &'static str> = encrypted
        .iter()
        .filter_map(|column| {
            patient_crypto::ENCRYPTED_COLUMNS
                .iter()
                .find(|(name, _)| *name == column.name)
                .map(|(_, aad)| ((column.table_oid, column.attnum), *aad))
        })
        .collect();

    struct Plan {
        kind: ValueKind,
        aad: Option<&'static str>,
    }
    let mut columns = Vec::with_capacity(executed.columns.len());
    let mut plans = Vec::with_capacity(executed.columns.len());
    for column in &executed.columns {
        let kind = ValueKind::of(column.type_info());
        let origin = column.relation_id().zip(column.relation_attribute_no());
        let aad = origin
            .and_then(|(oid, attnum)| aad_by_origin.get(&(oid.0, attnum)).copied())
            .filter(|_| kind == ValueKind::Bytea);
        let decrypted = aad.is_some() && may_decrypt;
        columns.push(ResultColumn {
            name: column.name().to_string(),
            type_name: column.type_info().name().to_string(),
            supported: kind.is_supported(),
            encrypted: aad.is_some(),
            decrypted,
        });
        plans.push(Plan {
            kind,
            aad: aad.filter(|_| decrypted),
        });
    }

    let mut rows = executed.rows;
    let truncated = rows.len() > MAX_ROWS;
    rows.truncate(MAX_ROWS);

    // ลอง AAD ของทุกคอลัมน์ผู้ป่วย — ใช้กับค่าที่ไม่มีต้นทาง (GCM ตรวจ AAD ถอดผิดคอลัมน์ไม่ผ่านอยู่แล้ว)
    let decrypt_any = |blob: &[u8]| {
        patient_crypto::ENCRYPTED_COLUMNS
            .iter()
            .find_map(|(_, aad)| crypto.decrypt(aad, blob).ok())
    };
    // งบการลองถอดของทั้งคำขอ (ทุกเซลล์ใช้ร่วมกัน; ค่าปริยาย embedded::MAX_DECRYPT_ATTEMPTS ตั้งใหม่ได้ที่ SqlConsole)
    let budget = embedded::Budget::new(decrypt_attempts);
    // งบหน่วยความจำของ JSON ทั้งคำขอ — ช่องที่ขยายใหญ่ (numeric/bytea/array) จ่ายก่อนจอง
    let mut json_budget = value::Budget::new(MAX_RESULT_BYTES);
    let mut embedded_columns = vec![false; plans.len()];
    let mut decrypted_cells = 0;
    let mut decrypt_errors = 0;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut cells = Vec::with_capacity(plans.len());
        for (index, plan) in plans.iter().enumerate() {
            let raw = row.try_get_raw(index).map_err(RepositoryError::from)?;
            let cell = match plan.aad {
                Some(aad) if !raw.is_null() => {
                    let blob = raw.as_bytes().unwrap_or_default();
                    match crypto.decrypt(aad, blob) {
                        Ok(plain) => {
                            decrypted_cells += 1;
                            Value::String(plain)
                        }
                        Err(_) => {
                            decrypt_errors += 1;
                            Value::Null
                        }
                    }
                }
                _ => {
                    let mut cell =
                        value::to_json(raw, plan.kind, &mut json_budget).map_err(|err| {
                            if err.downcast_ref::<value::ResultTooLarge>().is_some() {
                                return result_too_large();
                            }
                            let column = columns.get(index);
                            SqlConsoleError::QueryFailed(QueryFailure {
                                message: format!(
                                    "cannot read column \"{}\" of type {} — cast it to ::text",
                                    column.map_or("", |c| c.name.as_str()),
                                    column.map_or("", |c| c.type_name.as_str())
                                ),
                                code: None,
                                position: None,
                            })
                        })?;
                    if may_decrypt && embedded::decrypt_in_value(&mut cell, &decrypt_any, &budget) {
                        decrypted_cells += 1;
                        embedded_columns[index] = true;
                    }
                    cell
                }
            };
            cells.push(cell);
        }
        out.push(cells);
    }
    for (column, found) in columns.iter_mut().zip(embedded_columns) {
        column.decrypted |= found;
    }

    Ok((
        QueryResult {
            columns,
            rows: out,
            truncated,
            duration_ms: 0,
            decrypt_errors,
        },
        decrypted_cells,
    ))
}

/// ผลลัพธ์ใหญ่เกินเพดาน (400 พร้อมข้อความคงที่ — ไม่มีค่าจากผลลัพธ์ปน; ยังบันทึก audit status `error`)
fn result_too_large() -> SqlConsoleError {
    SqlConsoleError::QueryFailed(QueryFailure {
        message: RESULT_TOO_LARGE.to_string(),
        code: None,
        position: None,
    })
}

/// error จาก Postgres (คำสั่งผิด, ไม่มีสิทธิ์, หมดเวลา, connection เต็ม) → ส่งข้อความให้ผู้ใช้ — อื่น ๆ เป็นปัญหาของระบบ
fn to_failure(err: RepositoryError, prepared: &Prepared) -> SqlConsoleError {
    if matches!(err, RepositoryError::ResultTooLarge) {
        return result_too_large();
    }
    let RepositoryError::Database(sqlx::Error::Database(db)) = err else {
        return SqlConsoleError::Repository(err);
    };
    let pg = db.try_downcast_ref::<PgDatabaseError>();
    let mut message = db.message().to_string();
    if let Some(hint) = pg.and_then(PgDatabaseError::hint) {
        message.push_str("\nHINT: ");
        message.push_str(hint);
    }
    let position = match pg.and_then(PgDatabaseError::position) {
        Some(PgErrorPosition::Original(position)) => user_position(position, prepared),
        _ => None,
    };
    let code = db.code().map(|code| code.into_owned());
    if let Some(blocked) = read_only_violation(code.as_deref(), db.message()) {
        return SqlConsoleError::Blocked(blocked);
    }
    SqlConsoleError::QueryFailed(QueryFailure {
        message,
        code,
        position,
    })
}

/// error ของ Postgres ที่แปลว่า "คำสั่งนี้แก้ข้อมูล" แม้คำแรกจะผ่านด่านตรวจ (select … for update, explain analyze delete,
/// WITH ที่มี DELETE ข้างใน, select … into) — ให้หน้าเว็บเตือนแบบเดียวกับ INSERT/DELETE ตรง ๆ
/// เทียบข้อความภาษาอังกฤษของ Postgres (lc_messages ค่าเริ่มต้นของ image) ถ้าไม่ตรงก็แค่แสดงเป็น error ปกติ
fn read_only_violation(code: Option<&str>, message: &str) -> Option<Blocked> {
    match code? {
        // cannot execute DELETE in a read-only transaction
        "25006" => Some(Blocked::ReadOnly {
            keyword: message
                .strip_prefix("cannot execute ")
                .and_then(|rest| rest.strip_suffix(" in a read-only transaction"))
                .map(str::to_string),
        }),
        // WITH clause containing a data-modifying statement must be at the top level
        "0A000" if message.contains("data-modifying statement") => {
            Some(Blocked::ReadOnly { keyword: None })
        }
        // SELECT ... INTO is not allowed here (สร้างตารางใหม่)
        "42601" if message.starts_with("SELECT ... INTO") => Some(Blocked::ReadOnly {
            keyword: Some("SELECT INTO".to_string()),
        }),
        _ => None,
    }
}

/// ตำแหน่งในคำสั่งที่ส่งจริง → ตำแหน่งในข้อความที่ผู้ใช้พิมพ์ (แทนตัวช่วยแล้วความยาวเปลี่ยน = ไม่บอกตำแหน่ง)
fn user_position(position: usize, prepared: &Prepared) -> Option<usize> {
    if prepared.helpers_replaced > 0 {
        return None;
    }
    let position = position.checked_sub(prepared.prefix_chars)?;
    (1..=prepared.body_chars)
        .contains(&position)
        .then_some(position)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crypto() -> Crypto {
        Crypto::from_hex_keys(&"a".repeat(64), &"b".repeat(64)).unwrap()
    }

    #[test]
    fn queries_are_wrapped_with_a_row_limit_but_explain_is_not() {
        let prepared = prepare("select 1;\n", &crypto(), true).unwrap();
        assert_eq!(prepared.sql, "select * from (\nselect 1\n) as q limit 501");
        assert_eq!(prepared.prefix_chars, 16);

        let explain = prepare("explain select 1", &crypto(), true).unwrap();
        assert_eq!(explain.sql, "explain select 1");
        assert_eq!(explain.prefix_chars, 0);
    }

    #[test]
    fn empty_or_oversized_sql_is_rejected() {
        assert!(matches!(
            prepare("  \n", &crypto(), true),
            Err(SqlConsoleError::Validation(_))
        ));
        let long = format!("select '{}'", "x".repeat(MAX_SQL_CHARS));
        assert!(matches!(
            prepare(&long, &crypto(), true),
            Err(SqlConsoleError::Validation(_))
        ));
    }

    #[test]
    fn error_positions_map_back_to_the_typed_text() {
        let prepared = prepare("select nope", &crypto(), true).unwrap();
        // Postgres นับจากคำสั่งที่ครอบแล้ว: "select * from (\n" 16 ตัว + ตำแหน่งในข้อความเดิม
        assert_eq!(user_position(16 + 8, &prepared), Some(8));
        assert_eq!(user_position(3, &prepared), None);

        let with_helper = prepare("select :hn('1') x", &crypto(), true).unwrap();
        assert_eq!(user_position(20, &with_helper), None);
    }

    #[test]
    fn encrypted_columns_are_recognised_by_table_and_name() {
        assert!(is_encrypted_column("patient", "hn"));
        assert!(!is_encrypted_column("patient", "hn_idx"));
        assert!(!is_encrypted_column("users", "hn"));
    }

    #[test]
    fn write_statements_are_blocked_before_running() {
        let blocked = |sql: &str| prepare(sql, &crypto(), true).unwrap().blocked;
        assert!(matches!(
            blocked("  insert into x values (1);"),
            Some(Blocked::ReadOnly { keyword: Some(k) }) if k == "INSERT"
        ));
        assert!(matches!(
            blocked("-- note\ndelete from x"),
            Some(Blocked::ReadOnly { keyword: Some(k) }) if k == "DELETE"
        ));
        assert!(matches!(
            blocked("select 1; drop table x"),
            Some(Blocked::MultipleStatements)
        ));
        assert!(blocked("select ';' as semi;").is_none());
        assert!(blocked("with x as (select 1) select * from x").is_none());
    }

    #[test]
    fn postgres_read_only_errors_become_warnings() {
        let keyword = |code, message| read_only_violation(Some(code), message);
        assert!(matches!(
            keyword("25006", "cannot execute SELECT FOR UPDATE in a read-only transaction"),
            Some(Blocked::ReadOnly { keyword: Some(k) }) if k == "SELECT FOR UPDATE"
        ));
        assert!(matches!(
            keyword(
                "0A000",
                "WITH clause containing a data-modifying statement must be at the top level"
            ),
            Some(Blocked::ReadOnly { keyword: None })
        ));
        assert!(matches!(
            keyword("42601", "SELECT ... INTO is not allowed here"),
            Some(Blocked::ReadOnly { keyword: Some(k) }) if k == "SELECT INTO"
        ));
        assert!(keyword("42601", "syntax error at or near \"frm\"").is_none());
        assert!(keyword("42501", "permission denied for table sessions").is_none());
    }
}
