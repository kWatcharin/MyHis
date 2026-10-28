//! แกน ledger ที่ทุกเอกสารคลังใช้ร่วม: หา/สร้างล็อต (พร้อมแถวคงเหลือ), ลง movement, เปลี่ยนคงเหลือ
//! ทุกฟังก์ชันทำงานในทรานแซกชันของเอกสารผู้เรียก — ผู้เรียกต้องล็อกแถว item ก่อน (สร้างล็อตของรายการเดียวกันซ้อนไม่ได้)
//! และคงเหลือของล็อตล็อกด้วย `FOR UPDATE` ก่อนอ่านเสมอ (กันรับ/เบิก/ยกเลิกพร้อมกันแล้วยอดเพี้ยน)
//!
//! invariant: คงเหลือของล็อต (`stock_balance.qty_on_hand`) = ผลรวม `stock_movement.qty_delta` ของล็อต — ทุกที่ที่ลง
//! movement ต้องเรียก `add_to_balance` ด้วยจำนวนเดียวกันในทรานแซกชันเดียวกัน

use chrono::NaiveDate;
use sqlx::{Postgres, Transaction};

use super::RepositoryError;
use crate::domain::{ItemId, StockLotId, UserId};

/// ชนิดของ movement — ตรงกับ CHECK `stock_movement_type_check` (ขั้นเบิก/ปรับยอดเพิ่มชนิดของตัวเอง)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementType {
    /// ยอดยกมา (ใบรับประเภท opening)
    Opening,
    /// รับเข้า
    Receive,
    /// กลับรายการ movement เดิม (ยกเลิกเอกสาร) — จำนวนตรงข้ามกับแถวเดิม
    Reversal,
}

impl MovementType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Receive => "receive",
            Self::Reversal => "reversal",
        }
    }
}

/// ล็อตที่ล็อกแถวคงเหลือไว้แล้วในทรานแซกชันนี้
#[derive(Debug)]
pub(super) struct LockedLot {
    pub id: StockLotId,
    pub expiry_date: Option<NaiveDate>,
    pub qty_on_hand: i32,
}

/// ล็อตของรายการตามเลขล็อต (ไม่สนตัวพิมพ์ — None = ล็อตไม่ระบุ) ไม่มีก็สร้างพร้อมคงเหลือ 0 แล้วล็อกแถวคงเหลือ
/// ล็อตที่สร้างใหม่ได้วันหมดอายุ `expiry_date`; ล็อตเดิมคืนวันหมดอายุที่เก็บไว้ (ผู้เรียกตัดสินเองว่าตรงไหม)
pub(super) async fn lock_or_create_lot(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    item_id: ItemId,
    lot_no: Option<&str>,
    expiry_date: Option<NaiveDate>,
) -> Result<LockedLot, RepositoryError> {
    let created = sqlx::query_scalar!(
        r#"insert into stock_lot (item_id, lot_no, expiry_date, created_by, updated_by)
           values ($1, $2, $3, $4, $4)
           on conflict (item_id, lower(lot_no)) do nothing
           returning id as "id: StockLotId""#,
        item_id.0,
        lot_no,
        expiry_date,
        actor.0
    )
    .fetch_optional(&mut **tx)
    .await?;
    if let Some(id) = created {
        sqlx::query!("insert into stock_balance (lot_id) values ($1)", id.0)
            .execute(&mut **tx)
            .await?;
        return Ok(LockedLot {
            id,
            expiry_date,
            qty_on_hand: 0,
        });
    }

    let row = sqlx::query_as!(
        LockedLot,
        r#"select l.id as "id: StockLotId", l.expiry_date, b.qty_on_hand
           from stock_lot l
           join stock_balance b on b.lot_id = l.id
           where l.item_id = $1
             and ((l.lot_no is null and $2::text is null) or lower(l.lot_no) = lower($2))
           for update of b"#,
        item_id.0,
        lot_no
    )
    .fetch_one(&mut **tx)
    .await?;
    Ok(row)
}

/// เปลี่ยนวันหมดอายุของล็อต — ใช้กับล็อตที่คงเหลือ 0 เท่านั้น (ผู้เรียกตรวจ) เช่น ยกเลิกใบที่กรอกวันผิดแล้วรับใหม่
pub(super) async fn set_lot_expiry(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    lot_id: StockLotId,
    expiry_date: Option<NaiveDate>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "update stock_lot set expiry_date = $2, updated_by = $3, updated_at = now() where id = $1",
        lot_id.0,
        expiry_date,
        actor.0
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// คงเหลือของหลายล็อตพร้อมล็อกแถว (เรียงตาม lot_id กัน deadlock ระหว่างทรานแซกชันที่ล็อกหลายล็อต)
pub(super) async fn lock_balances(
    tx: &mut Transaction<'_, Postgres>,
    lot_ids: &[i32],
) -> Result<Vec<(StockLotId, i32)>, RepositoryError> {
    let rows = sqlx::query!(
        r#"select lot_id as "lot_id: StockLotId", qty_on_hand
           from stock_balance
           where lot_id = any($1)
           order by lot_id
           for update"#,
        lot_ids
    )
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| (r.lot_id, r.qty_on_hand))
        .collect())
}

/// ลง movement ของบรรทัดใบรับ (`reverses` = id ของ movement ที่กลับรายการ — เฉพาะ `Reversal`)
pub(super) async fn insert_receipt_movement(
    tx: &mut Transaction<'_, Postgres>,
    actor: UserId,
    lot_id: StockLotId,
    movement_type: MovementType,
    qty_delta: i32,
    receipt_line_id: i32,
    reverses: Option<i32>,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "insert into stock_movement
           (lot_id, movement_type, qty_delta, receipt_line_id, reverses_id, created_by)
         values ($1, $2, $3, $4, $5, $6)",
        lot_id.0,
        movement_type.as_str(),
        qty_delta,
        receipt_line_id,
        reverses,
        actor.0
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// เปลี่ยนคงเหลือของล็อตด้วยจำนวนเดียวกับ movement ที่เพิ่งลง — ติดลบไม่ได้ (CHECK) ผู้เรียกตรวจหลังล็อกแล้ว
pub(super) async fn add_to_balance(
    tx: &mut Transaction<'_, Postgres>,
    lot_id: StockLotId,
    qty_delta: i32,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        "update stock_balance set qty_on_hand = qty_on_hand + $2, updated_at = now() where lot_id = $1",
        lot_id.0,
        qty_delta
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
