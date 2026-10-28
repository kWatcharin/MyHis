-- ธุรกรรมคลังส่วนแรก (docs/inventory-design.md ข้อ 6, 9 ขั้น 2): ล็อต, คงเหลือต่อล็อต, ledger, ใบรับเข้า (รวมยอดยกมา)
-- และตัวรันเลขที่เอกสาร — เบิก/นับ/ปรับยอดจะมาเสียบ ledger นี้ภายหลัง
--
-- แหกรูปแบบเดิมของตาราง (id + uuid + audit + soft delete) โดยตั้งใจ:
--   * stock_movement (ledger) เพิ่มได้อย่างเดียว — ไม่มี updated/soft delete, trigger ห้าม UPDATE/DELETE/TRUNCATE
--     แก้ผิดด้วย movement กลับรายการ (reversal) ที่อ้างแถวเดิม เพราะ ledger เป็นหลักฐาน ไม่ใช่ข้อมูลตั้งค่า
--   * stock_receipt ใช้ "ยกเลิก" (cancelled_*) แทน soft delete — ใบที่ยกเลิกยังแสดงในรายการ และยกเลิก = ลง reversal
--   * stock_balance / stock_doc_counter ไม่มี uuid/audit — เป็นตัวนับ ประวัติอยู่ใน ledger และ audit_log
-- จำนวนทุกช่องเป็น integer ในหน่วยฐานของรายการ (ยกเว้น stock_receipt_line.quantity = หน่วยที่กรอก)

create table stock_lot (
    id           integer generated always as identity primary key,
    uuid         uuid not null unique default uuid_v7(),
    item_id      integer not null references item (id),
    lot_no       varchar(50),
    expiry_date  date,
    created_by   integer not null references users (id),
    created_at   timestamptz not null default now(),
    updated_by   integer not null references users (id),
    updated_at   timestamptz not null default now(),
    constraint stock_lot_expiry_requires_lot check (expiry_date is null or lot_no is not null)
);
-- lot no ห้ามซ้ำในรายการเดียว (ไม่สนตัวพิมพ์) และ null นับเป็นค่าเดียวกัน → ล็อตไม่ระบุได้แถวเดียวต่อรายการ
create unique index stock_lot_item_lot_uq on stock_lot (item_id, lower(lot_no)) nulls not distinct;
comment on table stock_lot is 'ล็อตของรายการคลัง (สร้างตอนรับเข้าครั้งแรก) — รายการที่ไม่ติดตามล็อตมีล็อตไม่ระบุแถวเดียว';
comment on column stock_lot.item_id is 'item.id';
comment on column stock_lot.lot_no is 'เลขล็อต (ห้ามซ้ำในรายการเดียว ไม่สนตัวพิมพ์) — null = ล็อตไม่ระบุ ของรายการที่ไม่ติดตามล็อต';
comment on column stock_lot.expiry_date is 'วันหมดอายุ — null = รายการไม่ติดตามวันหมดอายุ (มีวันหมดอายุต้องมีเลขล็อต — CHECK) รับซ้ำล็อตที่คงเหลือ 0 ด้วยวันใหม่ได้';

create table stock_balance (
    lot_id       integer primary key references stock_lot (id),
    qty_on_hand  integer not null default 0,
    updated_at   timestamptz not null default now(),
    constraint stock_balance_not_negative check (qty_on_hand >= 0)
);
comment on table stock_balance is 'คงเหลือต่อล็อต — อัปเดตในทรานแซกชันเดียวกับ stock_movement และเป็นแถวที่ล็อก (FOR UPDATE) กันรับ/เบิก/ยกเลิกซ้อน ต้องเท่ากับผลรวม qty_delta ของล็อตเสมอ';
comment on column stock_balance.lot_id is 'stock_lot.id';
comment on column stock_balance.qty_on_hand is 'คงเหลือ (หน่วยฐาน) ห้ามติดลบ (CHECK)';
comment on column stock_balance.updated_at is 'เวลาที่ยอดเปลี่ยนล่าสุด';

create table stock_receipt (
    id              integer generated always as identity primary key,
    uuid            uuid not null unique default uuid_v7(),
    doc_no          varchar(20) not null unique,
    budget_year_id  integer not null references budget_year (id),
    receipt_type    varchar(20) not null,
    received_on     date not null,
    source          varchar(200),
    ref_no          varchar(50),
    note            varchar(500),
    created_by      integer not null references users (id),
    created_at      timestamptz not null default now(),
    updated_by      integer not null references users (id),
    updated_at      timestamptz not null default now(),
    cancelled_by    integer references users (id),
    cancelled_at    timestamptz,
    cancel_reason   varchar(500),
    constraint stock_receipt_type_check check (receipt_type in ('receive', 'opening')),
    constraint stock_receipt_cancel_complete check (num_nulls(cancelled_by, cancelled_at, cancel_reason) in (0, 3))
);
create index stock_receipt_received_on_idx on stock_receipt (received_on);
create index stock_receipt_budget_year_id_idx on stock_receipt (budget_year_id);
comment on table stock_receipt is 'หัวใบรับเข้า (รวมยอดยกมา) — บันทึกแล้วลงคลังทันที แก้ได้แค่ source/ref_no/note, ผิดต้องยกเลิกทั้งใบ (ไม่มี soft delete)';
comment on column stock_receipt.doc_no is 'เลขที่ใบรับ RC<ปีงบ>-<ลำดับ 4 หลัก> เช่น RC2570-0001 รันต่อปีงบจาก stock_doc_counter';
comment on column stock_receipt.budget_year_id is 'ปีงบประมาณที่รันเลข budget_year.id (เลือกในฟอร์ม — วันที่รับต้องอยู่ในช่วงของปีงบนี้)';
comment on column stock_receipt.receipt_type is 'receive = รับเข้า, opening = ยอดยกมา (ของที่มีอยู่ตอนเริ่มใช้ระบบ)';
comment on column stock_receipt.received_on is 'วันที่รับ (วันตามเวลาไทย) ไม่เกินวันนี้';
comment on column stock_receipt.source is 'แหล่งที่มา (ข้อความอิสระ ฟอร์มเสนอค่าที่เคยกรอก)';
comment on column stock_receipt.ref_no is 'เลขเอกสารต้นทาง เช่น ใบส่งของ/ใบเบิก';
comment on column stock_receipt.note is 'หมายเหตุ';
comment on column stock_receipt.cancelled_by is 'ผู้ยกเลิก users.id (ยกเลิก = ลง movement reversal คืนยอดทุกบรรทัด)';
comment on column stock_receipt.cancelled_at is 'เวลาที่ยกเลิก';
comment on column stock_receipt.cancel_reason is 'เหตุผลที่ยกเลิก (บังคับ — cancelled_* ครบทั้ง 3 ช่องหรือว่างทั้ง 3, CHECK)';

create table stock_receipt_line (
    id             integer generated always as identity primary key,
    uuid           uuid not null unique default uuid_v7(),
    receipt_id     integer not null references stock_receipt (id),
    lot_id         integer not null references stock_lot (id),
    unit_id        integer not null references item_unit (id),
    unit_factor    integer not null,
    quantity       integer not null,
    base_quantity  integer not null generated always as (quantity * unit_factor) stored,
    sort_order     integer not null default 0,
    created_by     integer not null references users (id),
    created_at     timestamptz not null default now(),
    constraint stock_receipt_line_factor_check check (unit_factor >= 1),
    constraint stock_receipt_line_quantity_check check (quantity > 0)
);
-- ล็อตเดียวกันห้ามซ้ำในใบเดียว (รวมเป็นบรรทัดเดียว) — ครอบการค้นด้วย receipt_id ด้วย
create unique index stock_receipt_line_receipt_lot_uq on stock_receipt_line (receipt_id, lot_id);
create index stock_receipt_line_lot_id_idx on stock_receipt_line (lot_id);
comment on table stock_receipt_line is 'บรรทัดของใบรับ — รายการมาจากล็อต (ไม่เก็บ item_id ซ้ำ) แก้ไม่ได้หลังบันทึก';
comment on column stock_receipt_line.receipt_id is 'stock_receipt.id';
comment on column stock_receipt_line.lot_id is 'ล็อตที่รับเข้า stock_lot.id';
comment on column stock_receipt_line.unit_id is 'หน่วยที่กรอก item_unit.id (หน่วยฐาน หรือหน่วยแปลงของรายการ ณ วันรับ)';
comment on column stock_receipt_line.unit_factor is 'จำนวนหน่วยฐานต่อ 1 หน่วยที่กรอก ณ วันรับ (หน่วยฐาน = 1) — แก้ factor ของรายการภายหลังไม่กระทบใบเดิม';
comment on column stock_receipt_line.quantity is 'จำนวนในหน่วยที่กรอก (มากกว่า 0)';
comment on column stock_receipt_line.base_quantity is 'จำนวนหน่วยฐาน = quantity × unit_factor (generated) เท่ากับ qty_delta ของ movement รับเข้า';
comment on column stock_receipt_line.sort_order is 'ลำดับบรรทัดในใบ';

create table stock_movement (
    id               integer generated always as identity primary key,
    uuid             uuid not null unique default uuid_v7(),
    lot_id           integer not null references stock_lot (id),
    movement_type    varchar(20) not null,
    qty_delta        integer not null,
    receipt_line_id  integer references stock_receipt_line (id),
    reverses_id      integer unique references stock_movement (id),
    created_by       integer not null references users (id),
    created_at       timestamptz not null default now(),
    constraint stock_movement_type_check check (movement_type in ('opening', 'receive', 'reversal')),
    constraint stock_movement_qty_nonzero check (qty_delta <> 0),
    constraint stock_movement_inbound_positive check (movement_type not in ('opening', 'receive') or qty_delta > 0),
    constraint stock_movement_reversal_ref check ((movement_type = 'reversal') = (reverses_id is not null)),
    -- เอกสารต้นทางพอดีหนึ่งชนิด — ขั้นเบิก/ปรับยอดเพิ่มคอลัมน์ FK ของตัวเองแล้วเขียน CHECK นี้ใหม่
    constraint stock_movement_one_source check (num_nonnulls(receipt_line_id) = 1)
);
create index stock_movement_lot_id_idx on stock_movement (lot_id);
create index stock_movement_receipt_line_id_idx on stock_movement (receipt_line_id);
comment on table stock_movement is 'ledger การเคลื่อนไหวของคลัง — เพิ่มได้อย่างเดียว (trigger ห้ามแก้/ลบ) คงเหลือของล็อต = ผลรวม qty_delta';
comment on column stock_movement.lot_id is 'stock_lot.id';
comment on column stock_movement.movement_type is 'opening = ยอดยกมา, receive = รับเข้า, reversal = กลับรายการ (ยกเลิกเอกสาร)';
comment on column stock_movement.qty_delta is 'จำนวนที่เปลี่ยน (หน่วยฐาน) + เข้า / − ออก ไม่เป็น 0';
comment on column stock_movement.receipt_line_id is 'เอกสารต้นทาง stock_receipt_line.id (reversal ของใบรับอ้างบรรทัดเดิม)';
comment on column stock_movement.reverses_id is 'movement ที่ถูกกลับรายการ stock_movement.id (กลับได้ครั้งเดียว — unique)';
comment on column stock_movement.created_by is 'ผู้ทำรายการ users.id';
comment on column stock_movement.created_at is 'เวลาที่ลงรายการ (stock card เรียงตามลำดับนี้)';

-- trigger plpgsql ตัวแรกของโปรเจกต์: กันโค้ดผิด/คำสั่งมือแก้ประวัติ ledger (role ของแอปเป็นเจ้าของตาราง revoke ไม่ช่วย)
create function stock_movement_append_only() returns trigger
language plpgsql as $$
begin
    raise exception 'stock_movement is append-only (% not allowed)', tg_op;
end;
$$;
create trigger stock_movement_no_update_delete
    before update or delete on stock_movement
    for each row execute function stock_movement_append_only();
create trigger stock_movement_no_truncate
    before truncate on stock_movement
    for each statement execute function stock_movement_append_only();

create table stock_doc_counter (
    doc_type        varchar(20) not null,
    budget_year_id  integer not null references budget_year (id),
    last_no         integer not null,
    primary key (doc_type, budget_year_id),
    constraint stock_doc_counter_type_check check (doc_type in ('receipt')),
    constraint stock_doc_counter_positive check (last_no > 0)
);
comment on table stock_doc_counter is 'เลขที่เอกสารล่าสุดต่อชนิดต่อปีงบ — เพิ่มในทรานแซกชันเดียวกับเอกสาร (ล็อกแถว + rollback คืนเลข จึงไม่มีเลขโดด)';
comment on column stock_doc_counter.doc_type is 'ชนิดเอกสาร: receipt = ใบรับเข้า';
comment on column stock_doc_counter.budget_year_id is 'budget_year.id';
comment on column stock_doc_counter.last_no is 'ลำดับล่าสุดที่ใช้ไปแล้ว (ใบถัดไป = last_no + 1)';

-- สิทธิ์ใหม่ 4 ตัว (backend/src/domain/mod.rs) ให้ role ระบบ admin — แบบ migration master คลัง (กฎห้ามให้สิทธิ์เกินตัว)
-- stock_receipt.delete = ยกเลิกใบรับ (หน้าเว็บแสดงป้าย "ยกเลิก")
insert into role_permissions (role_id, permission)
select r.id, p.permission
from roles r
cross join unnest(array[
    'stock_receipt.view', 'stock_receipt.create', 'stock_receipt.update', 'stock_receipt.delete'
]) as p(permission)
where r.name = 'admin' and r.is_system
on conflict do nothing;

-- กลุ่มเมนูใหม่ "คลัง" (หัวข้อกลุ่ม = route null) ต่อท้ายเมนูชั้นบน + หน้า "รับเข้า"
-- แตะเฉพาะแถวใหม่: ไม่ขยับลำดับที่เจ้าของจัดไว้ ย้ายกลุ่ม/ลำดับเองได้ที่หน้าการตั้งค่าเพจ
with stock_group as (
    insert into pages (label, icon, route, sort_order)
    select 'คลัง', 'warehouse', null, coalesce(max(sort_order), 0) + 10
    from pages
    where ref_id is null
    returning id
)
insert into pages (label, icon, route, sort_order, ref_id)
select 'รับเข้า', 'package-plus', '/stock/receipts', 10, id
from stock_group;

insert into page_permissions (page_id, permission)
select id, 'stock_receipt.view' from pages where route = '/stock/receipts';
