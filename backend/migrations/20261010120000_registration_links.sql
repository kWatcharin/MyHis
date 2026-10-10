-- ลงทะเบียนผู้ใช้งานผ่านลิงก์ (docs/auth-design.md ข้อ 4, 5, 8): แอดมินสร้างบัญชี/รีเซ็ตรหัส → ได้ลิงก์ที่ใช้ได้
-- ครั้งเดียว อายุ 3 วัน ผู้ใช้เปิดลิงก์ กรอกข้อมูลตัวเอง (คำนำหน้า ชื่อ สกุล ตำแหน่ง) แล้วตั้งรหัสผ่านเอง
-- เลิกใช้รหัสผ่านชั่วคราว — แอดมินไม่เห็นรหัสผ่านของใครอีก

-- ข้อมูลผู้ใช้งาน (เจ้าหน้าที่ ไม่ใช่ข้อมูลผู้ป่วย จึงเก็บ plaintext ได้) nullable ทั้งหมด: ผู้ใช้เดิมยังไม่มี และผู้ใช้ใหม่
-- กรอกเองตอนลงทะเบียน
-- registered_at default now(): แถวที่ insert โดยไม่ระบุ (bootstrap admin จาก env, helper ของเทส) ถือว่าลงทะเบียนแล้ว
-- มีแค่การสร้างผู้ใช้ผ่านหน้าแอดมิน (users module) ที่ใส่ null เองจนกว่าผู้ใช้จะตั้งรหัสผ่านผ่านลิงก์
alter table users
    add column prename_id    integer references prename (id),
    add column first_name    text,
    add column last_name     text,
    add column position      text,
    add column registered_at timestamptz default now();

-- ผู้ใช้เดิมทุกคนมีรหัสผ่านของตัวเองอยู่แล้ว (สร้างก่อนมีลิงก์) — นับว่าลงทะเบียนตั้งแต่ตอนสร้างบัญชี
update users set registered_at = created_at;

comment on column users.prename_id is 'prename.id คำนำหน้า (ผู้ใช้กรอกเองตอนลงทะเบียน แอดมินแก้ได้)';
comment on column users.first_name is 'ชื่อจริง (ผู้ใช้กรอกเองตอนลงทะเบียน)';
comment on column users.last_name is 'นามสกุล (ผู้ใช้กรอกเองตอนลงทะเบียน)';
comment on column users.position is 'ตำแหน่ง เช่น พยาบาลวิชาชีพ (ไม่บังคับ)';
comment on column users.registered_at is 'ตั้งรหัสผ่านเองผ่านลิงก์ลงทะเบียนสำเร็จเมื่อ — null = ยังไม่ลงทะเบียน (login ไม่ได้)';
comment on column users.must_change_password is
    'ยังใช้รหัสผ่านชั่วคราว (bootstrap admin และบัญชีเดิมก่อนมีลิงก์ลงทะเบียน) — การรีเซ็ตใช้ลิงก์แทนแล้ว ไม่ตั้งค่านี้';

-- ลิงก์ลงทะเบียน/ตั้งรหัสผ่านใหม่ — แพทเทิร์นเดียวกับ sessions: คีย์คือ hash ของ token ไม่เก็บ token ดิบ ไม่มี id/uuid
-- (ไม่ใช่ entity ที่ frontend อ้างถึงด้วย id) ไม่ลบแถว — ใช้แล้วตั้ง used_at, ถูกแทนด้วยลิงก์ใหม่ตั้ง revoked_at
create table registration_links (
    token_hash  text primary key,
    user_id     integer not null references users (id) on delete cascade,
    created_by  integer references users (id) on delete set null,
    created_at  timestamptz not null default now(),
    expires_at  timestamptz not null,
    used_at     timestamptz,
    revoked_at  timestamptz
);

create index registration_links_user_id_idx on registration_links (user_id);

comment on table registration_links is 'ลิงก์ลงทะเบียน/ตั้งรหัสผ่านใหม่ที่แอดมินออกให้ ใช้ได้ครั้งเดียว';
comment on column registration_links.token_hash is 'SHA-256 ของ token ในลิงก์ (ไม่เก็บ token ดิบ — แบบ sessions.token_hash)';
comment on column registration_links.user_id is 'users.id บัญชีที่ลิงก์นี้ใช้ตั้งรหัสผ่าน';
comment on column registration_links.created_by is 'users.id แอดมินที่ออกลิงก์';
comment on column registration_links.expires_at is 'หมดอายุ (ออกลิงก์ + 3 วัน)';
comment on column registration_links.used_at is 'ผู้ใช้ลงทะเบียน/ตั้งรหัสสำเร็จเมื่อ (ใช้ซ้ำไม่ได้)';
comment on column registration_links.revoked_at is 'ถูกแทนด้วยลิงก์ใหม่เมื่อ (ใช้ไม่ได้แล้ว)';
