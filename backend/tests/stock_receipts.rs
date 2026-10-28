//! เทสระดับ service ของใบรับเข้า + ยอดยกมา + ledger (docs/inventory-design.md ข้อ 10): Σ ledger = คงเหลือ เสมอ,
//! แปลงหน่วย (รับเป็นกล่อง = หน่วยฐาน), เลขที่ใบรันต่อปีงบ, วันที่รับ, ล็อต/วันหมดอายุตามธงติดตามของรายการ,
//! ยกเลิก = กลับรายการ (movement เดิมไม่ถูกแก้), ledger แก้/ลบไม่ได้, สิทธิ์ราย action และกฎของ master รายการ
//! ที่บังคับเมื่อมีการเคลื่อนไหวแล้ว — ทุกเทส seed ปีงบที่ครอบวันนี้ (เวลาไทย) + หมวด/หน่วย/รายการก่อน

mod common;

use chrono::{Datelike, Days, NaiveDate};
use common::*;
use my_his_backend::audit::service as audit_service;
use my_his_backend::auth::CurrentUser;
use my_his_backend::domain::Resource;
use my_his_backend::item::service::{self as item_service, ConversionInput, ItemInput};
use my_his_backend::item::ItemError;
use my_his_backend::item_category::service::{self as category_service, CreateItemCategoryInput};
use my_his_backend::item_unit::service::{self as unit_service, CreateItemUnitInput};
use my_his_backend::stock::service::{self as stock, HeaderInput, ReceiptInput, ReceiptLineInput};
use my_his_backend::stock::{ReceiptType, StockError};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

/// ผู้ใช้ admin, ปีงบ 2570 ที่ครอบวันนี้, หน่วย เม็ด/กล่อง/ชิ้น และรายการสามแบบของการติดตาม
struct Fixture {
    actor: CurrentUser,
    today: NaiveDate,
    year: Uuid,
    supplies: Uuid,
    tablet: Uuid,
    boxed: Uuid,
    piece: Uuid,
    /// ติดตามล็อต + วันหมดอายุ (หมวดยา) หน่วยฐานเม็ด, 1 กล่อง = 100 เม็ด
    para: Uuid,
    /// ไม่ติดตาม (หมวดเวชภัณฑ์) หน่วยฐานชิ้น, 1 กล่อง = 50 ชิ้น
    glove: Uuid,
    /// ติดตามล็อตอย่างเดียว (หมวดเวชภัณฑ์) หน่วยฐานชิ้น
    tube: Uuid,
}

async fn thai_today(pool: &PgPool) -> NaiveDate {
    sqlx::query_scalar("select (now() at time zone 'Asia/Bangkok')::date")
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn insert_budget_year(
    pool: &PgPool,
    user_id: i32,
    year: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    is_active: bool,
) -> Uuid {
    sqlx::query_scalar(
        "insert into budget_year (year, start_date, end_date, is_active, created_by, updated_by)
         values ($1, $2, $3, $4, $5, $5) returning uuid",
    )
    .bind(year)
    .bind(start_date)
    .bind(end_date)
    .bind(is_active)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn category(name: &str, tracked: bool, sort_order: i32) -> CreateItemCategoryInput<'_> {
    CreateItemCategoryInput {
        name,
        default_track_lot: tracked,
        default_track_expiry: tracked,
        default_patient_linkable: false,
        sort_order,
        is_active: true,
    }
}

fn item(name: &str, category_id: Uuid, base_unit_id: Uuid) -> ItemInput<'_> {
    ItemInput {
        name,
        category_id,
        base_unit_id,
        track_lot: None,
        track_expiry: None,
        patient_linkable: None,
        reorder_point: None,
        is_active: true,
        conversions: Vec::new(),
    }
}

async fn fixture(pool: &PgPool) -> Fixture {
    let actor = admin(pool).await;
    let today = thai_today(pool).await;
    let year = insert_budget_year(
        pool,
        actor.user.id.0,
        "2570",
        today - Days::new(30),
        today + Days::new(300),
        true,
    )
    .await;

    category_service::create(pool, &actor, category("ยา", true, 0))
        .await
        .unwrap();
    category_service::create(pool, &actor, category("เวชภัณฑ์", false, 1))
        .await
        .unwrap();
    for name in ["เม็ด", "กล่อง", "ชิ้น"] {
        unit_service::create(
            pool,
            &actor,
            CreateItemUnitInput {
                name,
                sort_order: 0,
                is_active: true,
            },
        )
        .await
        .unwrap();
    }
    let categories = category_service::list(pool, &actor).await.unwrap();
    let category_id = |name: &str| {
        categories
            .iter()
            .find(|c| c.name == name)
            .expect("category seeded")
            .public_id
    };
    let units = unit_service::list(pool, &actor).await.unwrap();
    let unit_id = |name: &str| {
        units
            .iter()
            .find(|u| u.name == name)
            .expect("unit seeded")
            .public_id
    };
    let (drug, supplies) = (category_id("ยา"), category_id("เวชภัณฑ์"));
    let (tablet, boxed, piece) = (unit_id("เม็ด"), unit_id("กล่อง"), unit_id("ชิ้น"));

    item_service::create(
        pool,
        &actor,
        ItemInput {
            conversions: vec![ConversionInput {
                unit_id: boxed,
                factor: 100,
            }],
            ..item("พาราเซตามอล", drug, tablet)
        },
    )
    .await
    .unwrap();
    item_service::create(
        pool,
        &actor,
        ItemInput {
            conversions: vec![ConversionInput {
                unit_id: boxed,
                factor: 50,
            }],
            ..item("ถุงมือ", supplies, piece)
        },
    )
    .await
    .unwrap();
    item_service::create(
        pool,
        &actor,
        ItemInput {
            track_lot: Some(true),
            track_expiry: Some(false),
            ..item("หลอดเก็บเลือด", supplies, piece)
        },
    )
    .await
    .unwrap();
    let items = item_service::list(pool, &actor).await.unwrap();
    let item_id = |name: &str| {
        items
            .iter()
            .find(|v| v.item.name == name)
            .expect("item seeded")
            .item
            .public_id
    };

    Fixture {
        today,
        year,
        supplies,
        tablet,
        boxed,
        piece,
        para: item_id("พาราเซตามอล"),
        glove: item_id("ถุงมือ"),
        tube: item_id("หลอดเก็บเลือด"),
        actor,
    }
}

fn line(item_id: Uuid, unit_id: Uuid, quantity: i32) -> ReceiptLineInput<'static> {
    ReceiptLineInput {
        item_id,
        unit_id,
        quantity,
        lot_no: None,
        expiry_date: None,
    }
}

fn lot_line(
    item_id: Uuid,
    unit_id: Uuid,
    quantity: i32,
    lot_no: &str,
    expiry_date: Option<NaiveDate>,
) -> ReceiptLineInput<'_> {
    ReceiptLineInput {
        item_id,
        unit_id,
        quantity,
        lot_no: Some(lot_no),
        expiry_date,
    }
}

/// ใบรับเข้าวันนี้ในปีงบ 2570 ไม่มีข้อความหัวใบ
fn receipt<'a>(f: &Fixture, lines: Vec<ReceiptLineInput<'a>>) -> ReceiptInput<'a> {
    ReceiptInput {
        receipt_type: ReceiptType::Receive,
        budget_year_id: f.year,
        received_on: f.today,
        source: None,
        ref_no: None,
        note: None,
        lines,
    }
}

fn validation_of<T: std::fmt::Debug>(result: Result<T, StockError>) -> String {
    match result {
        Err(StockError::Validation(message)) => message,
        other => panic!("expected validation error, got {other:?}"),
    }
}

fn conflict_of<T: std::fmt::Debug>(result: Result<T, StockError>) -> String {
    match result {
        Err(StockError::Conflict(message)) => message,
        other => panic!("expected conflict, got {other:?}"),
    }
}

fn item_conflict_of(result: Result<(), ItemError>) -> String {
    match result {
        Err(ItemError::Conflict(message)) => message,
        other => panic!("expected conflict, got {other:?}"),
    }
}

/// คงเหลือรวมทุกล็อตของรายการ (หน่วยฐาน)
async fn on_hand(pool: &PgPool, item_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "select coalesce(sum(b.qty_on_hand), 0)::bigint
         from stock_balance b
         join stock_lot l on l.id = b.lot_id
         join item i on i.id = l.item_id
         where i.uuid = $1",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Σ qty_delta ของ ledger = qty_on_hand ทุกล็อต และทุกล็อตมีแถวคงเหลือ (เทสสำคัญที่สุดของคลัง)
async fn assert_ledger_matches_balance(pool: &PgPool) {
    let mismatched: i64 = sqlx::query_scalar(
        "select count(*) from stock_balance b
         where b.qty_on_hand <> coalesce((select sum(m.qty_delta) from stock_movement m
                                          where m.lot_id = b.lot_id), 0)",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(mismatched, 0, "ledger sum must equal stock_balance");
    let without_balance: i64 = sqlx::query_scalar(
        "select count(*) from stock_lot l
         where not exists (select 1 from stock_balance b where b.lot_id = l.id)",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(without_balance, 0, "every lot must have a balance row");
}

async fn movement_types(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("select movement_type from stock_movement order by id")
        .fetch_all(pool)
        .await
        .unwrap()
}

/// วันที่แบบที่ snapshot ของ audit ใช้ (dd/mm/พ.ศ.)
fn thai_date(date: NaiveDate) -> String {
    format!("{}/{}", date.format("%d/%m"), date.year() + 543)
}

#[sqlx::test]
async fn receipt_posts_lines_to_ledger_and_balance_in_base_units(pool: PgPool) {
    let f = fixture(&pool).await;
    let expiry = f.today + Days::new(365);
    let created = stock::create(
        &pool,
        &f.actor,
        ReceiptInput {
            source: Some(" คลังยา รพ. "),
            ref_no: Some("INV-1"),
            ..receipt(
                &f,
                vec![
                    lot_line(f.para, f.boxed, 2, "A1", Some(expiry)),
                    line(f.glove, f.piece, 5),
                ],
            )
        },
    )
    .await
    .unwrap();
    assert_eq!(created.doc_no, "RC2570-0001");

    let view = stock::get(&pool, &f.actor, created.public_id)
        .await
        .unwrap();
    assert_eq!(view.receipt.receipt_type, ReceiptType::Receive);
    assert_eq!(view.receipt.source.as_deref(), Some("คลังยา รพ."));
    assert_eq!(view.receipt.ref_no.as_deref(), Some("INV-1"));
    assert_eq!(view.receipt.line_count, 2);
    assert!(view.receipt.cancelled_at.is_none());
    let lines: Vec<_> = view
        .lines
        .iter()
        .map(|l| {
            (
                l.item_name.as_str(),
                l.lot_no.as_deref(),
                l.expiry_date,
                l.unit_name.as_str(),
                l.quantity,
                l.unit_factor,
                l.base_quantity,
                l.base_unit_name.as_str(),
            )
        })
        .collect();
    assert_eq!(
        lines,
        vec![
            (
                "พาราเซตามอล",
                Some("A1"),
                Some(expiry),
                "กล่อง",
                2,
                100,
                200,
                "เม็ด"
            ),
            ("ถุงมือ", None, None, "ชิ้น", 5, 1, 5, "ชิ้น"),
        ]
    );

    // 2 กล่อง × 100 เม็ด/กล่อง = 200 เม็ด
    assert_eq!(on_hand(&pool, f.para).await, 200);
    assert_eq!(on_hand(&pool, f.glove).await, 5);
    assert_eq!(movement_types(&pool).await, ["receive", "receive"]);
    assert_ledger_matches_balance(&pool).await;

    // ประวัติการใช้งาน: บรรทัดเป็นข้อความ (วันหมดอายุ พ.ศ.) ประเภทเป็นคำไทย
    let after: Value = sqlx::query_scalar(
        "select metadata -> 'after' from audit_log
         where action = 'stock_receipt_created' and target_type = 'stock_receipt'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after["doc_no"], "RC2570-0001");
    assert_eq!(after["receipt_type"], "รับเข้า");
    assert_eq!(after["budget_year"], "2570");
    assert_eq!(
        after["lines"],
        json!([
            format!(
                "พาราเซตามอล · lot A1 · หมด {} · 2 กล่อง (= 200 เม็ด)",
                thai_date(expiry)
            ),
            "ถุงมือ · 5 ชิ้น",
        ])
    );
    // ชื่อรายการในหน้าประวัติ = เลขที่ใบ
    let activities = audit_service::list_activities(&pool, &f.actor, f.today, f.today)
        .await
        .unwrap();
    let rows: Vec<_> = activities
        .events
        .iter()
        .filter(|e| e.target_type.as_deref() == Some("stock_receipt"))
        .map(|e| (e.action.as_str(), e.target_label.as_deref()))
        .collect();
    assert_eq!(rows, [("stock_receipt_created", Some("RC2570-0001"))]);
}

#[sqlx::test]
async fn opening_balance_and_doc_numbers_run_per_budget_year(pool: PgPool) {
    let f = fixture(&pool).await;
    let first = stock::create(
        &pool,
        &f.actor,
        ReceiptInput {
            receipt_type: ReceiptType::Opening,
            ..receipt(&f, vec![line(f.glove, f.piece, 10)])
        },
    )
    .await
    .unwrap();
    let second = stock::create(
        &pool,
        &f.actor,
        receipt(&f, vec![line(f.glove, f.boxed, 1)]),
    )
    .await
    .unwrap();
    // ปีงบอื่นที่ครอบวันเดียวกัน (ตารางปีงบไม่กันช่วงทับกัน — ฟอร์มให้เลือกปีงบเอง) เริ่มเลข 0001 ของตัวเอง
    let other_year =
        insert_budget_year(&pool, f.actor.user.id.0, "2571", f.today, f.today, true).await;
    let third = stock::create(
        &pool,
        &f.actor,
        ReceiptInput {
            budget_year_id: other_year,
            ..receipt(&f, vec![line(f.glove, f.piece, 1)])
        },
    )
    .await
    .unwrap();
    assert_eq!(
        [first.doc_no, second.doc_no, third.doc_no],
        ["RC2570-0001", "RC2570-0002", "RC2571-0001"]
    );

    // ยกมา 10 + 1 กล่อง (50 ชิ้น) + 1
    assert_eq!(on_hand(&pool, f.glove).await, 61);
    assert_eq!(
        movement_types(&pool).await,
        ["opening", "receive", "receive"]
    );
    let opening = stock::get(&pool, &f.actor, first.public_id).await.unwrap();
    assert_eq!(opening.receipt.receipt_type, ReceiptType::Opening);
    assert_ledger_matches_balance(&pool).await;
}

#[sqlx::test]
async fn received_date_must_be_within_the_budget_year_and_not_in_the_future(pool: PgPool) {
    let f = fixture(&pool).await;
    let uid = f.actor.user.id.0;
    let lines = || vec![line(f.glove, f.piece, 1)];

    assert_eq!(
        validation_of(
            stock::create(
                &pool,
                &f.actor,
                ReceiptInput {
                    received_on: f.today + Days::new(1),
                    ..receipt(&f, lines())
                },
            )
            .await
        ),
        "received date must not be in the future"
    );

    let past = insert_budget_year(
        &pool,
        uid,
        "2560",
        NaiveDate::from_ymd_opt(2016, 10, 1).unwrap(),
        NaiveDate::from_ymd_opt(2017, 9, 30).unwrap(),
        true,
    )
    .await;
    let inactive = insert_budget_year(&pool, uid, "2569", f.today, f.today, false).await;
    let deleted = insert_budget_year(&pool, uid, "2568", f.today, f.today, true).await;
    sqlx::query("update budget_year set deleted_at = now(), deleted_by = $2 where uuid = $1")
        .bind(deleted)
        .bind(uid)
        .execute(&pool)
        .await
        .unwrap();
    for (budget_year_id, message) in [
        (past, "received date is outside the budget year"),
        (inactive, "budget year is inactive"),
        (deleted, "unknown budget year"),
        (Uuid::new_v4(), "unknown budget year"),
    ] {
        assert_eq!(
            validation_of(
                stock::create(
                    &pool,
                    &f.actor,
                    ReceiptInput {
                        budget_year_id,
                        ..receipt(&f, lines())
                    },
                )
                .await
            ),
            message
        );
    }

    // บันทึกย้อนหลังในปีงบเดียวกันได้
    stock::create(
        &pool,
        &f.actor,
        ReceiptInput {
            received_on: f.today - Days::new(10),
            ..receipt(&f, lines())
        },
    )
    .await
    .unwrap();
}

#[sqlx::test]
async fn lines_are_validated_against_item_tracking_and_units(pool: PgPool) {
    let f = fixture(&pool).await;
    let expiry = Some(f.today + Days::new(30));
    let overlong_lot = "L".repeat(51);
    let cases: Vec<(ReceiptLineInput<'_>, &str)> = vec![
        (line(Uuid::new_v4(), f.piece, 1), "unknown item"),
        (
            line(f.glove, f.tablet, 1),
            "unit is not a unit of this item",
        ),
        (line(f.glove, f.piece, 0), "quantity must be greater than 0"),
        (
            line(f.glove, f.piece, -3),
            "quantity must be greater than 0",
        ),
        // 1,000,000 กล่อง × 50 = 50,000,000 ชิ้น เกินเพดานต่อบรรทัด
        (line(f.glove, f.boxed, 1_000_000), "quantity is too large"),
        (line(f.glove, f.boxed, i32::MAX), "quantity is too large"),
        (
            lot_line(f.glove, f.piece, 1, "X1", None),
            "item does not track lots",
        ),
        (
            ReceiptLineInput {
                expiry_date: expiry,
                ..line(f.glove, f.piece, 1)
            },
            "item does not track expiry",
        ),
        (line(f.tube, f.piece, 1), "lot number is required"),
        (
            lot_line(f.tube, f.piece, 1, "   ", None),
            "lot number is required",
        ),
        (
            lot_line(f.tube, f.piece, 1, "T1", expiry),
            "item does not track expiry",
        ),
        (
            lot_line(f.tube, f.piece, 1, &overlong_lot, None),
            "lot number must be at most 50 characters",
        ),
        (
            lot_line(f.para, f.tablet, 1, "A1", None),
            "expiry date is required",
        ),
        (
            lot_line(f.para, f.tablet, 1, "A1", Some(f.today - Days::new(1))),
            "item is already expired",
        ),
    ];
    for (bad_line, message) in cases {
        assert_eq!(
            validation_of(stock::create(&pool, &f.actor, receipt(&f, vec![bad_line])).await),
            message
        );
    }

    assert_eq!(
        validation_of(stock::create(&pool, &f.actor, receipt(&f, Vec::new())).await),
        "at least one line is required"
    );
    let too_many = (0..101).map(|_| line(f.glove, f.piece, 1)).collect();
    assert_eq!(
        validation_of(stock::create(&pool, &f.actor, receipt(&f, too_many)).await),
        "too many lines"
    );
    // ล็อตเดียวกันในใบเดียว (เลขล็อตไม่สนตัวพิมพ์ / ล็อตไม่ระบุของรายการเดียวกันแม้คนละหน่วย)
    for lines in [
        vec![
            lot_line(f.tube, f.piece, 1, "t1", None),
            lot_line(f.tube, f.piece, 2, "T1", None),
        ],
        vec![line(f.glove, f.piece, 1), line(f.glove, f.boxed, 1)],
    ] {
        assert_eq!(
            validation_of(stock::create(&pool, &f.actor, receipt(&f, lines)).await),
            "duplicate lot in receipt"
        );
    }
    let overlong_source = "ก".repeat(201);
    assert_eq!(
        validation_of(
            stock::create(
                &pool,
                &f.actor,
                ReceiptInput {
                    source: Some(&overlong_source),
                    ..receipt(&f, vec![line(f.glove, f.piece, 1)])
                },
            )
            .await
        ),
        "source must be at most 200 characters"
    );
    sqlx::query("update item set is_active = false where uuid = $1")
        .bind(f.glove)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        validation_of(
            stock::create(
                &pool,
                &f.actor,
                receipt(&f, vec![line(f.glove, f.piece, 1)])
            )
            .await
        ),
        "item is inactive"
    );

    // ไม่มีอะไรถูกบันทึก (รวมตัวรันเลข)
    let saved: i64 = sqlx::query_scalar(
        "select (select count(*) from stock_receipt) + (select count(*) from stock_doc_counter)
                + (select count(*) from stock_lot)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(saved, 0);
}

#[sqlx::test]
async fn existing_lot_keeps_its_expiry_while_it_has_stock(pool: PgPool) {
    let f = fixture(&pool).await;
    let first_expiry = f.today + Days::new(100);
    let other_expiry = f.today + Days::new(200);
    let first = stock::create(
        &pool,
        &f.actor,
        receipt(
            &f,
            vec![lot_line(f.para, f.tablet, 10, "A1", Some(first_expiry))],
        ),
    )
    .await
    .unwrap();

    // วันหมดอายุไม่ตรงกับล็อตที่ยังมีของ — ปฏิเสธภายในทรานแซกชัน (เลขที่ใบที่รันไปแล้วถูกคืนด้วย)
    assert_eq!(
        validation_of(
            stock::create(
                &pool,
                &f.actor,
                receipt(
                    &f,
                    vec![lot_line(f.para, f.tablet, 5, "A1", Some(other_expiry))]
                ),
            )
            .await
        ),
        "expiry date does not match the existing lot that still has stock"
    );
    // เลขล็อตไม่สนตัวพิมพ์ = ล็อตเดิม
    let second = stock::create(
        &pool,
        &f.actor,
        receipt(
            &f,
            vec![lot_line(f.para, f.boxed, 1, "a1", Some(first_expiry))],
        ),
    )
    .await
    .unwrap();
    assert_eq!(second.doc_no, "RC2570-0002");
    let lots = stock::lots(&pool, &f.actor, f.para).await.unwrap();
    let lots: Vec<_> = lots
        .iter()
        .map(|l| (l.lot_no.as_str(), l.expiry_date, l.qty_on_hand))
        .collect();
    assert_eq!(lots, vec![("A1", Some(first_expiry), 110)]);

    // ของหมดแล้ว (ยกเลิกทั้งสองใบ — เช่น กรอกวันหมดอายุผิด) รับล็อตเดิมด้วยวันใหม่ได้
    stock::cancel(&pool, &f.actor, second.public_id, "กรอกวันผิด")
        .await
        .unwrap();
    stock::cancel(&pool, &f.actor, first.public_id, "กรอกวันผิด")
        .await
        .unwrap();
    stock::create(
        &pool,
        &f.actor,
        receipt(
            &f,
            vec![lot_line(f.para, f.tablet, 3, "A1", Some(other_expiry))],
        ),
    )
    .await
    .unwrap();
    let lots = stock::lots(&pool, &f.actor, f.para).await.unwrap();
    let lots: Vec<_> = lots
        .iter()
        .map(|l| (l.lot_no.as_str(), l.expiry_date, l.qty_on_hand))
        .collect();
    assert_eq!(lots, vec![("A1", Some(other_expiry), 3)]);
    assert_ledger_matches_balance(&pool).await;
}

#[sqlx::test]
async fn cancel_reverses_every_line_and_keeps_the_original_movements(pool: PgPool) {
    let f = fixture(&pool).await;
    let expiry = f.today + Days::new(365);
    let created = stock::create(
        &pool,
        &f.actor,
        receipt(
            &f,
            vec![
                lot_line(f.para, f.boxed, 2, "A1", Some(expiry)),
                line(f.glove, f.piece, 5),
            ],
        ),
    )
    .await
    .unwrap();

    assert_eq!(
        validation_of(stock::cancel(&pool, &f.actor, created.public_id, "  ").await),
        "cancel reason is required"
    );
    stock::cancel(&pool, &f.actor, created.public_id, " นับผิด ")
        .await
        .unwrap();

    assert_eq!(on_hand(&pool, f.para).await, 0);
    assert_eq!(on_hand(&pool, f.glove).await, 0);
    let movements: Vec<(i32, String, i32, Option<i32>)> = sqlx::query_as(
        "select id, movement_type, qty_delta, reverses_id from stock_movement order by id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let (receive_para, receive_glove) = (movements[0].0, movements[1].0);
    assert_eq!(
        movements
            .iter()
            .map(|(_, kind, qty, reverses)| (kind.as_str(), *qty, *reverses))
            .collect::<Vec<_>>(),
        vec![
            ("receive", 200, None),
            ("receive", 5, None),
            ("reversal", -200, Some(receive_para)),
            ("reversal", -5, Some(receive_glove)),
        ]
    );
    assert_ledger_matches_balance(&pool).await;

    // ใบที่ยกเลิกยังอยู่ พร้อมบรรทัดเดิมและเหตุผล
    let view = stock::get(&pool, &f.actor, created.public_id)
        .await
        .unwrap();
    assert_eq!(view.receipt.cancel_reason.as_deref(), Some("นับผิด"));
    assert_eq!(view.receipt.cancelled_by_name.as_deref(), Some("Tester"));
    assert!(view.receipt.cancelled_at.is_some());
    assert_eq!(view.lines.len(), 2);

    assert_eq!(
        conflict_of(stock::cancel(&pool, &f.actor, created.public_id, "อีกครั้ง").await),
        "receipt is already cancelled"
    );
    assert_eq!(
        conflict_of(
            stock::update_header(
                &pool,
                &f.actor,
                created.public_id,
                HeaderInput {
                    source: Some("แก้"),
                    ref_no: None,
                    note: None,
                },
            )
            .await
        ),
        "receipt is cancelled"
    );

    let details: Value = sqlx::query_scalar(
        "select metadata from audit_log where action = 'stock_receipt_cancelled'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        details,
        json!({ "before": { "cancel_reason": null }, "after": { "cancel_reason": "นับผิด" } })
    );
}

#[sqlx::test]
async fn cancel_is_refused_as_a_whole_when_part_of_the_stock_is_gone(pool: PgPool) {
    let f = fixture(&pool).await;
    let created = stock::create(
        &pool,
        &f.actor,
        receipt(
            &f,
            vec![
                line(f.glove, f.piece, 5),
                lot_line(f.tube, f.piece, 3, "T1", None),
            ],
        ),
    )
    .await
    .unwrap();
    // จำลองการเบิก (ยังไม่มีเอกสารเบิก): ลดคงเหลือของล็อตตรง ๆ
    sqlx::query(
        "update stock_balance b set qty_on_hand = 2
         from stock_lot l join item i on i.id = l.item_id
         where l.id = b.lot_id and i.uuid = $1",
    )
    .bind(f.tube)
    .execute(&pool)
    .await
    .unwrap();

    assert_eq!(
        conflict_of(stock::cancel(&pool, &f.actor, created.public_id, "ยกเลิก").await),
        "stock from this receipt has already been issued"
    );
    // ไม่คืนบางส่วน
    assert_eq!(on_hand(&pool, f.glove).await, 5);
    assert_eq!(on_hand(&pool, f.tube).await, 2);
    assert_eq!(movement_types(&pool).await, ["receive", "receive"]);
    let view = stock::get(&pool, &f.actor, created.public_id)
        .await
        .unwrap();
    assert!(view.receipt.cancelled_at.is_none());
}

#[sqlx::test]
async fn ledger_rows_cannot_be_updated_deleted_or_truncated(pool: PgPool) {
    let f = fixture(&pool).await;
    stock::create(
        &pool,
        &f.actor,
        receipt(&f, vec![line(f.glove, f.piece, 5)]),
    )
    .await
    .unwrap();
    for sql in [
        "update stock_movement set qty_delta = 1",
        "delete from stock_movement",
        "truncate stock_movement",
    ] {
        let err = sqlx::query(sql).execute(&pool).await.unwrap_err();
        assert!(err.to_string().contains("append-only"), "{sql}: {err}");
    }
    assert_eq!(movement_types(&pool).await, ["receive"]);
}

#[sqlx::test]
async fn header_text_can_be_edited_and_is_audited(pool: PgPool) {
    let f = fixture(&pool).await;
    let created = stock::create(
        &pool,
        &f.actor,
        ReceiptInput {
            source: Some("คลังกลาง"),
            ref_no: Some("R1"),
            note: Some("ส่งช้า"),
            ..receipt(&f, vec![line(f.glove, f.piece, 5)])
        },
    )
    .await
    .unwrap();

    stock::update_header(
        &pool,
        &f.actor,
        created.public_id,
        HeaderInput {
            source: Some(" คลังยา "),
            ref_no: None,
            note: Some(""),
        },
    )
    .await
    .unwrap();
    let view = stock::get(&pool, &f.actor, created.public_id)
        .await
        .unwrap();
    assert_eq!(view.receipt.source.as_deref(), Some("คลังยา"));
    assert_eq!(view.receipt.ref_no, None);
    assert_eq!(view.receipt.note, None);
    // บรรทัด/ยอดไม่เปลี่ยน
    assert_eq!(on_hand(&pool, f.glove).await, 5);

    let details: Value =
        sqlx::query_scalar("select metadata from audit_log where action = 'stock_receipt_updated'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        details,
        json!({
            "before": { "source": "คลังกลาง", "ref_no": "R1", "note": "ส่งช้า" },
            "after": { "source": "คลังยา", "ref_no": null, "note": null },
        })
    );

    let overlong = "R".repeat(51);
    assert_eq!(
        validation_of(
            stock::update_header(
                &pool,
                &f.actor,
                created.public_id,
                HeaderInput {
                    source: None,
                    ref_no: Some(&overlong),
                    note: None,
                },
            )
            .await
        ),
        "ref no must be at most 50 characters"
    );
    assert!(matches!(
        stock::update_header(
            &pool,
            &f.actor,
            Uuid::new_v4(),
            HeaderInput {
                source: None,
                ref_no: None,
                note: None,
            },
        )
        .await,
        Err(StockError::NotFound)
    ));
}

#[sqlx::test]
async fn each_action_needs_its_own_permission(pool: PgPool) {
    let f = fixture(&pool).await;
    let created = stock::create(
        &pool,
        &f.actor,
        receipt(&f, vec![line(f.glove, f.piece, 5)]),
    )
    .await
    .unwrap();
    let id = created.public_id;
    let no_header = || HeaderInput {
        source: None,
        ref_no: None,
        note: None,
    };

    // สิทธิ์ของหน้าอื่นทุกตัวไม่ให้เข้าหน้านี้
    let other = actor_with(&pool, all_except(Resource::StockReceipt)).await;
    assert!(matches!(
        stock::list(&pool, &other).await,
        Err(StockError::Forbidden)
    ));
    assert!(matches!(
        stock::get(&pool, &other, id).await,
        Err(StockError::Forbidden)
    ));
    assert!(matches!(
        stock::options(&pool, &other).await,
        Err(StockError::Forbidden)
    ));
    assert!(matches!(
        stock::lots(&pool, &other, f.para).await,
        Err(StockError::Forbidden)
    ));

    let viewer = actor_with(&pool, vec![perm("stock_receipt.view")]).await;
    assert_eq!(stock::list(&pool, &viewer).await.unwrap().len(), 1);
    stock::get(&pool, &viewer, id).await.unwrap();
    stock::options(&pool, &viewer).await.unwrap();
    stock::lots(&pool, &viewer, f.tube).await.unwrap();
    assert!(matches!(
        stock::create(&pool, &viewer, receipt(&f, vec![line(f.glove, f.piece, 1)])).await,
        Err(StockError::Forbidden)
    ));
    assert!(matches!(
        stock::update_header(&pool, &viewer, id, no_header()).await,
        Err(StockError::Forbidden)
    ));
    assert!(matches!(
        stock::cancel(&pool, &viewer, id, "x").await,
        Err(StockError::Forbidden)
    ));

    let editor = actor_with(&pool, vec![perm("stock_receipt.update")]).await;
    stock::update_header(&pool, &editor, id, no_header())
        .await
        .unwrap();
    assert!(matches!(
        stock::list(&pool, &editor).await,
        Err(StockError::Forbidden)
    ));

    let canceller = actor_with(&pool, vec![perm("stock_receipt.delete")]).await;
    stock::cancel(&pool, &canceller, id, "ไม่ได้รับจริง")
        .await
        .unwrap();

    let creator = actor_with(&pool, vec![perm("stock_receipt.create")]).await;
    stock::create(
        &pool,
        &creator,
        receipt(&f, vec![line(f.glove, f.piece, 1)]),
    )
    .await
    .unwrap();

    // ไม่พบ uuid
    assert!(matches!(
        stock::get(&pool, &f.actor, Uuid::new_v4()).await,
        Err(StockError::NotFound)
    ));
    assert!(matches!(
        stock::cancel(&pool, &f.actor, Uuid::new_v4(), "x").await,
        Err(StockError::NotFound)
    ));
}

#[sqlx::test]
async fn item_base_unit_and_tracking_are_locked_once_it_has_movements(pool: PgPool) {
    let f = fixture(&pool).await;
    let tube = |base_unit_id: Uuid, track_lot: bool, reorder_point: Option<i32>| ItemInput {
        track_lot: Some(track_lot),
        track_expiry: Some(false),
        reorder_point,
        ..item("หลอดเก็บเลือด", f.supplies, base_unit_id)
    };
    let created = stock::create(
        &pool,
        &f.actor,
        receipt(&f, vec![lot_line(f.tube, f.piece, 3, "T1", None)]),
    )
    .await
    .unwrap();

    for changed in [tube(f.boxed, true, None), tube(f.piece, false, None)] {
        assert_eq!(
            item_conflict_of(item_service::replace(&pool, &f.actor, f.tube, changed).await),
            "item has stock movements"
        );
    }
    // ช่องอื่นแก้ได้ตามปกติ
    item_service::replace(&pool, &f.actor, f.tube, tube(f.piece, true, Some(5)))
        .await
        .unwrap();
    assert_eq!(
        item_conflict_of(item_service::delete(&pool, &f.actor, f.tube).await),
        "item still has stock"
    );

    // ของหมดแล้ว (ยกเลิกใบ) ลบได้ แต่ประวัติการเคลื่อนไหวยังล็อกหน่วยฐาน
    stock::cancel(&pool, &f.actor, created.public_id, "รับผิดรายการ")
        .await
        .unwrap();
    assert_eq!(
        item_conflict_of(
            item_service::replace(&pool, &f.actor, f.tube, tube(f.boxed, true, None)).await
        ),
        "item has stock movements"
    );
    item_service::delete(&pool, &f.actor, f.tube).await.unwrap();

    // รายการที่ไม่เคยมีการเคลื่อนไหวเปลี่ยนหน่วยฐานได้
    item_service::replace(
        &pool,
        &f.actor,
        f.para,
        ItemInput {
            conversions: Vec::new(),
            ..item("พาราเซตามอล", f.supplies, f.piece)
        },
    )
    .await
    .unwrap();
}

#[sqlx::test]
async fn options_list_active_items_budget_years_and_used_sources(pool: PgPool) {
    let f = fixture(&pool).await;
    insert_budget_year(&pool, f.actor.user.id.0, "2569", f.today, f.today, false).await;
    sqlx::query("update item set is_active = false where uuid = $1")
        .bind(f.tube)
        .execute(&pool)
        .await
        .unwrap();
    for source in ["คลังยา รพ.", "คลังยา รพ.", "บริจาค"]
    {
        stock::create(
            &pool,
            &f.actor,
            ReceiptInput {
                source: Some(source),
                ..receipt(&f, vec![line(f.glove, f.piece, 1)])
            },
        )
        .await
        .unwrap();
    }

    let options = stock::options(&pool, &f.actor).await.unwrap();
    let items: Vec<_> = options
        .items
        .iter()
        .map(|v| {
            (
                v.item.name.as_str(),
                v.item.base_unit_name.as_str(),
                v.item.track_lot,
                v.item.track_expiry,
                v.conversions
                    .iter()
                    .map(|c| (c.unit_name.as_str(), c.factor))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    assert_eq!(
        items,
        vec![
            ("พาราเซตามอล", "เม็ด", true, true, vec![("กล่อง", 100)]),
            ("ถุงมือ", "ชิ้น", false, false, vec![("กล่อง", 50)]),
        ]
    );
    let years: Vec<_> = options
        .budget_years
        .iter()
        .map(|b| b.year.as_str())
        .collect();
    assert_eq!(years, ["2570"]);
    // ใช้ล่าสุดก่อน ไม่ซ้ำ
    assert_eq!(options.sources, ["บริจาค", "คลังยา รพ."]);
}

#[sqlx::test]
async fn receipts_page_is_seeded_under_the_stock_menu_group(pool: PgPool) {
    assert_eq!(
        seeded_page(&pool, "/stock/receipts").await,
        (
            "รับเข้า".to_string(),
            Some("คลัง".to_string()),
            "stock_receipt.view".to_string()
        )
    );
}
