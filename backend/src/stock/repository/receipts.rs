//! query ของใบรับเข้า (stock_receipt + stock_receipt_line) — บันทึกแล้วลงคลังทันที, แก้ได้แค่ข้อความหัวใบ,
//! ยกเลิก = ลง movement กลับรายการทุกบรรทัด (ไม่ลบแถว) ทุกการเขียนเขียน `audit_log` ในทรานแซกชันเดียวกัน
//!
//! ลำดับการล็อก (กัน deadlock ระหว่างรับเข้ากับยกเลิก): แถว item ของทุกบรรทัด (เรียงตาม id) → แถวคงเหลือของล็อต

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::ledger::{self, MovementType};
use super::RepositoryError;
use crate::domain::item_tracking::ItemTracking;
use crate::domain::{
    changes, BudgetYearId, ItemId, ItemUnitId, StockLotId, StockReceiptId, UserId,
};

/// ประเภทใบรับ — ตรงกับ CHECK `stock_receipt_type_check` ค่าใน API เป็นชื่อเดียวกับใน DB ("receive" / "opening")
/// จึงใช้ชนิดเดียวกันทั้งสองฝั่ง (ค่าปิดตาย ไม่มีคอลัมน์อื่นรั่วออก API)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReceiptType {
    /// รับเข้าจริง
    Receive,
    /// ยอดยกมา — ของที่มีอยู่แล้วตอนเริ่มใช้ระบบ (movement ชนิด opening)
    Opening,
}

impl ReceiptType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Receive => "receive",
            Self::Opening => "opening",
        }
    }

    fn from_db(value: &str) -> Result<Self, RepositoryError> {
        match value {
            "receive" => Ok(Self::Receive),
            "opening" => Ok(Self::Opening),
            _ => Err(RepositoryError::InvalidValue("stock_receipt.receipt_type")),
        }
    }

    fn movement_type(self) -> MovementType {
        match self {
            Self::Receive => MovementType::Receive,
            Self::Opening => MovementType::Opening,
        }
    }
}

/// หัวใบรับหนึ่งใบ + จำนวนบรรทัด + ชื่อผู้รับ/ผู้ยกเลิก
#[derive(Debug, Clone)]
pub struct ReceiptRow {
    pub id: StockReceiptId,
    /// uuid ของแถว — id ที่ API เห็น
    pub public_id: Uuid,
    pub doc_no: String,
    pub budget_year_public_id: Uuid,
    pub budget_year: String,
    pub receipt_type: ReceiptType,
    pub received_on: NaiveDate,
    pub source: Option<String>,
    pub ref_no: Option<String>,
    pub note: Option<String>,
    pub line_count: i64,
    pub created_by_name: String,
    pub created_at: DateTime<Utc>,
    pub cancelled_by_name: Option<String>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancel_reason: Option<String>,
}

/// ใบรับทุกใบ (`public_id` = None) หรือใบเดียว เรียงวันที่รับใหม่→เก่า
pub async fn fetch_receipts(
    pool: &PgPool,
    public_id: Option<Uuid>,
) -> Result<Vec<ReceiptRow>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select r.id as "id: StockReceiptId", r.uuid as public_id, r.doc_no,
                  b.uuid as budget_year_public_id, b.year as budget_year, r.receipt_type,
                  r.received_on, r.source, r.ref_no, r.note,
                  (select count(*) from stock_receipt_line l where l.receipt_id = r.id) as "line_count!",
                  cu.display_name as created_by_name, r.created_at,
                  xu.display_name as "cancelled_by_name?", r.cancelled_at, r.cancel_reason
           from stock_receipt r
           join budget_year b on b.id = r.budget_year_id
           join users cu on cu.id = r.created_by
           left join users xu on xu.id = r.cancelled_by
           where $1::uuid is null or r.uuid = $1
           order by r.received_on desc, r.id desc"#,
        public_id
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|r| -> Result<ReceiptRow, RepositoryError> {
            Ok(ReceiptRow {
                id: r.id,
                public_id: r.public_id,
                doc_no: r.doc_no,
                budget_year_public_id: r.budget_year_public_id,
                budget_year: r.budget_year,
                receipt_type: ReceiptType::from_db(&r.receipt_type)?,
                received_on: r.received_on,
                source: r.source,
                ref_no: r.ref_no,
                note: r.note,
                line_count: r.line_count,
                created_by_name: r.created_by_name,
                created_at: r.created_at,
                cancelled_by_name: r.cancelled_by_name,
                cancelled_at: r.cancelled_at,
                cancel_reason: r.cancel_reason,
            })
        })
        .collect()
}

/// บรรทัดหนึ่งของใบรับ — รายการ/ล็อต/หน่วยเป็นชื่อแล้ว
#[derive(Debug, Clone)]
pub struct ReceiptLineRow {
    pub item_public_id: Uuid,
    pub item_name: String,
    pub lot_no: Option<String>,
    pub expiry_date: Option<NaiveDate>,
    pub unit_name: String,
    pub unit_factor: i32,
    pub quantity: i32,
    pub base_quantity: i32,
    pub base_unit_name: String,
}

pub async fn receipt_lines(
    pool: &PgPool,
    id: StockReceiptId,
) -> Result<Vec<ReceiptLineRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        ReceiptLineRow,
        r#"select i.uuid as item_public_id, i.name as item_name, sl.lot_no, sl.expiry_date,
                  u.name as unit_name, rl.unit_factor, rl.quantity, rl.base_quantity,
                  bu.name as base_unit_name
           from stock_receipt_line rl
           join stock_lot sl on sl.id = rl.lot_id
           join item i on i.id = sl.item_id
           join item_unit u on u.id = rl.unit_id
           join item_unit bu on bu.id = i.base_unit_id
           where rl.receipt_id = $1
           order by rl.sort_order, rl.id"#,
        id.0
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn find_receipt_id(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<StockReceiptId>, RepositoryError> {
    let id = sqlx::query_scalar!(
        r#"select id as "id: StockReceiptId" from stock_receipt where uuid = $1"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(id)
}

/// ปีงบที่ยังไม่ถูกลบ — service ตรวจสถานะและช่วงวัน
#[derive(Debug)]
pub struct BudgetYearLookup {
    pub id: BudgetYearId,
    pub year: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: bool,
}

pub async fn find_budget_year(
    pool: &PgPool,
    public_id: Uuid,
) -> Result<Option<BudgetYearLookup>, RepositoryError> {
    let row = sqlx::query_as!(
        BudgetYearLookup,
        r#"select id as "id: BudgetYearId", year, start_date, end_date, is_active
           from budget_year
           where uuid = $1 and deleted_at is null"#,
        public_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// รายการที่ยังไม่ถูกลบ พร้อมธงติดตามและหน่วยฐาน — ตัวที่ไม่พบจะไม่อยู่ในผลลัพธ์
#[derive(Debug)]
pub struct ItemLookup {
    pub id: ItemId,
    pub public_id: Uuid,
    pub is_active: bool,
    pub track_lot: bool,
    pub track_expiry: bool,
    pub base_unit_id: ItemUnitId,
    pub base_unit_public_id: Uuid,
}

pub async fn find_items(
    pool: &PgPool,
    public_ids: &[Uuid],
) -> Result<Vec<ItemLookup>, RepositoryError> {
    let rows = sqlx::query_as!(
        ItemLookup,
        r#"select i.id as "id: ItemId", i.uuid as public_id, i.is_active, i.track_lot, i.track_expiry,
                  i.base_unit_id as "base_unit_id: ItemUnitId", u.uuid as base_unit_public_id
           from item i
           join item_unit u on u.id = i.base_unit_id
           where i.uuid = any($1) and i.deleted_at is null"#,
        public_ids
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// หน่วยแปลงที่ยังไม่ถูกลบของรายการ (factor ปัจจุบัน — เก็บลงบรรทัดใบรับ)
#[derive(Debug)]
pub struct ConversionLookup {
    pub item_id: ItemId,
    pub unit_id: ItemUnitId,
    pub unit_public_id: Uuid,
    pub factor: i32,
}

pub async fn find_conversions(
    pool: &PgPool,
    item_ids: &[i32],
) -> Result<Vec<ConversionLookup>, RepositoryError> {
    let rows = sqlx::query_as!(
        ConversionLookup,
        r#"select cv.item_id as "item_id: ItemId", cv.unit_id as "unit_id: ItemUnitId",
                  u.uuid as unit_public_id, cv.factor
           from item_unit_conversion cv
           join item_unit u on u.id = cv.unit_id
           where cv.item_id = any($1) and cv.deleted_at is null"#,
        item_ids
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// ข้อความหัวใบที่แก้ได้ (trim แล้ว ค่าว่าง = None)
#[derive(Debug)]
pub struct HeaderWrite {
    pub source: Option<String>,
    pub ref_no: Option<String>,
    pub note: Option<String>,
}

/// บรรทัดที่ service ตรวจและ resolve id แล้ว — `tracking`/`base_unit_id`/`unit_factor` คือค่าที่ตรวจไว้
/// ตรวจซ้ำหลังล็อกแถว item (กันแก้รายการแข่งกับการบันทึก)
#[derive(Debug)]
pub struct LineWrite {
    pub item_id: ItemId,
    pub tracking: ItemTracking,
    pub base_unit_id: ItemUnitId,
    pub unit_id: ItemUnitId,
    /// หน่วยฐานต่อ 1 หน่วยที่กรอก (หน่วยฐาน = 1)
    pub unit_factor: i32,
    pub quantity: i32,
    /// None = ล็อตไม่ระบุ (รายการที่ไม่ติดตามล็อต)
    pub lot_no: Option<String>,
    pub expiry_date: Option<NaiveDate>,
}

#[derive(Debug)]
pub struct ReceiptWrite {
    pub receipt_type: ReceiptType,
    pub budget_year_id: BudgetYearId,
    /// ปีงบ (พ.ศ.) สำหรับเลขที่ใบ
    pub budget_year: String,
    pub received_on: NaiveDate,
    pub header: HeaderWrite,
    pub lines: Vec<LineWrite>,
}

#[derive(Debug)]
pub enum CreateOutcome {
    Created {
        public_id: Uuid,
        doc_no: String,
    },
    /// รายการ/หน่วยแปลงถูกแก้ระหว่างตรวจกับบันทึก (ไม่ได้บันทึกอะไร)
    ItemChanged,
    /// ล็อตเดิมที่ยังมีของ มีวันหมดอายุไม่ตรงกับที่กรอก (ไม่ได้บันทึกอะไร — ฟอร์มแสดงวันของล็อตเดิมจาก `/stock/lots`)
    LotExpiryMismatch,
}

/// รันเลขที่ใบ → หัวใบ → ต่อบรรทัด: ล็อต (หา/สร้าง) → บรรทัด → movement → คงเหลือ → audit ในทรานแซกชันเดียว
/// ผลที่ไม่ใช่ `Created` = ทรานแซกชันถูกทิ้ง (rollback เลขที่ใบด้วย จึงไม่มีเลขโดด)
pub async fn insert_receipt(
    pool: &PgPool,
    actor: UserId,
    w: &ReceiptWrite,
) -> Result<CreateOutcome, RepositoryError> {
    let mut tx = pool.begin().await?;
    if !lock_items_unchanged(&mut tx, &w.lines).await? {
        return Ok(CreateOutcome::ItemChanged);
    }

    let last_no = sqlx::query_scalar!(
        "insert into stock_doc_counter (doc_type, budget_year_id, last_no)
         values ('receipt', $1, 1)
         on conflict (doc_type, budget_year_id)
         do update set last_no = stock_doc_counter.last_no + 1
         returning last_no as \"last_no!\"",
        w.budget_year_id.0
    )
    .fetch_one(&mut *tx)
    .await?;
    let doc_no = format!("RC{}-{last_no:04}", w.budget_year);

    let receipt = sqlx::query!(
        r#"insert into stock_receipt
             (doc_no, budget_year_id, receipt_type, received_on, source, ref_no, note, created_by, updated_by)
           values ($1, $2, $3, $4, $5, $6, $7, $8, $8)
           returning id as "id: StockReceiptId", uuid"#,
        doc_no,
        w.budget_year_id.0,
        w.receipt_type.as_str(),
        w.received_on,
        w.header.source,
        w.header.ref_no,
        w.header.note,
        actor.0
    )
    .fetch_one(&mut *tx)
    .await?;

    let movement_type = w.receipt_type.movement_type();
    for (sort_order, line) in (0_i32..).zip(&w.lines) {
        let lot = ledger::lock_or_create_lot(
            &mut tx,
            actor,
            line.item_id,
            line.lot_no.as_deref(),
            line.expiry_date,
        )
        .await?;
        if lot.expiry_date != line.expiry_date {
            if lot.qty_on_hand > 0 {
                return Ok(CreateOutcome::LotExpiryMismatch);
            }
            // ล็อตที่ของหมดแล้วไม่มีของที่ถือวันหมดอายุเดิม — เช่น ยกเลิกใบที่กรอกวันผิดแล้วรับใหม่
            ledger::set_lot_expiry(&mut tx, actor, lot.id, line.expiry_date).await?;
        }

        let inserted = sqlx::query!(
            "insert into stock_receipt_line
               (receipt_id, lot_id, unit_id, unit_factor, quantity, sort_order, created_by)
             values ($1, $2, $3, $4, $5, $6, $7)
             returning id, base_quantity",
            receipt.id.0,
            lot.id.0,
            line.unit_id.0,
            line.unit_factor,
            line.quantity,
            sort_order,
            actor.0
        )
        .fetch_one(&mut *tx)
        .await?;
        ledger::insert_receipt_movement(
            &mut tx,
            actor,
            lot.id,
            movement_type,
            inserted.base_quantity,
            inserted.id,
            None,
        )
        .await?;
        ledger::add_to_balance(&mut tx, lot.id, inserted.base_quantity).await?;
    }

    let after = snapshot(&mut tx, receipt.id).await?;
    insert_audit(
        &mut tx,
        actor,
        "stock_receipt_created",
        receipt.id,
        Some(changes::created(after)),
    )
    .await?;
    tx.commit().await?;
    Ok(CreateOutcome::Created {
        public_id: receipt.uuid,
        doc_no,
    })
}

/// ล็อกแถว item ของทุกบรรทัด (เรียงตาม id) แล้วตรวจซ้ำว่ารายการยังใช้ได้ หน่วยฐาน/ธงติดตาม/factor ยังตรงกับที่
/// service ตรวจ — การแก้รายการล็อกแถวเดียวกัน จึงเปลี่ยนหน่วยฐาน/ธงแข่งกับการรับเข้าไม่ได้
async fn lock_items_unchanged(
    tx: &mut Transaction<'_, Postgres>,
    lines: &[LineWrite],
) -> Result<bool, RepositoryError> {
    let mut item_ids: Vec<i32> = lines.iter().map(|line| line.item_id.0).collect();
    item_ids.sort_unstable();
    item_ids.dedup();

    let items = sqlx::query!(
        r#"select id as "id: ItemId", base_unit_id as "base_unit_id: ItemUnitId", track_lot,
                  track_expiry, (is_active and deleted_at is null) as "usable!"
           from item
           where id = any($1)
           order by id
           for update"#,
        &item_ids
    )
    .fetch_all(&mut **tx)
    .await?;
    let factors = sqlx::query!(
        r#"select item_id as "item_id: ItemId", unit_id as "unit_id: ItemUnitId", factor
           from item_unit_conversion
           where item_id = any($1) and deleted_at is null"#,
        &item_ids
    )
    .fetch_all(&mut **tx)
    .await?;

    Ok(lines.iter().all(|line| {
        let item_ok = items.iter().any(|item| {
            item.id == line.item_id
                && item.usable
                && item.base_unit_id == line.base_unit_id
                && ItemTracking::from_flags(item.track_lot, item.track_expiry)
                    == Some(line.tracking)
        });
        let factor_ok = if line.unit_id == line.base_unit_id {
            line.unit_factor == 1
        } else {
            factors.iter().any(|f| {
                f.item_id == line.item_id
                    && f.unit_id == line.unit_id
                    && f.factor == line.unit_factor
            })
        };
        item_ok && factor_ok
    }))
}

#[derive(Debug)]
pub enum HeaderOutcome {
    Updated,
    /// ใบที่ยกเลิกแล้วแก้ไม่ได้
    Cancelled,
}

/// แก้ข้อความหัวใบ (แหล่งที่มา/เลขอ้างอิง/หมายเหตุ) + audit — บรรทัด/วันที่/ประเภทแก้ไม่ได้ (ผิด = ยกเลิกแล้วทำใหม่)
pub async fn update_receipt_header(
    pool: &PgPool,
    actor: UserId,
    id: StockReceiptId,
    h: &HeaderWrite,
) -> Result<HeaderOutcome, RepositoryError> {
    let mut tx = pool.begin().await?;
    let before = snapshot(&mut tx, id).await?;
    if is_cancelled(&mut tx, id).await? {
        return Ok(HeaderOutcome::Cancelled);
    }
    sqlx::query!(
        "update stock_receipt
         set source = $3, ref_no = $4, note = $5, updated_by = $2, updated_at = now()
         where id = $1",
        id.0,
        actor.0,
        h.source,
        h.ref_no,
        h.note
    )
    .execute(&mut *tx)
    .await?;
    let after = snapshot(&mut tx, id).await?;
    let details = changes::updated(&before, &after);
    insert_audit(&mut tx, actor, "stock_receipt_updated", id, Some(details)).await?;
    tx.commit().await?;
    Ok(HeaderOutcome::Updated)
}

#[derive(Debug)]
pub enum CancelOutcome {
    Cancelled,
    AlreadyCancelled,
    /// คงเหลือของบางล็อตน้อยกว่าที่ใบนี้รับเข้า (ถูกเบิกไปแล้ว) — ยกเลิกไม่ได้ทั้งใบ
    StockIssued,
}

/// ยกเลิกใบรับ: ล็อกรายการ → ล็อกคงเหลือ → ตรวจว่าพอคืน → movement กลับรายการทุกบรรทัด → ตั้ง cancelled_* → audit
pub async fn cancel_receipt(
    pool: &PgPool,
    actor: UserId,
    id: StockReceiptId,
    reason: &str,
) -> Result<CancelOutcome, RepositoryError> {
    let mut tx = pool.begin().await?;
    let before = snapshot(&mut tx, id).await?;
    if is_cancelled(&mut tx, id).await? {
        return Ok(CancelOutcome::AlreadyCancelled);
    }

    // ล็อกแถว item ก่อนคงเหลือ — ลำดับเดียวกับการรับเข้า
    sqlx::query!(
        "select i.id from item i
         where i.id in (select sl.item_id from stock_receipt_line rl
                        join stock_lot sl on sl.id = rl.lot_id
                        where rl.receipt_id = $1)
         order by i.id
         for update",
        id.0
    )
    .fetch_all(&mut *tx)
    .await?;
    let movements = sqlx::query!(
        r#"select m.id, m.lot_id as "lot_id: StockLotId", m.qty_delta,
                  m.receipt_line_id as "receipt_line_id!"
           from stock_movement m
           join stock_receipt_line rl on rl.id = m.receipt_line_id
           where rl.receipt_id = $1 and m.movement_type <> 'reversal'
           order by m.lot_id"#,
        id.0
    )
    .fetch_all(&mut *tx)
    .await?;
    let lot_ids: Vec<i32> = movements.iter().map(|m| m.lot_id.0).collect();
    let balances = ledger::lock_balances(&mut tx, &lot_ids).await?;
    let enough = movements.iter().all(|m| {
        balances
            .iter()
            .any(|(lot_id, qty_on_hand)| *lot_id == m.lot_id && *qty_on_hand >= m.qty_delta)
    });
    if !enough {
        return Ok(CancelOutcome::StockIssued);
    }

    for m in &movements {
        ledger::insert_receipt_movement(
            &mut tx,
            actor,
            m.lot_id,
            MovementType::Reversal,
            -m.qty_delta,
            m.receipt_line_id,
            Some(m.id),
        )
        .await?;
        ledger::add_to_balance(&mut tx, m.lot_id, -m.qty_delta).await?;
    }
    sqlx::query!(
        "update stock_receipt
         set cancelled_by = $2, cancelled_at = now(), cancel_reason = $3,
             updated_by = $2, updated_at = now()
         where id = $1",
        id.0,
        actor.0,
        reason
    )
    .execute(&mut *tx)
    .await?;
    let after = snapshot(&mut tx, id).await?;
    let details = changes::updated(&before, &after);
    insert_audit(&mut tx, actor, "stock_receipt_cancelled", id, Some(details)).await?;
    tx.commit().await?;
    Ok(CancelOutcome::Cancelled)
}

async fn is_cancelled(
    tx: &mut Transaction<'_, Postgres>,
    id: StockReceiptId,
) -> Result<bool, RepositoryError> {
    let cancelled = sqlx::query_scalar!(
        r#"select cancelled_at is not null as "cancelled!" from stock_receipt where id = $1"#,
        id.0
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(cancelled)
}

/// รายการที่ใช้งานอยู่สำหรับฟอร์มใบรับ (id ภายในใช้จับคู่หน่วยแปลง ไม่ออก API)
#[derive(Debug, Clone)]
pub struct ItemOptionRow {
    pub id: ItemId,
    pub public_id: Uuid,
    pub name: String,
    pub category_name: String,
    pub base_unit_public_id: Uuid,
    pub base_unit_name: String,
    pub track_lot: bool,
    pub track_expiry: bool,
}

#[derive(Debug, Clone)]
pub struct ConversionOptionRow {
    pub item_id: ItemId,
    pub unit_public_id: Uuid,
    pub unit_name: String,
    pub factor: i32,
}

#[derive(Debug, Clone)]
pub struct BudgetYearOptionRow {
    pub public_id: Uuid,
    pub year: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[derive(Debug)]
pub struct ReceiptOptions {
    pub items: Vec<ItemOptionRow>,
    pub conversions: Vec<ConversionOptionRow>,
    pub budget_years: Vec<BudgetYearOptionRow>,
    /// แหล่งที่มาที่เคยกรอก (ใช้ล่าสุดก่อน) — ฟอร์มเสนอให้เลือกเพื่อสะกดตรงกัน
    pub sources: Vec<String>,
}

/// แหล่งที่มาที่เสนอในฟอร์มสูงสุดกี่ค่า
const MAX_SOURCE_OPTIONS: i64 = 50;

pub async fn list_options(pool: &PgPool) -> Result<ReceiptOptions, RepositoryError> {
    let items = sqlx::query_as!(
        ItemOptionRow,
        r#"select i.id as "id: ItemId", i.uuid as public_id, i.name, c.name as category_name,
                  u.uuid as base_unit_public_id, u.name as base_unit_name, i.track_lot, i.track_expiry
           from item i
           join item_category c on c.id = i.category_id
           join item_unit u on u.id = i.base_unit_id
           where i.deleted_at is null and i.is_active
           order by c.sort_order, c.name, i.name"#
    )
    .fetch_all(pool);
    let conversions = sqlx::query_as!(
        ConversionOptionRow,
        r#"select cv.item_id as "item_id: ItemId", u.uuid as unit_public_id, u.name as unit_name,
                  cv.factor
           from item_unit_conversion cv
           join item i on i.id = cv.item_id
           join item_unit u on u.id = cv.unit_id
           where cv.deleted_at is null and i.deleted_at is null and i.is_active
           order by cv.item_id, cv.factor, u.name"#
    )
    .fetch_all(pool);
    let budget_years = sqlx::query_as!(
        BudgetYearOptionRow,
        r#"select uuid as public_id, year, start_date, end_date
           from budget_year
           where deleted_at is null and is_active
           order by start_date desc"#
    )
    .fetch_all(pool);
    let sources = sqlx::query_scalar!(
        r#"select source as "source!"
           from stock_receipt
           where source is not null
           group by source
           order by max(created_at) desc
           limit $1"#,
        MAX_SOURCE_OPTIONS
    )
    .fetch_all(pool);
    let (items, conversions, budget_years, sources) =
        tokio::try_join!(items, conversions, budget_years, sources)?;
    Ok(ReceiptOptions {
        items,
        conversions,
        budget_years,
        sources,
    })
}

/// ล็อตที่มีเลขของรายการหนึ่ง (เติมช่อง lot/วันหมดอายุในฟอร์ม) — มีของก่อน แล้วหมดอายุก่อน
#[derive(Debug, Clone)]
pub struct LotRow {
    pub lot_no: String,
    pub expiry_date: Option<NaiveDate>,
    pub qty_on_hand: i32,
}

pub async fn list_item_lots(
    pool: &PgPool,
    item_public_id: Uuid,
) -> Result<Vec<LotRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        LotRow,
        r#"select sl.lot_no as "lot_no!", sl.expiry_date, b.qty_on_hand
           from stock_lot sl
           join stock_balance b on b.lot_id = sl.id
           join item i on i.id = sl.item_id
           where i.uuid = $1 and i.deleted_at is null and sl.lot_no is not null
           order by b.qty_on_hand > 0 desc, sl.expiry_date nulls last, sl.id"#,
        item_public_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// audit ของใบรับ (หน้า "ประวัติการใช้งาน") — target_id = uuid ของใบ ชื่อรายการ = เลขที่ใบ (join ตอนแสดงผล)
async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    action: &str,
    id: StockReceiptId,
    details: Option<Value>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into audit_log (actor_user_id, action, target_type, target_id, metadata)
         select $1, $2, 'stock_receipt', uuid::text, $4 from stock_receipt where id = $3",
        actor.0,
        action,
        id.0,
        details
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// ค่าของใบสำหรับรายละเอียดในประวัติการใช้งาน — ประเภทเป็นคำไทย, บรรทัดเป็นข้อความ เช่น
/// "พาราเซตามอล 500 mg · lot A123 · หมด 31/03/2570 · 2 กล่อง (= 200 เม็ด)" (วันหมดอายุ พ.ศ.)
/// ล็อกแถวใบไว้จนจบทรานแซกชัน ไม่พบแถว = null
async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    id: StockReceiptId,
) -> Result<Value, RepositoryError> {
    let row = sqlx::query_scalar!(
        r#"select jsonb_build_object(
                  'doc_no', r.doc_no,
                  'receipt_type', case r.receipt_type when 'opening' then 'ยอดยกมา' else 'รับเข้า' end,
                  'budget_year', b.year,
                  'received_on', r.received_on,
                  'source', r.source, 'ref_no', r.ref_no, 'note', r.note,
                  'lines', coalesce((
                      select jsonb_agg(
                          i.name
                          || coalesce(' · lot ' || sl.lot_no, '')
                          || coalesce(' · หมด ' || to_char(sl.expiry_date, 'DD/MM/')
                                      || (extract(year from sl.expiry_date)::int + 543), '')
                          || ' · ' || rl.quantity || ' ' || u.name
                          || case when rl.unit_factor > 1
                                  then ' (= ' || rl.base_quantity || ' ' || bu.name || ')' else '' end
                          order by rl.sort_order, rl.id)
                      from stock_receipt_line rl
                      join stock_lot sl on sl.id = rl.lot_id
                      join item i on i.id = sl.item_id
                      join item_unit u on u.id = rl.unit_id
                      join item_unit bu on bu.id = i.base_unit_id
                      where rl.receipt_id = r.id
                  ), '[]'::jsonb),
                  'cancel_reason', r.cancel_reason
              ) as "snapshot!"
           from stock_receipt r
           join budget_year b on b.id = r.budget_year_id
           where r.id = $1
           for update of r"#,
        id.0
    )
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.unwrap_or(Value::Null))
}
