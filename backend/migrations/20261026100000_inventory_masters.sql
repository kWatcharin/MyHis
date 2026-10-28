-- master ของระบบคลัง (docs/inventory-design.md ข้อ 6): หมวด, หน่วยนับ, รายการ + หน่วยแปลงของรายการ
-- รูปแบบร่วมเหมือน master เดิม (20261004100100_master_tables.sql): id identity, uuid v7, audit, soft delete
-- ยังไม่ seed ข้อมูล — ผู้ใช้สร้างเองผ่านหน้าตั้งค่า (created_by ต้องอ้างผู้ใช้จริง)
--
-- วันหมดอายุเก็บที่ล็อต (ตาราง stock ที่จะตามมา) และรายการที่ไม่ติดตามล็อตมีล็อตไม่ระบุล็อตเดียว
-- → ติดตามวันหมดอายุต้องติดตามล็อตด้วยเสมอ (CHECK *_expiry_requires_lot) ไม่งั้นแยกวันหมดอายุของคนละรอบไม่ได้
-- จำนวนทุกช่องเป็น integer ในหน่วยฐาน (หน่วยเล็กสุดที่เบิกจริง เช่น เม็ด) — ไม่มีเศษ

create table item_category (
    id                        integer generated always as identity primary key,
    uuid                      uuid not null unique default uuid_v7(),
    name                      varchar(100) not null,
    default_track_lot         boolean not null default false,
    default_track_expiry      boolean not null default false,
    default_patient_linkable  boolean not null default false,
    sort_order                integer not null default 0,
    is_active                 boolean not null default true,
    created_by                integer not null references users (id),
    created_at                timestamptz not null default now(),
    updated_by                integer not null references users (id),
    updated_at                timestamptz not null default now(),
    deleted_by                integer references users (id),
    deleted_at                timestamptz,
    constraint item_category_expiry_requires_lot check (not default_track_expiry or default_track_lot)
);
-- ชื่อหมวดซ้ำกันไม่ได้ (ไม่สนตัวพิมพ์เล็ก/ใหญ่ นับเฉพาะแถวที่ยังไม่ถูกลบ)
create unique index item_category_name_uq on item_category (lower(name)) where deleted_at is null;
comment on table item_category is 'master: หมวดของรายการคลัง';
comment on column item_category.name is 'ชื่อหมวด (ห้ามซ้ำ ไม่สนตัวพิมพ์ นับเฉพาะแถวที่ยังไม่ถูกลบ)';
comment on column item_category.default_track_lot is 'ค่าตั้งต้นของ item.track_lot ตอนสร้างรายการใหม่ — แก้ภายหลังไม่กระทบรายการเดิม';
comment on column item_category.default_track_expiry is 'ค่าตั้งต้นของ item.track_expiry ตอนสร้างรายการใหม่ (ต้องติดตามล็อตด้วย) — แก้ภายหลังไม่กระทบรายการเดิม';
comment on column item_category.default_patient_linkable is 'ค่าตั้งต้นของ item.patient_linkable ตอนสร้างรายการใหม่ — แก้ภายหลังไม่กระทบรายการเดิม';
comment on column item_category.sort_order is 'ลำดับ';
comment on column item_category.is_active is 'ใช้งานอยู่หรือไม่ (ปิด = เลือกให้รายการใหม่ไม่ได้ รายการเดิมยังอยู่หมวดนี้)';

create table item_unit (
    id          integer generated always as identity primary key,
    uuid        uuid not null unique default uuid_v7(),
    name        varchar(100) not null,
    sort_order  integer not null default 0,
    is_active   boolean not null default true,
    created_by  integer not null references users (id),
    created_at  timestamptz not null default now(),
    updated_by  integer not null references users (id),
    updated_at  timestamptz not null default now(),
    deleted_by  integer references users (id),
    deleted_at  timestamptz
);
create unique index item_unit_name_uq on item_unit (lower(name)) where deleted_at is null;
comment on table item_unit is 'master: หน่วยนับของรายการคลัง (ชื่อหน่วยใช้ร่วมกันทุกรายการ อัตราแปลงอยู่ที่ item_unit_conversion)';
comment on column item_unit.name is 'ชื่อหน่วย เช่น เม็ด แผง กล่อง (ห้ามซ้ำ ไม่สนตัวพิมพ์ นับเฉพาะแถวที่ยังไม่ถูกลบ)';
comment on column item_unit.sort_order is 'ลำดับ';
comment on column item_unit.is_active is 'ใช้งานอยู่หรือไม่ (ปิด = เลือกให้รายการใหม่ไม่ได้ รายการเดิมใช้ต่อได้)';

create table item (
    id                integer generated always as identity primary key,
    uuid              uuid not null unique default uuid_v7(),
    name              varchar(100) not null,
    category_id       integer not null references item_category (id),
    base_unit_id      integer not null references item_unit (id),
    -- ธงไม่มี default โดยตั้งใจ: service ใส่เสมอ (จากฟอร์ม หรือคัดลอกค่าตั้งต้นของหมวดตอนสร้าง)
    track_lot         boolean not null,
    track_expiry      boolean not null,
    patient_linkable  boolean not null,
    reorder_point     integer check (reorder_point >= 0),
    is_active         boolean not null default true,
    created_by        integer not null references users (id),
    created_at        timestamptz not null default now(),
    updated_by        integer not null references users (id),
    updated_at        timestamptz not null default now(),
    deleted_by        integer references users (id),
    deleted_at        timestamptz,
    constraint item_expiry_requires_lot check (not track_expiry or track_lot)
);
-- ชื่อรายการซ้ำกันไม่ได้ทั้งระบบ (ไม่ใช่แค่ในหมวด) — ตอนเบิกเลือกจากชื่อ
create unique index item_name_uq on item (lower(name)) where deleted_at is null;
create index item_category_id_idx on item (category_id);
create index item_base_unit_id_idx on item (base_unit_id);
comment on table item is 'master: รายการคลัง (ยา เวชภัณฑ์ วัสดุแล็บ)';
comment on column item.name is 'ชื่อรายการ เช่น พาราเซตามอล 500 mg (ห้ามซ้ำ ไม่สนตัวพิมพ์ นับเฉพาะแถวที่ยังไม่ถูกลบ)';
comment on column item.category_id is 'หมวด item_category.id';
comment on column item.base_unit_id is 'หน่วยฐาน item_unit.id = หน่วยเล็กสุดที่เบิกจริง จำนวนทุกช่องของรายการนี้นับเป็นหน่วยนี้';
comment on column item.track_lot is 'ติดตามล็อตหรือไม่ (ไม่ติดตาม = ของทั้งหมดอยู่ในล็อตไม่ระบุล็อตเดียว)';
comment on column item.track_expiry is 'ติดตามวันหมดอายุหรือไม่ (ต้องติดตามล็อตด้วย เพราะวันหมดอายุเก็บที่ล็อต)';
comment on column item.patient_linkable is 'ผูกผู้ป่วยตอนเบิกได้หรือไม่ (ผูกได้ = ระบุผู้ป่วยหรือไม่ก็ได้)';
comment on column item.reorder_point is 'จุดสั่งซื้อ (หน่วยฐาน) เตือนเมื่อคงเหลือ <= ค่านี้ — null = ไม่ตั้ง, ห้ามติดลบ (CHECK)';
comment on column item.is_active is 'ใช้งานอยู่หรือไม่';

create table item_unit_conversion (
    id          integer generated always as identity primary key,
    uuid        uuid not null unique default uuid_v7(),
    item_id     integer not null references item (id),
    unit_id     integer not null references item_unit (id),
    factor      integer not null check (factor > 1),
    created_by  integer not null references users (id),
    created_at  timestamptz not null default now(),
    updated_by  integer not null references users (id),
    updated_at  timestamptz not null default now(),
    deleted_by  integer references users (id),
    deleted_at  timestamptz
);
-- รายการเดียวมีหน่วยเดียวกันซ้ำไม่ได้ (นับเฉพาะแถวที่ยังไม่ถูกลบ) — ครอบการค้นด้วย item_id ด้วย
create unique index item_unit_conversion_item_unit_uq
    on item_unit_conversion (item_id, unit_id) where deleted_at is null;
create index item_unit_conversion_unit_id_idx on item_unit_conversion (unit_id);
comment on table item_unit_conversion is 'หน่วยอื่นของรายการ (นอกจากหน่วยฐาน) และอัตราแปลงเป็นหน่วยฐาน';
comment on column item_unit_conversion.item_id is 'item.id';
comment on column item_unit_conversion.unit_id is 'item_unit.id (ไม่ใช่หน่วยฐานของรายการ — service ตรวจ)';
comment on column item_unit_conversion.factor is 'จำนวนหน่วยฐานต่อ 1 หน่วยนี้ (มากกว่า 1 — CHECK) เทียบหน่วยฐานเสมอ (ไม่ต่อทอด) เช่น 1 กล่อง = 100 (เม็ด)';

-- สิทธิ์ใหม่ 12 ตัว (backend/src/domain/mod.rs) ให้ role ระบบ admin — ต้องทำด้วย migration เพราะกฎห้ามให้สิทธิ์เกินตัวเอง
-- (docs/auth-design.md ข้อ 10): ก่อนนี้ไม่มีใครถือ จึงไม่มีใครติ๊กให้ใครได้ แตะเฉพาะแถวของ admin — role อื่นผู้ดูแลติ๊กเอง
insert into role_permissions (role_id, permission)
select r.id, p.permission
from roles r
cross join unnest(array[
    'item_category.view', 'item_category.create', 'item_category.update', 'item_category.delete',
    'item_unit.view', 'item_unit.create', 'item_unit.update', 'item_unit.delete',
    'item.view', 'item.create', 'item.update', 'item.delete'
]) as p(permission)
where r.name = 'admin' and r.is_system
on conflict do nothing;

-- เพจ 3 หน้าต่อท้ายกลุ่มเมนู "การตั้งค่า" (หัวข้อกลุ่ม = แถวที่ route เป็น null — 20261012100000_page_ref_id.sql)
-- แตะเฉพาะแถวใหม่: ลำดับต่อจากเพจสุดท้ายของกลุ่ม ไม่ขยับลำดับที่เจ้าของจัดไว้ผ่านหน้าการตั้งค่าเพจ
with settings_group as (
    select id from pages where route is null and label = 'การตั้งค่า'
), last_page as (
    select coalesce(max(p.sort_order), 0) as sort_order
    from pages p
    join settings_group g on p.ref_id = g.id
)
insert into pages (label, icon, route, sort_order, ref_id)
select v.label, v.icon, v.route, last_page.sort_order + v.step, (select id from settings_group)
from (values
    ('ตั้งค่าหมวดคลัง', 'tags', '/settings/item-categories', 10),
    ('ตั้งค่าหน่วยนับ', 'ruler', '/settings/item-units', 20),
    ('ตั้งค่ารายการคลัง', 'package', '/settings/items', 30)
) as v(label, icon, route, step)
cross join last_page;

insert into page_permissions (page_id, permission)
select p.id, v.permission
from pages p
join (values
    ('/settings/item-categories', 'item_category.view'),
    ('/settings/item-units', 'item_unit.view'),
    ('/settings/items', 'item.view')
) as v(route, permission) on v.route = p.route;
