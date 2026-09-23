-- Page/menu access control ตาม docs/auth-design.md หัวข้อ 4, 11
-- ไม่มี is_system บน pages (ต่างจาก roles โดยตั้งใจ — ดูเหตุผลในเอกสาร)

create table pages (
    id          uuid primary key default gen_random_uuid(),
    label       text not null,
    icon        text,
    route       text not null unique,
    sort_order  integer not null default 0,
    created_at  timestamptz not null default now(),
    updated_at  timestamptz not null default now()
);

create table page_permissions (
    page_id     uuid not null references pages (id) on delete cascade,
    permission  text not null, -- ต้องตรงกับ Permission enum ฝั่ง Rust เสมอ (เหมือน role_permissions)
    primary key (page_id, permission)
);

-- Seed 3 หน้าเริ่มต้นตามที่ผู้ใช้ระบุ: Dashboard เป็น default (สาธารณะสำหรับผู้ login แล้วทุกคน —
-- ไม่มีแถวใน page_permissions) และหน้าตั้งค่า 2 อันที่ต้อง user.manage
insert into pages (label, icon, route, sort_order) values
    ('Dashboard', 'home', '/dashboard', 0);

insert into pages (label, icon, route, sort_order) values
    ('การตั้งค่าเพจ', 'layout-list', '/settings/pages', 10),
    ('การตั้งค่า role', 'shield-check', '/settings/roles', 20);

insert into page_permissions (page_id, permission)
select id, 'user.manage' from pages where route in ('/settings/pages', '/settings/roles');
