-- หน้า "ลงทะเบียนผู้ป่วย" (CRUD ตาราง patient + ผูกโรคประจำตัว medical_condition) — API: /api/patients
-- ไม่เปลี่ยน schema ตาราง (unique/index ของ patient และ medical_condition มีครบแล้วใน
-- 20261004100200_patient_and_visits.sql) แค่เพิ่มสิทธิ์ใหม่ให้ role system + เพิ่มหน้าลงเมนู

-- สิทธิ์ใหม่ patient.read / patient.write ให้ role system admin และ nurse (ใช้งานได้ทันทีเหมือนเดิม)
-- role ที่ admin สร้างเองไม่ได้รับอัตโนมัติ — ตั้งผ่านหน้าการตั้งค่า role
insert into role_permissions (role_id, permission)
select r.id, p.permission
from roles r
cross join (values ('patient.read'), ('patient.write')) as p (permission)
where r.name in ('admin', 'nurse')
on conflict do nothing;

-- เมนูเดี่ยว (ไม่อยู่ในกลุ่ม) ใต้ Dashboard — ต้องมี patient.read ถึงจะเห็น
insert into pages (label, icon, route, sort_order, group_label) values
    ('ลงทะเบียนผู้ป่วย', 'user-plus', '/patients', 5, null);

insert into page_permissions (page_id, permission)
select id, 'patient.read' from pages where route = '/patients';
