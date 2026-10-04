-- ตาราง master ตาม docs/my-his-data-dict.xlsx: prename, nationality, disease, patient_group, lab,
-- budget_year
--
-- รูปแบบร่วมของทุกตาราง (เหมือน users/roles/pages หลัง migration 20261004100000):
--   id    integer identity = pk ใช้ join/FK ภายใน backend เท่านั้น
--   uuid  uuidv7 unique    = id ที่ frontend/API เห็น (ไม่เปิดเผย id ลำดับ)
--   audit created_by/updated_by (users.id), created_at/updated_at (UTC), soft delete ผ่าน
--   deleted_by/deleted_at — updated_at ไม่มี trigger service ตั้งเองตอน UPDATE (ตามที่ทำมาตลอด)
-- ยังไม่ seed ข้อมูล — ตารางว่างจนกว่าจะมีฟีเจอร์จัดการ master

create table prename (
    id            integer generated always as identity primary key,
    uuid          uuid not null unique default uuid_v7(),
    name          text not null,
    abbreviation  text,
    sort_order    integer not null default 0,
    created_by    integer not null references users (id),
    created_at    timestamptz not null default now(),
    updated_by    integer not null references users (id),
    updated_at    timestamptz not null default now(),
    deleted_by    integer references users (id),
    deleted_at    timestamptz
);
comment on table prename is 'master: คำนำหน้า';
comment on column prename.name is 'คำนำหน้าเต็ม';
comment on column prename.abbreviation is 'คำนำหน้าย่อ';
comment on column prename.sort_order is 'ลำดับ';

create table nationality (
    id             integer generated always as identity primary key,
    uuid           uuid not null unique default uuid_v7(),
    national_name  varchar(100) not null,
    nation         varchar(100) not null,
    sort_order     integer not null default 0,
    is_active      boolean not null default true,
    created_by     integer not null references users (id),
    created_at     timestamptz not null default now(),
    updated_by     integer not null references users (id),
    updated_at     timestamptz not null default now(),
    deleted_by     integer references users (id),
    deleted_at     timestamptz
);
comment on table nationality is 'master: สัญชาติ';
comment on column nationality.national_name is 'เชื้อชาติ/สัญชาติ';
comment on column nationality.nation is 'ประเทศ';
comment on column nationality.sort_order is 'ลำดับ';
comment on column nationality.is_active is 'ใช้งานอยู่หรือไม่';

create table disease (
    id            integer generated always as identity primary key,
    uuid          uuid not null unique default uuid_v7(),
    name          varchar(100) not null,
    abbreviation  varchar(100) not null,
    sort_order    integer not null default 0,
    is_active     boolean not null default true,
    created_by    integer not null references users (id),
    created_at    timestamptz not null default now(),
    updated_by    integer not null references users (id),
    updated_at    timestamptz not null default now(),
    deleted_by    integer references users (id),
    deleted_at    timestamptz
);
comment on table disease is 'master: โรค';
comment on column disease.name is 'ชื่อโรค';
comment on column disease.abbreviation is 'คำย่อ';
comment on column disease.sort_order is 'ลำดับ';
comment on column disease.is_active is 'ใช้งานอยู่หรือไม่';

create table patient_group (
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
comment on table patient_group is 'master: กลุ่มคนไข้';
comment on column patient_group.name is 'ชื่อกลุ่มคนไข้';
comment on column patient_group.sort_order is 'ลำดับ';
comment on column patient_group.is_active is 'ใช้งานอยู่หรือไม่';

create table lab (
    id            integer generated always as identity primary key,
    uuid          uuid not null unique default uuid_v7(),
    name          text not null,
    abbreviation  text,
    is_active     boolean not null default true,
    sort_order    integer not null default 0,
    created_by    integer not null references users (id),
    created_at    timestamptz not null default now(),
    updated_by    integer not null references users (id),
    updated_at    timestamptz not null default now(),
    deleted_by    integer references users (id),
    deleted_at    timestamptz
);
comment on table lab is 'master: แลป';
comment on column lab.name is 'ชื่อเต็ม';
comment on column lab.abbreviation is 'คำย่อ';
comment on column lab.is_active is 'ใช้งานอยู่หรือไม่';
comment on column lab.sort_order is 'ลำดับ';

create table budget_year (
    id          integer generated always as identity primary key,
    uuid        uuid not null unique default uuid_v7(),
    year        varchar(4) not null check (year ~ '^[0-9]{4}$'),
    is_active   boolean not null default true,
    sort_order  integer not null default 0,
    start_date  date not null,
    end_date    date not null,
    created_by  integer not null references users (id),
    created_at  timestamptz not null default now(),
    updated_by  integer not null references users (id),
    updated_at  timestamptz not null default now(),
    deleted_by  integer references users (id),
    deleted_at  timestamptz,
    check (end_date >= start_date)
);
-- ปีงบประมาณซ้ำกันไม่ได้ (นับเฉพาะแถวที่ยังไม่ถูกลบ)
create unique index budget_year_year_uq on budget_year (year) where deleted_at is null;
comment on table budget_year is 'master: ปีงบประมาณ';
comment on column budget_year.year is 'ปีงบประมาณ (พ.ศ.) เช่น 2570';
comment on column budget_year.is_active is 'ใช้งานอยู่หรือไม่';
comment on column budget_year.sort_order is 'ลำดับ';
comment on column budget_year.start_date is 'วันที่เริ่มต้น';
comment on column budget_year.end_date is 'วันที่สิ้นสุด';
