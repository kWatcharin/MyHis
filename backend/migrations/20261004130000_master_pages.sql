-- หน้าตั้งค่า master ที่เหลือ 4 ตัว (CRUD): คำนำหน้า, สัญชาติ, กลุ่มคนไข้, แลป
-- กันชื่อซ้ำ (ไม่สนตัวพิมพ์เล็ก/ใหญ่ นับเฉพาะแถวที่ยังไม่ถูกลบ) + เพิ่มหน้าลงเมนูกลุ่ม "การตั้งค่า"
-- ต้องมี user.manage ถึงจะเห็น (เหมือนหน้าตั้งค่าเพจ/role/ปีงบประมาณ/โรค) — API: /api/prenames,
-- /api/nationalities, /api/patient-groups, /api/labs

create unique index prename_name_uq on prename (lower(name)) where deleted_at is null;
comment on column prename.name is 'คำนำหน้าเต็ม (ห้ามซ้ำ ไม่สนตัวพิมพ์ นับเฉพาะแถวที่ยังไม่ถูกลบ)';

create unique index nationality_national_name_uq on nationality (lower(national_name)) where deleted_at is null;
comment on column nationality.national_name is 'เชื้อชาติ/สัญชาติ (ห้ามซ้ำ ไม่สนตัวพิมพ์ นับเฉพาะแถวที่ยังไม่ถูกลบ)';

create unique index patient_group_name_uq on patient_group (lower(name)) where deleted_at is null;
comment on column patient_group.name is 'ชื่อกลุ่มคนไข้ (ห้ามซ้ำ ไม่สนตัวพิมพ์ นับเฉพาะแถวที่ยังไม่ถูกลบ)';

create unique index lab_name_uq on lab (lower(name)) where deleted_at is null;
comment on column lab.name is 'ชื่อเต็ม (ห้ามซ้ำ ไม่สนตัวพิมพ์ นับเฉพาะแถวที่ยังไม่ถูกลบ)';

insert into pages (label, icon, route, sort_order, group_label) values
    ('ตั้งค่าคำนำหน้า', 'user-round', '/settings/prenames', 50, 'การตั้งค่า'),
    ('ตั้งค่าสัญชาติ', 'flag', '/settings/nationalities', 60, 'การตั้งค่า'),
    ('ตั้งค่ากลุ่มคนไข้', 'users', '/settings/patient-groups', 70, 'การตั้งค่า'),
    ('ตั้งค่าแลป', 'flask-conical', '/settings/labs', 80, 'การตั้งค่า');

insert into page_permissions (page_id, permission)
select id, 'user.manage' from pages
where route in ('/settings/prenames', '/settings/nationalities', '/settings/patient-groups', '/settings/labs');
