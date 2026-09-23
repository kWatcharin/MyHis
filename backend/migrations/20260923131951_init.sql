-- Milestone 1: schema เต็มของ auth ตาม docs/auth-design.md หัวข้อ 4
-- users + roles/role_permissions/user_roles (many-to-many, docs/auth-design.md หัวข้อ 10) +
-- sessions + audit_log
--
-- ไม่ seed ผู้ใช้ใด ๆ ที่นี่ — ผู้ใช้ admin คนแรกสร้างตอน app bootstrap จาก environment variable
-- เท่านั้น (docs/auth-design.md หัวข้อ 12: ห้าม hard-code รหัสผ่านไว้ใน migration)
--
-- gen_random_uuid() เป็นฟังก์ชัน built-in ตั้งแต่ PostgreSQL 13 ไม่ต้อง CREATE EXTENSION

create table users (
    id                    uuid primary key default gen_random_uuid(),
    username              text not null unique,
    display_name          text not null,
    password_hash         text not null,
    is_active             boolean not null default true,
    must_change_password  boolean not null default false,
    failed_attempts       integer not null default 0,
    locked_until          timestamptz,
    created_at            timestamptz not null default now(),
    updated_at            timestamptz not null default now()
);
-- หมายเหตุ: updated_at ไม่มี trigger auto-update — service layer ต้องตั้งค่าเองตอน UPDATE ทุกครั้ง

create table roles (
    id           uuid primary key default gen_random_uuid(),
    name         text not null unique,
    description  text,
    is_system    boolean not null default false,
    created_at   timestamptz not null default now(),
    updated_at   timestamptz not null default now()
);
-- is_system = true ห้ามลบ/เปลี่ยนชื่อ — บังคับที่ service layer (ดู docs/auth-design.md หัวข้อ 4)

create table role_permissions (
    role_id     uuid not null references roles (id) on delete cascade,
    permission  text not null, -- ต้องตรงกับ Permission enum ฝั่ง Rust เสมอ (parse ตอนอ่าน)
    primary key (role_id, permission)
);

create table user_roles (
    user_id      uuid not null references users (id) on delete cascade,
    role_id      uuid not null references roles (id) on delete restrict, -- ห้ามลบ role ที่ยังมีคนถือ
    assigned_at  timestamptz not null default now(),
    primary key (user_id, role_id)
);

create index user_roles_role_id_idx on user_roles (role_id);

create table sessions (
    token_hash           text primary key,
    user_id              uuid not null references users (id) on delete cascade,
    created_at           timestamptz not null default now(),
    last_seen_at         timestamptz not null default now(),
    idle_expires_at      timestamptz not null,
    absolute_expires_at  timestamptz not null,
    user_agent           text
);

create index sessions_user_id_idx on sessions (user_id);

create table audit_log (
    id             bigserial primary key,
    actor_user_id  uuid references users (id) on delete set null, -- เก็บ row ไว้แม้ user ถูกลบ
    action         text not null,
    target_type    text,
    target_id      text,
    metadata       jsonb,
    ip             inet,
    created_at     timestamptz not null default now()
);

-- Seed role ที่เป็น is_system (ต้องมีก่อนถึงจะ bootstrap admin คนแรกได้ — หัวข้อ 12)
insert into roles (name, description, is_system) values
    ('admin', 'ดูแลระบบ สร้าง/ปิดบัญชีผู้ใช้ และจัดการ role', true),
    ('nurse', 'งานพยาบาล ดูและบันทึกตารางเวร', true);

-- Seed permission เริ่มต้นของ role ที่ seed มา — เป็นค่าเริ่มต้นที่สมเหตุสมผลตามชื่อ role เท่านั้น
-- แก้ภายหลังได้ผ่าน PATCH /roles/{id} โดยไม่ต้อง deploy ใหม่ (นี่คือประเด็นของการทำตารางนี้)
insert into role_permissions (role_id, permission)
select id, permission
from roles, unnest(array['user.manage', 'shift.read', 'shift.write', 'audit.read']) as permission
where roles.name = 'admin';

insert into role_permissions (role_id, permission)
select id, permission
from roles, unnest(array['shift.read', 'shift.write']) as permission
where roles.name = 'nurse';
