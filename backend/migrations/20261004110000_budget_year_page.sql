-- เพิ่มหน้า "ตั้งค่าปีงบประมาณ" ลงเมนูกลุ่ม "การตั้งค่า" (แพทเทิร์นเดียวกับ 20260923144933_pages.sql)
-- ต้องมี user.manage ถึงจะเห็น (เหมือนหน้าตั้งค่าเพจ/role) — API คือ /api/budget-years
insert into pages (label, icon, route, sort_order, group_label) values
    ('ตั้งค่าปีงบประมาณ', 'calendar-range', '/settings/budget-years', 30, 'การตั้งค่า');

insert into page_permissions (page_id, permission)
select id, 'user.manage' from pages where route = '/settings/budget-years';
