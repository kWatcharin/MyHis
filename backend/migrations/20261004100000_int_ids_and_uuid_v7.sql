-- users / roles / pages: เปลี่ยน pk จาก uuid เป็น `id integer` (identity) สำหรับ join/FK ภายใน
-- และเก็บ uuid เดิมไว้เป็นคอลัมน์ `uuid` (unique) ให้ frontend/API ใช้ — ค่า uuid ของแถวเดิมไม่เปลี่ยน
-- API, cookie และ client ที่อ้าง uuid อยู่จึงไม่กระทบ
--
-- ไม่แตะ audit_log.id (bigserial อยู่แล้ว), sessions (pk = token_hash) และตารางกลาง
-- (user_roles/role_permissions/page_permissions — ไม่มี entity ของตัวเอง) ยกเว้น FK ที่ชี้ไปหา 3 ตารางข้างบน
--
-- ลำดับ: เพิ่มคอลัมน์ int ใหม่ + backfill → drop คอลัมน์ uuid FK เก่าของตารางลูก (constraint/index ที่
-- เกี่ยวข้องหลุดไปด้วย) → แปลง pk ของตารางแม่ → rename + สร้าง constraint/index ของตารางลูกกลับ

-- UUIDv7 (เรียงตามเวลา) — PostgreSQL 16 ยังไม่มี uuidv7() built-in (มาใน 18)
-- ใช้ gen_random_uuid() (v4) แล้วเขียน timestamp ms ทับ 48 บิตแรก และตั้ง version nibble เป็น 7
create function uuid_v7() returns uuid
language sql
volatile
as $$
    select encode(
        set_bit(
            set_bit(
                overlay(
                    uuid_send(gen_random_uuid())
                    placing substring(int8send(floor(extract(epoch from clock_timestamp()) * 1000)::bigint) from 3)
                    from 1 for 6
                ),
                52, 1
            ),
            53, 1
        ),
        'hex'
    )::uuid
$$;

-- 1) id ใหม่ของตารางแม่ (identity เติมให้แถวเดิมอัตโนมัติ)
alter table users add column id_new integer generated always as identity;
alter table roles add column id_new integer generated always as identity;
alter table pages add column id_new integer generated always as identity;

-- 2) คอลัมน์ FK int ของตารางลูก + backfill ด้วย join ตาม uuid เดิม
alter table user_roles
    add column user_id_new integer,
    add column role_id_new integer;
update user_roles ur set user_id_new = u.id_new from users u where u.id = ur.user_id;
update user_roles ur set role_id_new = r.id_new from roles r where r.id = ur.role_id;

alter table role_permissions add column role_id_new integer;
update role_permissions rp set role_id_new = r.id_new from roles r where r.id = rp.role_id;

alter table page_permissions add column page_id_new integer;
update page_permissions pp set page_id_new = p.id_new from pages p where p.id = pp.page_id;

alter table sessions add column user_id_new integer;
update sessions s set user_id_new = u.id_new from users u where u.id = s.user_id;

alter table audit_log add column actor_user_id_new integer;
update audit_log a set actor_user_id_new = u.id_new from users u where u.id = a.actor_user_id;

-- 3) drop คอลัมน์ uuid FK เก่า (พร้อม pk/fk/index ที่เกี่ยวข้อง: user_roles_pkey, user_roles_role_id_idx,
--    sessions_user_id_idx ฯลฯ)
alter table user_roles drop column user_id, drop column role_id;
alter table role_permissions drop column role_id;
alter table page_permissions drop column page_id;
alter table sessions drop column user_id;
alter table audit_log drop column actor_user_id;

-- 4) ตารางแม่: uuid เดิม → คอลัมน์ `uuid`, id_new → `id` (pk)
alter table users drop constraint users_pkey;
alter table users rename column id to uuid;
alter table users rename column id_new to id;
alter table users add primary key (id);
alter table users add constraint users_uuid_key unique (uuid);
alter table users alter column uuid set default uuid_v7();

alter table roles drop constraint roles_pkey;
alter table roles rename column id to uuid;
alter table roles rename column id_new to id;
alter table roles add primary key (id);
alter table roles add constraint roles_uuid_key unique (uuid);
alter table roles alter column uuid set default uuid_v7();

alter table pages drop constraint pages_pkey;
alter table pages rename column id to uuid;
alter table pages rename column id_new to id;
alter table pages add primary key (id);
alter table pages add constraint pages_uuid_key unique (uuid);
alter table pages alter column uuid set default uuid_v7();

-- 5) ตารางลูก: rename + not null + สร้าง pk/fk/index กลับตามเดิม
alter table user_roles rename column user_id_new to user_id;
alter table user_roles rename column role_id_new to role_id;
alter table user_roles
    alter column user_id set not null,
    alter column role_id set not null,
    add primary key (user_id, role_id),
    add foreign key (user_id) references users (id) on delete cascade,
    add foreign key (role_id) references roles (id) on delete restrict; -- ห้ามลบ role ที่ยังมีคนถือ
create index user_roles_role_id_idx on user_roles (role_id);

alter table role_permissions rename column role_id_new to role_id;
alter table role_permissions
    alter column role_id set not null,
    add primary key (role_id, permission),
    add foreign key (role_id) references roles (id) on delete cascade;

alter table page_permissions rename column page_id_new to page_id;
alter table page_permissions
    alter column page_id set not null,
    add primary key (page_id, permission),
    add foreign key (page_id) references pages (id) on delete cascade;

alter table sessions rename column user_id_new to user_id;
alter table sessions
    alter column user_id set not null,
    add foreign key (user_id) references users (id) on delete cascade;
create index sessions_user_id_idx on sessions (user_id);

alter table audit_log rename column actor_user_id_new to actor_user_id;
alter table audit_log
    add foreign key (actor_user_id) references users (id) on delete set null; -- เก็บ row ไว้แม้ user ถูกลบ
