-- หน้า "ตั้งค่าโรค" (CRUD ตาราง disease): กันชื่อโรคซ้ำ + เพิ่มหน้าลงเมนูกลุ่ม "การตั้งค่า"
-- ต้องมี user.manage ถึงจะเห็น (เหมือนหน้าตั้งค่าเพจ/role/ปีงบประมาณ) — API คือ /api/diseases

-- ชื่อโรคซ้ำกันไม่ได้ (ไม่สนตัวพิมพ์เล็ก/ใหญ่ นับเฉพาะแถวที่ยังไม่ถูกลบ) — คำย่อซ้ำได้
create unique index disease_name_uq on disease (lower(name)) where deleted_at is null;
comment on column disease.name is 'ชื่อโรค (ห้ามซ้ำ ไม่สนตัวพิมพ์ นับเฉพาะแถวที่ยังไม่ถูกลบ)';

insert into pages (label, icon, route, sort_order, group_label) values
    ('ตั้งค่าโรค', 'stethoscope', '/settings/diseases', 40, 'การตั้งค่า');

insert into page_permissions (page_id, permission)
select id, 'user.manage' from pages where route = '/settings/diseases';
