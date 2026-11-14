# เบิกจ่าย (stock issue) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** ทำใบเบิกจ่ายครบตาม spec — ตัดของออกจากคลังแบบ FEFO, ผูกผู้ป่วยต่อบรรทัด, พรีวิวล็อตก่อนบันทึก, ยกเลิก/แก้หัวใบ และหน้า "เบิกจ่าย" `/stock/issues`

**Architecture:** เพิ่มเอกสารชนิดที่สองในโมดูล `stock/` บน ledger เดิม (`stock_movement` append-only + `stock_balance` ที่ล็อก `FOR UPDATE`)
- ตรรกะเลือกล็อตเป็นฟังก์ชันล้วน `stock/fefo.rs` ใช้ทั้งตอนพรีวิว (ไม่ล็อก) และตอนบันทึก (หลังล็อก)
- service/repository ของใบเบิกแยกไฟล์ `issues.rs` แบบ `users/service/`
- หน้าเว็บอยู่ใน `features/stock/issues/`

**Tech Stack:** Rust + Axum + SQLx (`query!` + `.sqlx` offline), PostgreSQL 16, React + TypeScript + Vite + bun — ไม่เพิ่ม dependency

**Spec:** `docs/inventory-design.md` ข้อ 1, 5, 6 (ตาราง / กฎใบเบิก / API / หน้าจอ), 7, 10 ที่ลงวันที่ 2026-11-02 + `docs/rust-guidelines.md`

## Context
- spec เบิกจ่ายผ่านการออกแบบครบ 5 ส่วน และอยู่ใน PR [kWatcharin/MyHis#3](https://github.com/kWatcharin/MyHis/pull/3)
- แผนนี้แตกงานลงมือเป็นงานย่อย (บันทึก 2026-11-03 ตามแบบ `docs/security-followup-plan.md`) — วิธีลงมือดู Execution ท้ายแผน

## Global Constraints
- **ห้าม `git commit` จนกว่าผู้ใช้สั่ง** (CLAUDE.md) — แผนมี "จุด commit" 2 จุด แต่ละจุดต้อง build ได้เอง
  - โค้ดที่ใช้ `query!` ต้องไปพร้อม `.sqlx` และ migration ของมัน
- ห้ามเพิ่ม crate/npm package ใหม่
  - ติดตั้ง `sqlx-cli` ในเครื่องเป็นเครื่องมือ dev ไม่ใช่ dependency ของโปรเจกต์
- **ข้อมูลผู้ป่วย:**
  - API ส่ง uuid เท่านั้น (ไม่ส่ง id int)
  - `audit_log.metadata`, error message และ log ห้ามมีชื่อ/HN/uuid ผู้ป่วย — snapshot เขียนแค่ "· ผูกผู้ป่วย"
- **ค่าเพดาน:**
  - บรรทัด 1–100
  - จำนวน > 0 และ ≤ 10,000,000 หน่วยฐาน
  - `destination` 1–200, `ref_no` ≤ 50, `note` ≤ 500, `cancel_reason` 1–500 ตัวอักษร (นับตัวอักษร ไม่ใช่ byte)
  - ปลายทางที่เสนอ ≤ 50
- เลขใบ `WD<ปีงบ>-<4 หลัก>` จาก `stock_doc_counter` (`doc_type = 'issue'`) ในทรานแซกชันเดียวกับใบ
- **วันที่:** "วันนี้" = `repository::thai_today` (นาฬิกา DB) ล็อตใช้ได้ถ้า `expiry_date is null or expiry_date >= issued_on`
- **ล็อก:** แถว item (เรียงตาม id) → แถวคงเหลือ (เรียงตาม lot_id) และเลือกล็อตหลังล็อกแล้วเท่านั้น
- **ห้ามในโค้ด request path:**
  - `unwrap()`/`expect()`
  - `format!` ประกอบ SQL
  - `#[allow]` ที่ไม่มีคอมเมนต์
- error message เป็นภาษาอังกฤษ หน้าเว็บแปลไทยใน `issueErrors.ts`
- **ตรวจก่อนทุกจุด commit:**
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `SQLX_OFFLINE=true cargo build`
  - `bun run build && bun run lint`

## Review Focus
1. **บรรทัดที่ยังกรอกไม่ครบ** (ยังไม่เลือกหน่วยหรือจำนวนว่าง) ระหว่างพิมพ์ → พรีวิวต้องข้ามบรรทัดนั้น ไม่ขึ้น error — Task 11 ขั้นตรวจมือ
2. **ผู้มี `stock_issue.view` แต่ไม่มี `patient.view`** เปิดใบที่ผูกผู้ป่วย → เห็น "ผูกผู้ป่วย" และหน้าเว็บไม่เรียก `GET /patients` (ซึ่งจะได้ 403) — Task 10
3. **ผู้ป่วยถูกลบหลังเบิก** → detail ยังคืน uuid และหน้าเว็บแสดง "ผูกผู้ป่วย (ไม่พบในทะเบียน)" — เทส Task 7 + ตรวจมือ Task 10
4. **รายการที่มีของแต่หมดอายุหมดทุกล็อต ณ วันที่เบิก** → พรีวิว "เบิกได้ 0" และบันทึกได้ 409 พร้อม `available = 0` — เทส Task 1 + Task 3
5. **รายการเดียวกันสองบรรทัด (คนละผู้ป่วย) ที่บรรทัดที่สองเกินยอดที่เหลือ** → 409 บอกรายการนั้น และ `available` = ยอดที่เหลือหลังบรรทัดแรก — เทส Task 4

---

## ก่อนเริ่ม (เตรียมเครื่อง — ไม่มี docker daemon และยังไม่มี sqlx-cli)
```bash
# Postgres 16 ในเครื่อง (initdb ห้ามรันเป็น root → ใช้ user postgres) เก็บข้อมูลในโฟลเดอร์ชั่วคราวนอก repo
# (เซสชัน cloud ใช้ scratchpad ของเซสชัน)
SCRATCH=${SCRATCH:-$(mktemp -d)}
PGDATA="$SCRATCH/pgdata"
mkdir -p "$PGDATA" && chown postgres "$PGDATA"
runuser -u postgres -- /usr/lib/postgresql/16/bin/initdb -D "$PGDATA" -U postgres --auth=trust
runuser -u postgres -- /usr/lib/postgresql/16/bin/pg_ctl -D "$PGDATA" -l "$PGDATA/log" -o "-p 5433 -k /tmp" start
createdb -h localhost -p 5433 -U postgres my_his
export DATABASE_URL=postgres://postgres@localhost:5433/my_his
cargo install sqlx-cli --no-default-features --features rustls,postgres --locked
cd backend && cargo sqlx migrate run && cargo test --test stock_receipts   # baseline ต้องเขียวก่อนเริ่ม
```

## File Structure
| ไฟล์ | หน้าที่ |
|---|---|
| `backend/migrations/20261102100000_stock_issues.sql` (ใหม่) | ตาราง/ALTER/CHECK ตาม spec ข้อ 6 + สิทธิ์ admin + เพจ "เบิกจ่าย" |
| `backend/src/stock/fefo.rs` (ใหม่) | ฟังก์ชันล้วนเลือกล็อต FEFO + unit test |
| `backend/src/stock/repository/lookups.rs` (ใหม่ — ย้ายจาก `receipts.rs`) | `find_budget_year`, `find_items`, `find_conversions` ที่สองเอกสารใช้ร่วม |
| `backend/src/stock/repository/ledger.rs` | เพิ่ม `MovementType::Issue`, `insert_issue_movement`, `lock_item_lots_with_stock` |
| `backend/src/stock/repository/issues.rs` (ใหม่) | SQL ของใบเบิกทั้งหมด (`pub mod issues`) |
| `backend/src/stock/service/issues.rs` (ใหม่) | กฎใบเบิก (`pub mod issues` — เรียกเป็น `stock::service::issues::create`) |
| `backend/src/stock/{dto,handlers,routes}.rs`, `src/error.rs` | HTTP ของใบเบิก + 409 ของไม่พอ |
| `backend/src/domain/mod.rs`, `src/sql_console/setup.rs`, `src/audit/repository.rs` | สิทธิ์ใหม่, หน้า SQL, ชื่อรายการในประวัติ |
| `backend/tests/common/stock.rs` (ใหม่ — ย้ายจาก `stock_receipts.rs`), `backend/tests/stock_issues.rs` (ใหม่) | เทส |
| `frontend/src/features/stock/stockApi.ts`, `features/stock/CancelDocumentDialog.tsx` (ใหม่), `features/patients/PatientCombobox.tsx` (ใหม่) | API + ของใช้ร่วม |
| `frontend/src/features/stock/issues/*` (ใหม่) | หน้า/ฟอร์ม/รายละเอียด/แก้หัวใบ/ข้อความ error |

---

### Task 1: ฟังก์ชัน FEFO
**Files:** Create `backend/src/stock/fefo.rs`; Modify `backend/src/stock/mod.rs` (`mod fefo;`)

**Interfaces — Produces** (`pub(super)` ใช้ได้ทั้ง `stock::service` และ `stock::repository`):
```rust
pub(super) struct LotStock { pub lot_id: StockLotId, pub item_id: ItemId, pub lot_no: Option<String>,
                             pub expiry_date: Option<NaiveDate>, pub qty_on_hand: i32 }
pub(super) struct LineRequest { pub item_id: ItemId, pub base_quantity: i32 }   // ตามลำดับบรรทัด
#[derive(Debug, Clone, PartialEq, Eq)] pub(super) struct Take { pub lot_id: StockLotId, pub quantity: i32 }
#[derive(Debug, Clone, PartialEq, Eq)] pub(super) enum LineAllocation { Taken(Vec<Take>), Short { available: i32 } }
pub(super) fn allocate(lots: &[LotStock], lines: &[LineRequest], issued_on: NaiveDate) -> Vec<LineAllocation>
```
กฎของ `allocate`:
- ล็อตใช้ได้ถ้า `qty_on_hand > 0` และ (`expiry_date` ว่าง หรือ `>= issued_on`)
- เรียงตาม (ไม่มีวันหมดอายุไว้ท้าย, `expiry_date`, `lot_id.0`)
- ยอดคงเหลือต่อล็อตลดตามบรรทัดก่อนหน้า
- บรรทัดที่ไม่พอไม่หักอะไร และ `available` = ยอดที่ยังใช้ได้ของรายการนั้น ณ จุดนั้น

- [ ] **Step 1: เขียน unit test (`#[cfg(test)] mod tests` ในไฟล์เดียวกัน)**
  - `earliest_expiry_goes_first_and_undated_lots_last`: ล็อต exp +30 / ไม่มีวันหมดอายุ / exp +10 แล้วขอ 5 → `Taken([Take{lot exp+10, 5}])`
  - `same_expiry_falls_back_to_lot_id`: สองล็อตวันเดียวกัน id 7 และ 3 → ได้ id 3 ก่อน
  - `lot_expiring_on_the_issue_date_is_usable_but_not_the_day_before`: exp = issued_on → Taken และ exp = issued_on − 1 → `Short{available: 0}`
  - `takes_across_lots_and_can_empty_them_exactly`: ล็อต 30 + 100 ขอ 130 → `[Take{a,30}, Take{b,100}]`
  - `short_line_takes_nothing_and_reports_what_is_left`: ล็อต 50 ขอ 60 → `Short{available: 50}` แล้วบรรทัดถัดไปขอ 50 → Taken 50
  - `lines_of_the_same_item_take_in_line_order`: ล็อต 30 + 100 บรรทัดละ 40 → บรรทัด 1 = 30 + 10 และบรรทัด 2 = 40 จากล็อตที่สอง
  - `expired_only_stock_reports_zero_available` (Review Focus 4)
- [ ] **Step 2:** `cd backend && cargo test --lib stock::fefo` ต้อง FAIL (ยังไม่มี `allocate`)
- [ ] **Step 3:** เขียน `allocate` ตามกฎข้างบน
- [ ] **Step 4:** `cargo test --lib stock::fefo` ต้อง PASS 7 เทส ส่วน clippy จะเตือน dead_code จนถึง Task 3 (มีผู้เรียก) ซึ่งยอมรับได้ เพราะ clippy เป็นด่านที่จุด commit

### Task 2: Migration + สิทธิ์ + หน้า SQL + เทสของ schema
**Files:**
- Create `backend/migrations/20261102100000_stock_issues.sql`, `backend/tests/common/stock.rs`, `backend/tests/stock_issues.rs`
- Modify `backend/src/domain/mod.rs`, `backend/src/sql_console/setup.rs`, `backend/tests/common/mod.rs` (`pub mod stock;`), `backend/tests/stock_receipts.rs` (ใช้ helper จาก common), `backend/tests/{permissions,schema,pages}.rs`

**Interfaces — Produces:**
- `domain::StockIssueId(pub i32)`
- `Resource::StockIssue` + PERMISSIONS `stock_issue.view/create/update/delete`
- `tests/common/stock.rs`: `Fixture` เดิม (เพิ่ม `para` เป็น `patient_linkable: Some(true)`), `fixture`, `thai_today`, `insert_budget_year`, `on_hand`, `assert_ledger_matches_balance`, `movement_types`, `receive(f, lines)` (ช่วยรับเข้าผ่าน `stock::create`)

- [ ] **Step 1:** ย้าย helper จาก `stock_receipts.rs` ไป `tests/common/stock.rs` แล้วรัน `cargo test --test stock_receipts` ต้อง PASS (refactor ไม่เปลี่ยนพฤติกรรม)
- [ ] **Step 2:** เขียนเทสใน `tests/stock_issues.rs`
  - `issue_movements_must_be_negative_and_match_their_source`: insert ด้วย SQL ตรง แล้ว error ต้องมีชื่อ constraint `stock_movement_outbound_negative` / `stock_movement_source_matches_type` / `stock_movement_one_source`
  - `an_issue_line_takes_each_lot_once`: movement `issue` ซ้ำ (บรรทัดเดิม ล็อตเดิม) ต้องชน `stock_movement_issue_line_lot_uq`
  - `issues_page_is_seeded_next_to_receipts`: เพจ `/stock/issues` (label "เบิกจ่าย", ไอคอน `package-minus`) มี `ref_id` เดียวกับ `/stock/receipts` และมี `page_permissions` เป็น `stock_issue.view`
  - แก้ตัวเลขในเทสเดิม:
    - `permissions.rs` สองที่ และ `schema.rs` จาก 64 → 68
    - `pages.rs` เพิ่ม `"- เบิกจ่าย /stock/issues"` ต่อจากรับเข้า
    - `schema.rs` เพิ่มสองตารางใหม่ในเทสตารางที่มี id/uuid
- [ ] **Step 3:** `cargo test --test stock_issues --test permissions --test schema --test pages` ต้อง FAIL
- [ ] **Step 4:** เขียน migration ตาม spec ข้อ 6
  - ตาราง + comment ภาษาไทยทุกคอลัมน์แบบ migration ใบรับ
  - index
  - CHECK ที่เขียนใหม่ใช้ `drop constraint` + `add constraint` แยกคำสั่ง
  - `stock_doc_counter_type_check` → `('receipt', 'issue')`
  - grant สิทธิ์ admin แบบ migration ใบรับ
  - เพจ: `insert into pages (label, icon, route, sort_order, ref_id) select 'เบิกจ่าย', 'package-minus', '/stock/issues', sort_order + 10, ref_id from pages where route = '/stock/receipts'` + `page_permissions`
- [ ] **Step 5:** เพิ่ม `StockIssueId`, `Resource::StockIssue` และ 4 แถว PERMISSIONS แล้วเพิ่ม `"stock_issue"`, `"stock_issue_line"` ใน `READABLE_TABLES` (เรียงตามตัวอักษร ขนาด 29 → 31)
- [ ] **Step 6:** `cargo sqlx migrate run && cargo test --test stock_issues --test permissions --test schema --test pages --test stock_receipts --test sql_console` ต้อง PASS

### Task 3: บันทึกใบเบิก (ไม่ผูกผู้ป่วย)
**Files:**
- Create `backend/src/stock/repository/lookups.rs`, `repository/issues.rs`, `service/issues.rs`
- Modify `repository/{mod,receipts,ledger}.rs`, `service/mod.rs`
- Test `backend/tests/stock_issues.rs`

**Interfaces:**
- **Consumes:** `fefo::{LotStock, LineRequest, LineAllocation, allocate}` (Task 1), `StockIssueId` (Task 2)
- **Produces:**
  - lookups ย้ายจาก `receipts.rs` ไป `lookups.rs` และ re-export ด้วย `pub use lookups::*` ให้ใบรับใช้ชื่อเดิม `ItemLookup` เพิ่ม `patient_linkable: bool`
  - ledger:
    - `MovementType::Issue` (`"issue"`)
    - `insert_issue_movement(tx, actor: UserId, lot_id: StockLotId, movement_type: MovementType, qty_delta: i32, issue_line_id: i32, reverses: Option<i32>) -> Result<(), RepositoryError>`
    - `lock_item_lots_with_stock(tx, item_ids: &[i32]) -> Result<Vec<LotStock>, RepositoryError>` (`qty_on_hand > 0`, `order by b.lot_id for update of b`)
  - `repository::issues`:
    - `IssueHeaderWrite { destination: String, ref_no: Option<String>, note: Option<String> }`
    - `IssueLineWrite { item_id: ItemId, base_unit_id: ItemUnitId, unit_id: ItemUnitId, unit_factor: i32, quantity: i32, patient_id: Option<PatientId> }`
    - `IssueWrite { budget_year_id: BudgetYearId, budget_year: String, issued_on: NaiveDate, header: IssueHeaderWrite, lines: Vec<IssueLineWrite> }`
    - `enum IssueCreateOutcome { Created { public_id: Uuid, doc_no: String }, ItemChanged, InsufficientStock { line_index: usize, available: i32 } }`
    - `insert_issue(pool, actor: UserId, w: &IssueWrite) -> Result<IssueCreateOutcome, RepositoryError>`
    - ตัวช่วยภายใน `lock_issue_items_unchanged` (ไม่ถูกลบ + หน่วยฐาน/factor ตรง โดยไม่ตรวจ `is_active`), `insert_audit` (target `stock_issue`), `snapshot`
  - `service::issues`:
    - `IssueLineInput { item_id: Uuid, unit_id: Uuid, quantity: i32, patient_id: Option<Uuid> }`
    - `IssueInput<'a> { budget_year_id: Uuid, issued_on: NaiveDate, destination: &'a str, ref_no: Option<&'a str>, note: Option<&'a str>, lines: Vec<IssueLineInput> }`
    - `CreatedIssue { public_id: Uuid, doc_no: String }`
    - `create(pool, actor: &CurrentUser, input: IssueInput<'_>) -> Result<CreatedIssue, StockError>`
  - `StockError::InsufficientStock { item_id: Uuid, available: i32 }` (`#[error("insufficient stock")]`)
- ข้อความ validation:
  - `at least one line is required`, `too many lines`
  - `destination is required` / `destination must be at most 200 characters` (มาจาก `clean_required`)
  - `unknown budget year`, `budget year is inactive`, `issued date is outside the budget year`, `issued date must not be in the future`
  - `unknown item`, `unit is not a unit of this item`, `quantity must be greater than 0`, `quantity is too large`
  - conflict: `item changed while saving, please try again`

- [ ] **Step 1:** ย้าย lookups ไป `lookups.rs` แล้วรัน `cargo test --test stock_receipts` ต้อง PASS
- [ ] **Step 2:** เขียนเทส
  - `issue_posts_negative_movements_and_reduces_balance_in_base_units`: รับพาราเซตามอล 2 กล่อง (200 เม็ด) แล้วเบิก 1 กล่อง
    - `on_hand == 100`
    - `movement_types == ["receive","issue"]`
    - `doc_no == "WD2570-0001"` และใบถัดไป `"WD2570-0002"`
    - `assert_ledger_matches_balance`
  - `fefo_takes_the_earliest_expiry_and_skips_expired_lots`: รับย้อนหลัง (received_on = today−10) ล็อต OLD (exp today−1) 50, SOON (exp +10) 30, LATE (exp +90) 100
    - เบิก 40 วันนี้ → SOON −30, LATE −10 และ OLD ไม่ถูกแตะ
    - เบิกย้อนหลัง issued_on = today−5 จำนวน 20 → OLD −20
  - `issue_more_than_usable_stock_is_refused_without_touching_stock`: ได้ `InsufficientStock { item_id: para, available: 130 }` ยอดไม่เปลี่ยน และใบถัดไปยังได้เลข `-0001` (เลขไม่โดด)
  - `expired_only_stock_is_refused_with_zero_available` (Review Focus 4)
  - `concurrent_issues_for_the_last_units_succeed_once`: `tokio::spawn` สองคำขอ คำขอละ 100 จากคงเหลือ 100 (แบบ `tests/credential_races.rs`) → Ok หนึ่ง, InsufficientStock หนึ่ง, `on_hand == 0` และ ledger ตรง
  - `issue_date_must_be_in_an_active_budget_year_and_not_in_the_future`: ทุกข้อความวันที่/ปีงบข้างบน และลงย้อนหลังในปีงบได้
  - `lines_are_validated_against_units_limits_and_deleted_items`:
    - หน่วยไม่ใช่ของรายการ / จำนวน 0 / จำนวนเกินเพดาน → validation
    - รายการที่ปิดใช้งานแต่มีของ → เบิกได้
    - รายการที่ถูกลบ → `unknown item`
- [ ] **Step 3:** `cargo test --test stock_issues` ต้อง FAIL ด้วยเหตุว่ายังไม่มี `issues::create`
- [ ] **Step 4:** เขียน ledger + `repository::issues::insert_issue`
  - ลำดับในทรานแซกชันเดียว:
    1. `lock_issue_items_unchanged`
    2. `lock_item_lots_with_stock`
    3. `fefo::allocate` (เจอบรรทัด Short แรกให้คืน `InsufficientStock` แล้วทิ้งทรานแซกชัน เลขที่ใบจึง rollback ไปด้วย)
    4. ออกเลข `WD` แบบใบรับ
    5. หัวใบ
    6. ต่อบรรทัด: บรรทัด → `insert_issue_movement(−qty)` + `add_to_balance(−qty)` ต่อล็อต
    7. snapshot + audit `stock_issue_created`
  - เขียน `service::issues::create` ตามกฎใบเบิกใน spec (ตรวจแบบ `service/receipts.rs::create`/`prepare_line`) แล้วแปลง `line_index` เป็น uuid ของรายการ
- [ ] **Step 5:** `cargo test --test stock_issues && cargo test --test stock_receipts` ต้อง PASS

### Task 4: ผูกผู้ป่วยต่อบรรทัด
**Files:** Modify `repository/issues.rs`, `service/issues.rs`; Test `tests/stock_issues.rs`

**Interfaces:**
- **Produces:** `repository::issues::PatientLookup { id: PatientId, public_id: Uuid, is_active: bool }` และ `find_patients(pool, public_ids: &[Uuid])` (แบบ `lab_visit/repository.rs::find_patients` แต่เขียนในโมดูลนี้ เพราะโมดูลไม่เรียกกันข้ามโมดูล)
- **ข้อความ:** `item cannot be linked to a patient`, `unknown patient`, `patient is inactive`, `duplicate item for the same patient`
- ถ้ามีบรรทัดที่มี `patient_id` แต่ผู้ใช้ไม่มี `patient.view` ตอบ `Forbidden` ก่อนตรวจอย่างอื่นทั้งหมด

- [ ] **Step 1:** เขียนเทส (ใช้ `insert_prename` + `insert_patient` ใน `tests/common`)
  - `patient_lines_need_a_linkable_item_and_an_active_patient`: ถุงมือ + ผู้ป่วย → `item cannot be linked to a patient` ผู้ป่วยปิดใช้งาน → `patient is inactive` uuid สุ่ม → `unknown patient`
  - `patient_lines_need_patient_view_before_patient_checks`: actor มีแค่ `all_of(Resource::StockIssue)` และส่ง uuid สุ่ม → `Forbidden` (ไม่ใช่ `unknown patient`)
  - `same_item_for_two_patients_takes_in_line_order`: ล็อต 30 + 100 สองบรรทัดคนละผู้ป่วยบรรทัดละ 40 → ล็อตที่ถูกตัดตรงกับ fefo
  - `second_line_of_the_same_item_reports_what_is_left_after_the_first` (Review Focus 5): คงเหลือ 100 บรรทัด 70 + 50 → `InsufficientStock { available: 30 }`
  - `duplicate_item_for_the_same_patient_is_refused`
  - `issue_audit_has_no_patient_identity`: `metadata::text` มี "ผูกผู้ป่วย" และไม่มี uuid ของผู้ป่วย
- [ ] **Step 2:** `cargo test --test stock_issues patient` ต้อง FAIL
- [ ] **Step 3:** เขียนส่วนตรวจผู้ป่วยใน `create` และใส่ `patient_id` ลงบรรทัด (`snapshot` เพิ่ม "· ผูกผู้ป่วย")
- [ ] **Step 4:** `cargo test --test stock_issues` ต้อง PASS ทั้งไฟล์

### Task 5: พรีวิว FEFO
**Files:** Modify `repository/issues.rs`, `service/issues.rs`; Test `tests/stock_issues.rs`

**Interfaces — Produces:**
- `repository::issues::available_lots(pool, item_ids: &[i32]) -> Result<Vec<LotStock>, RepositoryError>` (ไม่ล็อก)
- `service::issues`:
  - `PreviewLineInput { item_id: Uuid, unit_id: Uuid, quantity: i32 }`
  - `PreviewInput { issued_on: NaiveDate, lines: Vec<PreviewLineInput> }`
  - `LotTaken { lot_no: Option<String>, expiry_date: Option<NaiveDate>, quantity: i32 }`
  - `enum PreviewLine { Taken(Vec<LotTaken>), Short { available: i32 } }`
  - `preview(pool, actor, input: PreviewInput) -> Result<Vec<PreviewLine>, StockError>` (สิทธิ์ `stock_issue.create`, ตรวจบรรทัดแบบ `create` ยกเว้นผู้ป่วย และไม่ตรวจวันที่กับปีงบ)

- [ ] **Step 1:** เขียนเทส
  - `preview_matches_the_saved_allocation_and_changes_nothing`: พรีวิวแล้ว `on_hand` เท่าเดิม จากนั้นบันทึก แล้วล็อตใน detail ต้องเท่ากับพรีวิว (เทียบผ่าน SQL ของ movement จนกว่า Task 7 จะมี detail)
  - `preview_reports_short_lines_without_failing`: บรรทัดเกินได้ `Short { available }` และบรรทัดอื่นยังได้ Taken
  - `preview_needs_create_permission`: actor ที่มีแค่ view ได้ `Forbidden`
- [ ] **Step 2:** FAIL → **Step 3:** เขียนโค้ด → **Step 4:** `cargo test --test stock_issues preview` ต้อง PASS

### Task 6: ยกเลิกใบ + แก้หัวใบ
**Files:** Modify `repository/issues.rs`, `service/issues.rs`; Test `tests/stock_issues.rs`

**Interfaces — Produces:**
- repository:
  - `find_issue_id(pool, public_id: Uuid) -> Result<Option<StockIssueId>, RepositoryError>`
  - `enum IssueHeaderOutcome { Updated, Cancelled }`
  - `update_issue_header(pool, actor: UserId, id: StockIssueId, h: &IssueHeaderWrite) -> Result<IssueHeaderOutcome, RepositoryError>`
  - `enum IssueCancelOutcome { Cancelled, AlreadyCancelled, ItemDeleted }`
  - `cancel_issue(pool, actor: UserId, id: StockIssueId, reason: &str) -> Result<IssueCancelOutcome, RepositoryError>` (ล็อก item → คงเหลือ แล้วทำ reversal ทุก movement `issue` ด้วย `insert_issue_movement(Reversal, +qty, issue_line_id เดิม, Some(id เดิม))`)
- service:
  - `IssueHeaderInput<'a> { destination: &'a str, ref_no: Option<&'a str>, note: Option<&'a str> }`
  - `update_header(pool, actor, public_id: Uuid, input: IssueHeaderInput<'_>) -> Result<(), StockError>`
  - `cancel(pool, actor, public_id: Uuid, reason: &str) -> Result<(), StockError>`
- ข้อความ: `issue is cancelled`, `issue is already cancelled`, `an item on this issue has been deleted`, `cancel reason is required`
- audit `stock_issue_updated` / `stock_issue_cancelled`
- `audit/repository.rs` เพิ่ม `when 'stock_issue' then siss.doc_no` + left join `stock_issue`

- [ ] **Step 1:** เขียนเทส
  - `cancel_returns_stock_to_the_same_lots_and_keeps_movements`: movement เดิมยังอยู่ และ reversal อ้าง `reverses_id` ครบ `on_hand` คืนค่าเดิม ledger ตรง
  - `cancel_twice_is_refused`
  - `cancel_is_refused_when_an_item_was_deleted`: เบิกจนหมด → ลบรายการ → ยกเลิกใบ ได้ 409
  - `cancel_returns_stock_into_an_expired_lot`
  - `header_can_be_edited_and_is_audited_with_the_doc_no`: ใช้ `audit_service` ดูประวัติ ต้องมีชื่อรายการ = `WD2570-0001`
  - `cancelled_issue_header_cannot_be_edited`
- [ ] **Step 2:** FAIL → **Step 3:** เขียนโค้ด → **Step 4:** `cargo test --test stock_issues` ต้อง PASS ทั้งไฟล์ (cargo รับตัวกรองชื่อเทสได้ทีละตัว)

### Task 7: list / detail / options + สิทธิ์ราย action
**Files:** Modify `repository/issues.rs`, `service/issues.rs`; Test `tests/stock_issues.rs`

**Interfaces — Produces:**
- repository:
  - `IssueRow { id, public_id, doc_no, budget_year_public_id, budget_year, issued_on, destination, ref_no, note, line_count: i64, has_patient: bool, created_by_name, created_at, cancelled_by_name, cancelled_at, cancel_reason }`
  - `fetch_issues(pool, public_id: Option<Uuid>)` (เรียง `issued_on desc, id desc`)
  - `IssueLineRow { public_id: Uuid, item_public_id, item_name, unit_name, unit_factor, quantity, base_quantity, base_unit_name, patient_public_id: Option<Uuid> }`
  - `IssueLotRow { line_public_id: Uuid, lot_no: Option<String>, expiry_date: Option<NaiveDate>, quantity: i32 }` (= −qty_delta ของแถว `issue`)
  - `issue_lines(pool, id) -> Result<(Vec<IssueLineRow>, Vec<IssueLotRow>), RepositoryError>`
  - `IssueItemOptionRow { …ข้อมูลเดียวกับ ItemOptionRow ของใบรับ…, patient_linkable, is_active, on_hand: i64 }`
  - `IssueOptions { items, conversions, budget_years, destinations: Vec<String> }`
  - `list_issue_options(pool)` (รายการไม่ถูกลบที่ `on_hand > 0` รวมล็อตหมดอายุ, ปีงบใช้งานอยู่, ปลายทางล่าสุดก่อน ≤ 50)
- service:
  - `list(pool, actor) -> Vec<IssueRow>`
  - `IssueLineView { line: IssueLineRow, lots: Vec<LotTaken> }` และ `IssueView { issue: IssueRow, lines: Vec<IssueLineView> }`
  - `get(pool, actor, public_id) -> IssueView`
  - `IssueOptionsView` + `options(pool, actor)` (ทุกตัวใช้สิทธิ์ `stock_issue.view`)

- [ ] **Step 1:** เขียนเทส
  - `list_detail_and_options_show_what_the_form_needs`:
    - options มีรายการปิดใช้งานที่ยังมีของ และไม่มีรายการที่คงเหลือ 0
    - `on_hand` รวมล็อตหมดอายุ
    - ปลายทางเรียงล่าสุดก่อน
    - detail มีล็อตต่อบรรทัด
    - list มี `has_patient`
  - `detail_keeps_the_patient_uuid_after_the_patient_is_deleted` (Review Focus 3): `soft_delete_row(patient)` แล้ว `get` ยังคืน `patient_public_id`
  - `each_issue_action_needs_its_own_permission`: view/create/update/delete แยกกัน (actor ที่มี `all_except(Resource::StockIssue)` ถูกปฏิเสธทุกตัว) และ preview ใช้ create
- [ ] **Step 2:** FAIL → **Step 3:** เขียนโค้ด → **Step 4:** `cargo test --test stock_issues` ต้อง PASS ทั้งไฟล์

### Task 8: HTTP + 409 ของไม่พอ + เอกสาร (จุด commit ที่ 1: backend)
**Files:**
- Modify `backend/src/stock/{dto,handlers,routes}.rs`, `backend/src/error.rs`, `docs/my-his-data-dict.xlsx`, `docs/auth-design.md`, `CLAUDE.md`, `docs/inventory-design.md`
- Test `tests/stock_issues.rs`

**Interfaces — Produces (JSON camelCase; request ใส่ `deny_unknown_fields`):**
- routes:
  - `GET/POST /stock/issues`
  - `GET /stock/issues/options`
  - `POST /stock/issues/preview`
  - `GET/PUT /stock/issues/{id}`
  - `POST /stock/issues/{id}/cancel`
- DTO:
  - `CreateIssueRequest { budgetYearId, issuedOn, destination, refNo?, note?, lines: [{ itemId, unitId, quantity, patientId? }] }`
  - `PreviewIssueRequest { issuedOn, lines: [{ itemId, unitId, quantity }] }`
  - `UpdateIssueRequest { destination, refNo, note }`
  - `CancelIssueRequest { reason }`
  - response แบบใบรับ
- พรีวิวตอบ `[{ "status": "ok", "lots": [{ lotNo, expiryDate, quantity }] } | { "status": "short", "available": n }]` (`#[serde(tag = "status", rename_all = "camelCase")]`)
- error:
  - `AppError::InsufficientStock { item_id: Uuid, available: i32 }` → 409 + `ErrorBody` เพิ่ม `item_id: Option<Uuid>` และ `available: Option<i32>` (`skip_serializing_if`)
  - `From<StockError>` แปลงให้ด้วย

- [ ] **Step 1:** เขียนเทส router `insufficient_stock_409_carries_item_id_and_available`
  - ใช้ `test_app` + `insert_user_with_password` + `login` + `send` (POST `/api/stock/issues`)
  - ต้องได้ `status == 409`, `body["error"] == "insufficient stock"`, `body["itemId"] == para` และ `body["available"] == 130`
- [ ] **Step 2:** FAIL → **Step 3:** เขียน DTO/handlers/routes/error → **Step 4:** PASS
- [ ] **Step 5: เอกสาร**
  - data dict ใช้ skill `anthropic-skills:xlsx`:
    - เพิ่มชีต `stock_issue`, `stock_issue_line` ต่อจาก `stock_receipt_line` ในรูปแบบเดียวกับชีต `stock_receipt`
    - แก้ชีต `stock_movement` (`issue_line_id`, ชนิด `issue`) และ `stock_doc_counter` (`issue`)
  - `auth-design.md`: สิทธิ์ `stock_issue.*`
  - `CLAUDE.md`: บรรทัด `stock/` เพิ่มใบเบิกและหน้า "เบิกจ่าย"
  - `inventory-design.md`: สถานะ "ทำแล้ว" เฉพาะ backend ส่วนหน้าเว็บรอจุดที่ 2
- [ ] **Step 6: ตรวจทั้งหมด**
  - `cargo sqlx prepare`
  - `cargo fmt --check`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo test`
  - `SQLX_OFFLINE=true cargo build` (ปิด `DATABASE_URL`)
  - ทุกตัวต้องผ่าน
- [ ] **Step 7: จุด commit ที่ 1** — แสดง diff stat แล้ว **ถามผู้ใช้ก่อน commit** (`feat(stock): stock issues with FEFO, patient links and preview`) และถาม push ด้วย (PR #3 ยังเปิดอยู่ commit จะเข้า PR นั้น)

### Task 9: หน้าเว็บ — API + ของใช้ร่วม
**Files:**
- Modify `frontend/src/features/stock/stockApi.ts`, `features/stock/receipts/StockReceiptsPage.tsx`
- Create `features/stock/CancelDocumentDialog.tsx`, `features/patients/PatientCombobox.tsx`, `features/stock/issues/issueErrors.ts`
- Delete `features/stock/receipts/CancelReceiptDialog.tsx`

**Interfaces — Produces:**
- type ใน `stockApi.ts` ตรงกับ DTO Task 8: `StockIssue`, `StockIssueLine`, `StockIssueLot`, `StockIssueDetail`, `IssueOptions`, `IssueItemOption`, `IssueInput`, `IssuePreviewLine`
- ฟังก์ชัน: `listIssues`, `getIssue`, `issueOptions`, `previewIssue`, `createIssue`, `updateIssue`, `cancelIssue`
- `CancelDocumentDialog({ title: string | null, description: string, inputId: string, onOpenChange, onConfirm(reason): Promise<void> })` (title null = ปิด)
- `PatientCombobox({ id, patients: Patient[], value: string | null, onChange(id: string | null), disabled? })`
  - ค้นด้วยชื่อ/HN/NAP ผ่าน `allTokensIncluded`/`textIncludes` ของ `lib/search`
  - เลือกได้เฉพาะผู้ป่วยที่ใช้งานอยู่
- `issueErrorMessage(err: unknown): string` (ตารางแปลข้อความทุกตัวจาก Task 3–7) และ `insufficientStockOf(err: unknown): { itemId: string; available: number } | null` (อ่าน `ApiError.body`)

- [ ] **Step 1:** เขียนโค้ด และให้หน้ารับเข้าใช้ `CancelDocumentDialog` (title "ยกเลิกใบรับ {docNo}")
- [ ] **Step 2:** `cd frontend && bun run build && bun run lint` ต้องผ่าน
- [ ] **Step 3: ตรวจมือ** — ยกเลิกใบรับได้เหมือนเดิม และเหตุผลว่างต้องกดยืนยันไม่ได้

### Task 10: หน้ารายการ + รายละเอียด + แก้หัวใบ + การเชื่อมต่อ
**Files:**
- Create `features/stock/issues/{StockIssuesPage,StockIssueDetailDialog,StockIssueHeaderDialog}.tsx`
- Modify `frontend/src/App.tsx` (route `/stock/issues` + `RequireAuth permission="stock_issue.view"`)
- Modify `features/admin/permissions.ts` (`{ key: 'stock_issue', route: '/stock/issues', fallbackLabel: 'เบิกจ่าย', actions: CRUD, actionLabels: { delete: 'ยกเลิก' } }`)
- Modify `features/audit/activity-history/{activities,activityDetails}.ts` (`stock_issue_cancelled: { label: 'ยกเลิก', kind: 'delete' }` และป้ายช่อง `issued_on` 'วันที่เบิก', `destination` 'ปลายทาง')

**Interfaces — Consumes:** Task 9 และ `usePagePermissions('stock_issue')` + `usePagePermissions('patient').canView`

- [ ] **Step 1:** เขียนหน้าตาม spec "หน้าจอใบเบิก"
  - ค้นหาด้วยเลขใบ/เลขอ้างอิง, ปลายทาง, ปีงบ, สถานะ แบบหน้ารับเข้า
  - detail เรียก `patientsApi.list()` เฉพาะเมื่อ `canView` ของ patient เท่านั้น
- [ ] **Step 2:** `bun run build && bun run lint` ต้องผ่าน
- [ ] **Step 3: ตรวจมือ** (เปิดแอปด้วย skill `run`)
  - เมนู "เบิกจ่าย" อยู่ใต้กลุ่มคลัง
  - ปุ่มแสดงตามสิทธิ์
  - ดูใบในมุมมอง role ที่ไม่มี `patient.view` (หน้า role-preview) ต้องเห็น "ผูกผู้ป่วย" และไม่มีคำขอ `/api/patients` (Review Focus 2)
  - ผู้ป่วยที่ถูกลบแสดง "ผูกผู้ป่วย (ไม่พบในทะเบียน)" (Review Focus 3)

### Task 11: ฟอร์มเบิก + พรีวิว (จุด commit ที่ 2: หน้าเว็บ)
**Files:** Create `features/stock/issues/StockIssueFormDialog.tsx`; Modify `StockIssuesPage.tsx`

**Interfaces — Consumes:** `previewIssue`, `createIssue`, `insufficientStockOf` (Task 9)

- [ ] **Step 1:** เขียนฟอร์มตาม spec
  - **หัวใบ:**
    - วันที่ตั้งต้น `todayIso()` และปีงบเติมจากวันที่แบบฟอร์มรับ
    - ปลายทาง combobox เสนอค่าเดิม
    - คำเตือน "อย่าใส่ชื่อ/HN ผู้ป่วย — ผูกผู้ป่วยที่บรรทัด"
  - **บรรทัด:**
    - เลือกรายการจาก options (มีแต่ของที่มี) แสดงคงเหลือ และป้าย "ปิดใช้งาน"
    - หน่วย/จำนวน และข้อความแปลงหน่วยแบบฟอร์มรับ
    - ช่องผู้ป่วยแสดงเฉพาะรายการ `patientLinkable` และผู้มี `patient.view`
    - บรรทัดใหม่ใช้ผู้ป่วยของบรรทัดก่อน ถ้ารายการผูกได้
  - **พรีวิว:**
    - `useEffect` + `setTimeout` 300 ms (ไม่เพิ่ม library)
    - ส่งเฉพาะบรรทัดที่ครบ (รายการ + หน่วย + จำนวน > 0) แล้วแมปผลกลับด้วย key ของบรรทัด
    - แสดง "lot A123 (หมด 31/03/2570) 20 เม็ด · …" และ "ล็อตไม่ระบุ" ส่วนของไม่พอแสดงตัวแดง "ไม่พอ — เบิกได้ N <หน่วยฐาน>"
  - **บันทึก:**
    - กดได้เมื่อทุกบรรทัดครบและพอ และพรีวิวไม่ค้าง
    - ถ้าได้ 409 `insufficientStockOf` ให้พรีวิวใหม่และไฮไลต์บรรทัดของ `itemId`
- [ ] **Step 2:** `bun run build && bun run lint` ต้องผ่าน
- [ ] **Step 3: ตรวจมือ**
  - เบิกจริงหลายล็อต พรีวิวต้องตรงกับรายละเอียดหลังบันทึก
  - บรรทัดที่กรอกไม่ครบระหว่างพิมพ์ต้องไม่ขึ้น error (Review Focus 1)
  - ของหมดอายุทั้งหมดขึ้น "เบิกได้ 0"
  - เบิกตัดหน้าจากอีกแท็บแล้วฟอร์มต้องไฮไลต์บรรทัด
  - ใช้บนจอกว้างประมาณ 390px ได้
- [ ] **Step 4:** แก้ `inventory-design.md` สถานะเป็น "ทำแล้วทั้งหมด"
- [ ] **Step 5: จุด commit ที่ 2** — ถามผู้ใช้ก่อน commit (`feat(ui): stock issue page with FEFO preview`) และ push

## Execution
- ผู้ใช้เลือก **Native** (2026-11-03): ทำทีละ task ในเซสชันเดียว ตามลำดับ Task 1 → 11 (skill `superpowers:executing-plans`)
  แล้วมีผู้รีวิวหนึ่งคนตรวจทั้ง branch ตอนจบ เหตุผลที่เลือก:
  - task ผูกกันด้วย interface ต่อเนื่องเป็นสาย (fefo → create → preview → cancel → HTTP → หน้าเว็บ)
  - จุดเสี่ยงอยู่ที่ไม่กี่ไฟล์เดิม
  - subagent ต่อ task จะต้องโหลดบริบทใหม่ซ้ำทุกครั้ง
