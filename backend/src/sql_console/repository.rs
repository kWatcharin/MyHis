//! การเข้าถึงฐานข้อมูลของหน้า SQL — ห้ามรู้เรื่อง HTTP
//!
//! SQL ของผู้ใช้รันผ่าน connection ของ role อ่านอย่างเดียว (`SqlConsole`) ที่เปิดใหม่ทุกครั้งแล้วปิด ใน transaction
//! READ ONLY ที่ตั้ง timeout ด้วย SET LOCAL ส่วน audit_log เขียนผ่าน pool หลักของแอป (role อ่านอย่างเดียวเขียนไม่ได้)
//!
//! `sqlx::query(sql)` แบบไม่ตรวจตอนคอมไพล์ใช้เฉพาะ SQL ของผู้ใช้ (หน้าที่ของโมดูลนี้โดยตรง) query อื่นใช้ `query!`

use std::time::Duration;

use serde_json::Value;
use sqlx::postgres::{PgColumn, PgConnection, PgRow};
use sqlx::{Connection, Executor, PgPool, Statement};

use crate::domain::UserId;
use crate::sql_console::setup::SqlConsole;

/// รอ connect ได้ไม่เกินนี้ (ต่อ DB ในเครื่องเดียวกัน ปกติไม่กี่มิลลิวินาที)
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
/// รอ lock ของแถว/ตารางไม่เกินนี้ — คำสั่งอ่านไม่ควรต้องรอใคร
const LOCK_TIMEOUT: &str = "1s";

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("could not connect to the database in time")]
    ConnectTimeout,
}

/// ผลที่ได้จาก Postgres ก่อนแปลงค่า — `columns` มาจาก prepare (มีแม้ไม่มีแถว) รวม OID/attnum ต้นทางของคอลัมน์
pub struct Executed {
    pub columns: Vec<PgColumn>,
    pub rows: Vec<PgRow>,
}

/// คอลัมน์ที่เข้ารหัสของตาราง patient ในฐานข้อมูลนี้ (OID ของตาราง + attnum) — ค้นใหม่ทุกครั้ง ไม่ cache
pub struct EncryptedColumn {
    pub table_oid: u32,
    pub attnum: i16,
    pub name: String,
}

pub struct SchemaColumn {
    pub table: String,
    pub column: String,
    pub data_type: String,
}

pub async fn connect(console: &SqlConsole) -> Result<PgConnection, RepositoryError> {
    tokio::time::timeout(
        CONNECT_TIMEOUT,
        PgConnection::connect_with(console.options()),
    )
    .await
    .map_err(|_| RepositoryError::ConnectTimeout)?
    .map_err(RepositoryError::from)
}

/// ปิด connection หลังใช้ — ปิดไม่สำเร็จก็ไม่เป็นไร (ถูกทิ้งอยู่ดี และ transaction ไม่มีอะไรให้ commit)
pub async fn close(conn: PgConnection) {
    let _ = conn.close().await;
}

/// รันคำสั่งเดียว (extended protocol ปฏิเสธข้อความที่มีหลายคำสั่งเอง) ใน transaction READ ONLY แล้ว rollback
/// ผู้เรียกจำกัดจำนวนแถวด้วยการครอบ LIMIT ไว้แล้ว (sql_text.rs)
pub async fn execute(
    conn: &mut PgConnection,
    sql: &str,
    statement_timeout: Duration,
) -> Result<Executed, RepositoryError> {
    let mut tx = conn.begin().await?;
    sqlx::query!("set transaction read only")
        .execute(&mut *tx)
        .await?;
    let timeout = format!("{}ms", statement_timeout.as_millis());
    // set_config(..., true) = SET LOCAL (มีผลเฉพาะ transaction นี้) แต่รับค่าผ่าน bind ได้
    sqlx::query!(
        r#"select set_config('statement_timeout', $1, true) as "statement_timeout!",
                  set_config('lock_timeout', $2, true) as "lock_timeout!""#,
        timeout,
        LOCK_TIMEOUT
    )
    .fetch_one(&mut *tx)
    .await?;

    let statement = (&mut *tx).prepare(sql).await?;
    let columns = statement.columns().to_vec();
    let rows = sqlx::query(sql)
        .persistent(false)
        .fetch_all(&mut *tx)
        .await?;

    // อ่านอย่างเดียว ไม่มีอะไรต้องเก็บ — rollback ไม่สำเร็จ (เช่น ผู้ใช้สั่ง COMMIT เอง) ไม่ทำให้ผลเปลี่ยน
    let _ = tx.rollback().await;
    Ok(Executed { columns, rows })
}

pub async fn encrypted_columns(
    conn: &mut PgConnection,
    table: &str,
    columns: &[String],
) -> Result<Vec<EncryptedColumn>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select c.oid::int8 as "table_oid!", a.attnum, a.attname::text as "name!"
           from pg_class c
           join pg_namespace n on n.oid = c.relnamespace
           join pg_attribute a on a.attrelid = c.oid
           where n.nspname = 'public' and c.relname::text = $1 and a.attname::text = any($2::text[])
             and not a.attisdropped"#,
        table,
        columns
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let table_oid = u32::try_from(row.table_oid).ok()?;
            Some(EncryptedColumn {
                table_oid,
                attnum: row.attnum,
                name: row.name,
            })
        })
        .collect())
}

/// ชื่อตารางของ OID ที่คอลัมน์ในผลลัพธ์มาจาก (ใช้บันทึกใน audit_log)
pub async fn table_names(
    conn: &mut PgConnection,
    oids: &[i64],
) -> Result<Vec<String>, RepositoryError> {
    Ok(sqlx::query_scalar!(
        r#"select relname::text as "name!" from pg_class where oid::int8 = any($1) order by relname"#,
        oids
    )
    .fetch_all(&mut *conn)
    .await?)
}

/// คอลัมน์ที่ role อ่านได้ (information_schema แสดงเฉพาะคอลัมน์ที่ผู้ใช้ปัจจุบันมีสิทธิ์) — ใช้ทำ auto complete
pub async fn readable_columns(
    conn: &mut PgConnection,
) -> Result<Vec<SchemaColumn>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select c.table_name::text as "table!", c.column_name::text as "column!",
                  c.data_type::text as "data_type!"
           from information_schema.columns c
           where c.table_schema = 'public'
           order by c.table_name, c.ordinal_position"#
    )
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| SchemaColumn {
            table: row.table,
            column: row.column,
            data_type: row.data_type,
        })
        .collect())
}

/// บันทึกการรันหนึ่งครั้ง — metadata ไม่มีตัว SQL หรือค่าจากผลลัพธ์ (service เป็นคนประกอบ)
pub async fn insert_audit(
    pool: &PgPool,
    actor: UserId,
    metadata: Value,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, metadata)
         values ($1, 'sql_executed', 'sql', $2)",
        actor.0,
        metadata
    )
    .execute(pool)
    .await?;
    Ok(())
}
