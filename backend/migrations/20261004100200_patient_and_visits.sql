-- ตารางคนไข้และการนัดหมาย ตาม docs/my-his-data-dict.xlsx: patient, visit_group, medical_condition,
-- lab_visit_group, lab_visit (รูปแบบ id/uuid/audit เหมือน migration master_tables)
--
-- patient: คอลัมน์ที่ data dict ระบุ encrypt = yes (hn, nap_no, first_name, last_name) เก็บเป็น bytea
-- ciphertext ที่เข้ารหัสที่ชั้น app (AES-256-GCM, backend/src/crypto.rs) — DB ไม่เห็น plaintext/key
-- ค้นหาด้วย hn / nap_no แบบ "เท่ากับ" ผ่าน blind index (HMAC-SHA256) ใน *_idx เท่านั้น
-- ห้ามใช้ argon2 กับคอลัมน์เหล่านี้: salt สุ่มทำให้ค้นด้วย `=` ไม่ได้ และถอดค่ากลับมาแสดงไม่ได้
--
-- ชื่อคอลัมน์ที่แก้จาก data dict (typo/กำกวม): national_id → nationality_id (กันสับสนกับเลขบัตร
-- ประชาชน), lab_visit_group.budget_year → budget_year_id, patien_id → patient_id,
-- visit_group.patient_type_id → patient_group_id

create table patient (
    id              integer generated always as identity primary key,
    uuid            uuid not null unique default uuid_v7(),
    hn              bytea not null,
    hn_idx          bytea not null,
    nap_no          bytea not null,
    nap_no_idx      bytea not null,
    sort_order      integer not null default 0,
    prename_id      integer not null references prename (id),
    first_name      bytea not null,
    last_name       bytea not null,
    birth_date      date,
    age             integer check (age >= 0),
    weight          numeric(5, 2),
    height          numeric(5, 2),
    sex             integer not null check (sex in (1, 2)),
    is_thai         boolean not null,
    nationality_id  integer references nationality (id),
    is_active       boolean not null default true,
    created_by      integer not null references users (id),
    created_at      timestamptz not null default now(),
    updated_by      integer not null references users (id),
    updated_at      timestamptz not null default now(),
    deleted_by      integer references users (id),
    deleted_at      timestamptz
);
-- hn ซ้ำกันไม่ได้ (นับเฉพาะแถวที่ยังไม่ถูกลบ) — บังคับผ่าน blind index
create unique index patient_hn_idx_uq on patient (hn_idx) where deleted_at is null;
create index patient_nap_no_idx_idx on patient (nap_no_idx);
create index patient_prename_id_idx on patient (prename_id);
create index patient_nationality_id_idx on patient (nationality_id);
comment on table patient is 'master: คนไข้ (คอลัมน์ที่ encrypt เก็บเป็น ciphertext ที่เข้ารหัสโดย app)';
comment on column patient.hn is 'Hospital number (encrypt)';
comment on column patient.hn_idx is 'blind index ของ hn (HMAC-SHA256) ไว้ค้นหาแบบ = เท่านั้น';
comment on column patient.nap_no is 'เลขประจำตัวคนไข้ (encrypt)';
comment on column patient.nap_no_idx is 'blind index ของ nap_no (HMAC-SHA256) ไว้ค้นหาแบบ = เท่านั้น';
comment on column patient.sort_order is 'ลำดับ';
comment on column patient.prename_id is 'id คำนำหน้า prename.id';
comment on column patient.first_name is 'ชื่อ (encrypt)';
comment on column patient.last_name is 'สกุล (encrypt)';
comment on column patient.birth_date is 'วันเดือนปีเกิด';
comment on column patient.age is 'อายุ';
comment on column patient.weight is 'น้ำหนัก (kg)';
comment on column patient.height is 'ส่วนสูง (cm)';
comment on column patient.sex is '1 = male, 2 = female';
comment on column patient.is_thai is 'คนไทยหรือไม่';
comment on column patient.nationality_id is 'สัญชาติ nationality.id';
comment on column patient.is_active is 'ใช้งานอยู่หรือไม่';

create table visit_group (
    id                integer generated always as identity primary key,
    uuid              uuid not null unique default uuid_v7(),
    patient_group_id  integer not null references patient_group (id),
    seq               integer not null default 0,
    created_by        integer not null references users (id),
    created_at        timestamptz not null default now(),
    updated_by        integer not null references users (id),
    updated_at        timestamptz not null default now(),
    deleted_by        integer references users (id),
    deleted_at        timestamptz
);
create index visit_group_patient_group_id_idx on visit_group (patient_group_id);
comment on table visit_group is 'กลุ่มคนไข้ที่มา';
comment on column visit_group.patient_group_id is 'กลุ่มคนไข้ patient_group.id';
comment on column visit_group.seq is 'ลำดับ';

create table medical_condition (
    id          integer generated always as identity primary key,
    uuid        uuid not null unique default uuid_v7(),
    disease_id  integer not null references disease (id),
    patient_id  integer not null references patient (id),
    sort_order  integer not null default 0,
    is_active   boolean not null default true,
    created_by  integer not null references users (id),
    created_at  timestamptz not null default now(),
    updated_by  integer not null references users (id),
    updated_at  timestamptz not null default now(),
    deleted_by  integer references users (id),
    deleted_at  timestamptz
);
-- คนไข้คนเดียวมีโรคเดียวกันซ้ำไม่ได้ (นับเฉพาะแถวที่ยังไม่ถูกลบ)
create unique index medical_condition_patient_disease_uq
    on medical_condition (patient_id, disease_id) where deleted_at is null;
create index medical_condition_disease_id_idx on medical_condition (disease_id);
comment on table medical_condition is 'โรคประจำตัวของคนป่วย';
comment on column medical_condition.disease_id is 'disease.id';
comment on column medical_condition.patient_id is 'patient.id';
comment on column medical_condition.sort_order is 'ลำดับ';
comment on column medical_condition.is_active is 'ใช้งานอยู่หรือไม่';

create table lab_visit_group (
    id                integer generated always as identity primary key,
    uuid              uuid not null unique default uuid_v7(),
    budget_year_id    integer not null references budget_year (id),
    patient_group_id  integer not null references patient_group (id),
    cd4_1st_date      date not null,
    cd4_2nd_date      date not null,
    vl_date           date not null,
    sort_order        integer not null default 0,
    created_by        integer not null references users (id),
    created_at        timestamptz not null default now(),
    updated_by        integer not null references users (id),
    updated_at        timestamptz not null default now(),
    deleted_by        integer references users (id),
    deleted_at        timestamptz
);
create unique index lab_visit_group_year_group_uq
    on lab_visit_group (budget_year_id, patient_group_id) where deleted_at is null;
create index lab_visit_group_patient_group_id_idx on lab_visit_group (patient_group_id);
comment on table lab_visit_group is 'นัดหมายกลุ่ม lab';
comment on column lab_visit_group.budget_year_id is 'ปีงบประมาณ budget_year.id';
comment on column lab_visit_group.patient_group_id is 'กลุ่มคนไข้ patient_group.id';
comment on column lab_visit_group.cd4_1st_date is 'วันนัดตรวจ cd4 ครั้งที่ 1';
comment on column lab_visit_group.cd4_2nd_date is 'วันนัดตรวจ cd4 ครั้งที่ 2';
comment on column lab_visit_group.vl_date is 'วันนัด vl';
comment on column lab_visit_group.sort_order is 'ลำดับ';

create table lab_visit (
    id                    integer generated always as identity primary key,
    uuid                  uuid not null unique default uuid_v7(),
    patient_id            integer not null references patient (id),
    lab_visit_group_id    integer not null references lab_visit_group (id),
    vl_result             text,
    drug_recipe           text,
    take_med_month_range  integer check (take_med_month_range >= 0),
    take_med_year_range   integer check (take_med_year_range >= 0),
    sort_order            integer not null default 0,
    created_by            integer not null references users (id),
    created_at            timestamptz not null default now(),
    updated_by            integer not null references users (id),
    updated_at            timestamptz not null default now(),
    deleted_by            integer references users (id),
    deleted_at            timestamptz
);
create index lab_visit_patient_id_idx on lab_visit (patient_id);
create index lab_visit_lab_visit_group_id_idx on lab_visit (lab_visit_group_id);
comment on table lab_visit is 'นัดหมาย lab';
comment on column lab_visit.patient_id is 'patient.id';
comment on column lab_visit.lab_visit_group_id is 'กลุ่ม lab_visit_group.id';
comment on column lab_visit.vl_result is 'ผล vl';
comment on column lab_visit.drug_recipe is 'สูตรยา';
comment on column lab_visit.take_med_month_range is 'ระยะเวลากินยา: ส่วนที่เป็นเดือน';
comment on column lab_visit.take_med_year_range is 'ระยะเวลากินยา: ส่วนที่เป็นปี';
comment on column lab_visit.sort_order is 'ลำดับ';
