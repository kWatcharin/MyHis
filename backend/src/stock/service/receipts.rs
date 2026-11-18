//! ใบรับเข้า + ยอดยกมา (สิทธิ์ `stock_receipt.*` — `delete` = ยกเลิกใบ): ตรวจปีงบ/วันที่รับ, รายการ/หน่วย/จำนวน,
//! ล็อต/วันหมดอายุตามธงติดตามของรายการ (`ItemTracking`) แล้วให้ repository ลงคลังในทรานแซกชันเดียว
//!
//! บันทึกแล้วลงคลังทันที — แก้ได้แค่แหล่งที่มา/เลขอ้างอิง/หมายเหตุ บรรทัด/วันที่/ประเภทผิดต้องยกเลิกทั้งใบแล้วทำใหม่
//! (ledger เพิ่มได้อย่างเดียว)

use std::collections::{HashMap, HashSet};

use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

use super::{clean_optional, clean_required, conflict, require, validation, StockError};
use crate::auth::CurrentUser;
use crate::domain::item_tracking::ItemTracking;
use crate::domain::{Action, ItemId, Resource};
use crate::stock::repository::{
    self, BudgetYearOptionRow, CancelOutcome, ConversionLookup, ConversionOptionRow, CreateOutcome,
    HeaderOutcome, HeaderWrite, ItemLookup, ItemOptionRow, LineWrite, LotRow, ReceiptLineRow,
    ReceiptRow, ReceiptType, ReceiptWrite,
};

/// บรรทัดต่อใบ — กันคำขอที่ส่งรายการมาไม่จำกัด (ใบรับจริงไม่กี่สิบบรรทัด)
const MAX_LINES_PER_RECEIPT: usize = 100;

/// จำนวนหน่วยฐานต่อบรรทัดสูงสุด — กันพิมพ์ศูนย์เกินและกันคงเหลือสะสมเกินช่วง integer ของ DB
const MAX_BASE_QUANTITY: i32 = 10_000_000;

/// ตรงกับความยาวคอลัมน์ใน migration stock_receipts
const MAX_SOURCE_CHARS: usize = 200;
const MAX_REF_NO_CHARS: usize = 50;
const MAX_NOTE_CHARS: usize = 500;
const MAX_LOT_NO_CHARS: usize = 50;
const MAX_REASON_CHARS: usize = 500;

fn require_receipt(actor: &CurrentUser, action: Action) -> Result<(), StockError> {
    require(actor, Resource::StockReceipt, action)
}

/// ใบรับหนึ่งใบพร้อมบรรทัด (เรียงตามลำดับที่กรอก)
#[derive(Debug)]
pub struct ReceiptView {
    pub receipt: ReceiptRow,
    pub lines: Vec<ReceiptLineRow>,
}

pub async fn list(pool: &PgPool, actor: &CurrentUser) -> Result<Vec<ReceiptRow>, StockError> {
    require_receipt(actor, Action::View)?;
    Ok(repository::fetch_receipts(pool, None).await?)
}

pub async fn get(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
) -> Result<ReceiptView, StockError> {
    require_receipt(actor, Action::View)?;
    let receipt = repository::fetch_receipts(pool, Some(public_id))
        .await?
        .into_iter()
        .next()
        .ok_or(StockError::NotFound)?;
    let lines = repository::receipt_lines(pool, receipt.id).await?;
    Ok(ReceiptView { receipt, lines })
}

/// รายการที่ใช้งานอยู่หนึ่งรายการพร้อมหน่วยแปลง (เรียงตาม factor)
#[derive(Debug)]
pub struct ItemOptionView {
    pub item: ItemOptionRow,
    pub conversions: Vec<ConversionOptionRow>,
}

#[derive(Debug)]
pub struct ReceiptOptionsView {
    pub items: Vec<ItemOptionView>,
    pub budget_years: Vec<BudgetYearOptionRow>,
    pub sources: Vec<String>,
}

/// ตัวเลือกของฟอร์มใบรับ: รายการ/หน่วย/ธงติดตาม, ปีงบที่ใช้งานอยู่, แหล่งที่มาที่เคยกรอก — สิทธิ์ดูใบรับพอ
/// (ไม่ต้องมีสิทธิ์หน้าตั้งค่ารายการ แบบ `/items/options`)
pub async fn options(pool: &PgPool, actor: &CurrentUser) -> Result<ReceiptOptionsView, StockError> {
    require_receipt(actor, Action::View)?;
    let options = repository::list_options(pool).await?;
    let mut by_item: HashMap<ItemId, Vec<ConversionOptionRow>> =
        HashMap::with_capacity(options.items.len());
    for conversion in options.conversions {
        by_item
            .entry(conversion.item_id)
            .or_default()
            .push(conversion);
    }
    Ok(ReceiptOptionsView {
        items: options
            .items
            .into_iter()
            .map(|item| ItemOptionView {
                conversions: by_item.remove(&item.id).unwrap_or_default(),
                item,
            })
            .collect(),
        budget_years: options.budget_years,
        sources: options.sources,
    })
}

/// ล็อตที่มีเลขของรายการ (เติมช่อง lot/วันหมดอายุในฟอร์ม) — รายการที่ไม่พบ = ไม่มีล็อต
pub async fn lots(
    pool: &PgPool,
    actor: &CurrentUser,
    item_id: Uuid,
) -> Result<Vec<LotRow>, StockError> {
    require_receipt(actor, Action::View)?;
    Ok(repository::list_item_lots(pool, item_id).await?)
}

/// บรรทัดที่ผู้ใช้กรอก (uuid ยังไม่ resolve) — จำนวนเป็นหน่วยที่เลือก (`unit_id` = หน่วยฐานหรือหน่วยแปลงของรายการ)
#[derive(Debug)]
pub struct ReceiptLineInput<'a> {
    pub item_id: Uuid,
    pub unit_id: Uuid,
    pub quantity: i32,
    pub lot_no: Option<&'a str>,
    pub expiry_date: Option<NaiveDate>,
}

#[derive(Debug)]
pub struct ReceiptInput<'a> {
    pub receipt_type: ReceiptType,
    pub budget_year_id: Uuid,
    pub received_on: NaiveDate,
    pub source: Option<&'a str>,
    pub ref_no: Option<&'a str>,
    pub note: Option<&'a str>,
    pub lines: Vec<ReceiptLineInput<'a>>,
}

/// ข้อความหัวใบที่แก้ได้ — ทั้งสามช่องแทนที่ค่าเดิม (ว่าง/None = ล้าง)
#[derive(Debug)]
pub struct HeaderInput<'a> {
    pub source: Option<&'a str>,
    pub ref_no: Option<&'a str>,
    pub note: Option<&'a str>,
}

#[derive(Debug)]
pub struct CreatedReceipt {
    pub public_id: Uuid,
    pub doc_no: String,
}

fn clean_header(
    source: Option<&str>,
    ref_no: Option<&str>,
    note: Option<&str>,
) -> Result<HeaderWrite, StockError> {
    Ok(HeaderWrite {
        source: clean_optional(source, "source", MAX_SOURCE_CHARS)?,
        ref_no: clean_optional(ref_no, "ref no", MAX_REF_NO_CHARS)?,
        note: clean_optional(note, "note", MAX_NOTE_CHARS)?,
    })
}

/// ตรวจบรรทัดหนึ่งกับรายการ/หน่วยแปลงที่ค้นมาแล้ว → `LineWrite` (factor ณ ตอนนี้, ล็อต/วันหมดอายุตามธงติดตาม)
fn prepare_line(
    line: &ReceiptLineInput<'_>,
    items: &[ItemLookup],
    conversions: &[ConversionLookup],
    received_on: NaiveDate,
) -> Result<LineWrite, StockError> {
    let item = items
        .iter()
        .find(|item| item.public_id == line.item_id)
        .ok_or_else(|| validation("unknown item"))?;
    if !item.is_active {
        return Err(validation("item is inactive"));
    }

    let (unit_id, unit_factor) = if line.unit_id == item.base_unit_public_id {
        (item.base_unit_id, 1)
    } else {
        let conversion = conversions
            .iter()
            .find(|c| c.item_id == item.id && c.unit_public_id == line.unit_id)
            .ok_or_else(|| validation("unit is not a unit of this item"))?;
        (conversion.unit_id, conversion.factor)
    };
    if line.quantity <= 0 {
        return Err(validation("quantity must be greater than 0"));
    }
    line.quantity
        .checked_mul(unit_factor)
        .filter(|base| *base <= MAX_BASE_QUANTITY)
        .ok_or_else(|| validation("quantity is too large"))?;

    // DB CHECK item_expiry_requires_lot กันสถานะที่ไม่มีจริงไว้แล้ว
    let tracking = ItemTracking::from_flags(item.track_lot, item.track_expiry)
        .ok_or_else(|| validation("expiry tracking requires lot tracking"))?;
    let lot_no = clean_optional(line.lot_no, "lot number", MAX_LOT_NO_CHARS)?;
    let (lot_no, expiry_date) = match tracking {
        ItemTracking::Untracked => {
            if lot_no.is_some() {
                return Err(validation("item does not track lots"));
            }
            if line.expiry_date.is_some() {
                return Err(validation("item does not track expiry"));
            }
            (None, None)
        }
        ItemTracking::Lot => {
            let lot_no = lot_no.ok_or_else(|| validation("lot number is required"))?;
            if line.expiry_date.is_some() {
                return Err(validation("item does not track expiry"));
            }
            (Some(lot_no), None)
        }
        ItemTracking::LotAndExpiry => {
            let lot_no = lot_no.ok_or_else(|| validation("lot number is required"))?;
            let expiry_date = line
                .expiry_date
                .ok_or_else(|| validation("expiry date is required"))?;
            if expiry_date < received_on {
                return Err(validation("item is already expired"));
            }
            (Some(lot_no), Some(expiry_date))
        }
    };

    Ok(LineWrite {
        item_id: item.id,
        tracking,
        base_unit_id: item.base_unit_id,
        unit_id,
        unit_factor,
        quantity: line.quantity,
        lot_no,
        expiry_date,
    })
}

pub async fn create(
    pool: &PgPool,
    actor: &CurrentUser,
    input: ReceiptInput<'_>,
) -> Result<CreatedReceipt, StockError> {
    require_receipt(actor, Action::Create)?;
    if input.lines.is_empty() {
        return Err(validation("at least one line is required"));
    }
    if input.lines.len() > MAX_LINES_PER_RECEIPT {
        return Err(validation("too many lines"));
    }
    let header = clean_header(input.source, input.ref_no, input.note)?;

    let budget_year = repository::find_budget_year(pool, input.budget_year_id)
        .await?
        .ok_or_else(|| validation("unknown budget year"))?;
    if !budget_year.is_active {
        return Err(validation("budget year is inactive"));
    }
    if input.received_on < budget_year.start_date || input.received_on > budget_year.end_date {
        return Err(validation("received date is outside the budget year"));
    }
    if input.received_on > repository::thai_today(pool).await? {
        return Err(validation("received date must not be in the future"));
    }

    let item_public_ids: Vec<Uuid> = input.lines.iter().map(|line| line.item_id).collect();
    let items = repository::find_items(pool, &item_public_ids).await?;
    let item_ids: Vec<i32> = items.iter().map(|item| item.id.0).collect();
    let conversions = repository::find_conversions(pool, &item_ids).await?;
    let lines = input
        .lines
        .iter()
        .map(|line| prepare_line(line, &items, &conversions, input.received_on))
        .collect::<Result<Vec<_>, _>>()?;
    // ล็อตเดียวกัน = รายการเดียวกัน + เลขล็อตเดียวกันไม่สนตัวพิมพ์ (หรือไม่ระบุล็อตทั้งคู่) — ตรงกับ unique ของ stock_lot
    // lowercase ครั้งเดียวต่อบรรทัด แทนการเทียบทุกคู่ (docs/memory-optimization.md ข้อ 5.6)
    let mut seen_lots = HashSet::with_capacity(lines.len());
    for line in &lines {
        let lot = (line.item_id, line.lot_no.as_deref().map(str::to_lowercase));
        if !seen_lots.insert(lot) {
            return Err(validation("duplicate lot in receipt"));
        }
    }

    let write = ReceiptWrite {
        receipt_type: input.receipt_type,
        budget_year_id: budget_year.id,
        budget_year: budget_year.year,
        received_on: input.received_on,
        header,
        lines,
    };
    match repository::insert_receipt(pool, actor.user.id, &write).await? {
        CreateOutcome::Created { public_id, doc_no } => Ok(CreatedReceipt { public_id, doc_no }),
        CreateOutcome::ItemChanged => Err(conflict("item changed while saving, please try again")),
        CreateOutcome::LotExpiryMismatch => Err(validation(
            "expiry date does not match the existing lot that still has stock",
        )),
    }
}

pub async fn update_header(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    input: HeaderInput<'_>,
) -> Result<(), StockError> {
    require_receipt(actor, Action::Update)?;
    let header = clean_header(input.source, input.ref_no, input.note)?;
    let id = repository::find_receipt_id(pool, public_id)
        .await?
        .ok_or(StockError::NotFound)?;
    match repository::update_receipt_header(pool, actor.user.id, id, &header).await? {
        HeaderOutcome::Updated => Ok(()),
        HeaderOutcome::Cancelled => Err(conflict("receipt is cancelled")),
    }
}

/// ยกเลิกใบรับ (สิทธิ์ `stock_receipt.delete`) — คืนยอดทุกบรรทัดด้วย movement กลับรายการ ต้องมีเหตุผล
pub async fn cancel(
    pool: &PgPool,
    actor: &CurrentUser,
    public_id: Uuid,
    reason: &str,
) -> Result<(), StockError> {
    require_receipt(actor, Action::Delete)?;
    let reason = clean_required(reason, "cancel reason", MAX_REASON_CHARS)?;
    let id = repository::find_receipt_id(pool, public_id)
        .await?
        .ok_or(StockError::NotFound)?;
    match repository::cancel_receipt(pool, actor.user.id, id, &reason).await? {
        CancelOutcome::Cancelled => Ok(()),
        CancelOutcome::AlreadyCancelled => Err(conflict("receipt is already cancelled")),
        CancelOutcome::StockIssued => {
            Err(conflict("stock from this receipt has already been issued"))
        }
    }
}
