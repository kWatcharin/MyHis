-- หน้า "นัดหมายแล็ป" ในกลุ่มเมนู "Lab" (CRUD lab_visit_group = head + lab_visit = รายชื่อคนไข้) —
-- API: /api/lab-visit-groups, /api/lab-visits ใช้สิทธิ์ patient.read / patient.write เดิม เพราะหน้านี้แสดง
-- ชื่อ/HN คนไข้ (ข้อมูลระบุตัวถอดรหัสที่ /api/patients ที่เดียว หน้าเว็บ join เอง) จึงไม่มี permission ใหม่

-- คนไข้คนเดียวอยู่ในนัดหมายเดียวกันซ้ำไม่ได้ (นับเฉพาะแถวที่ยังไม่ถูกลบ — ลบออกแล้วเพิ่มกลับได้)
create unique index lab_visit_group_patient_uq
    on lab_visit (lab_visit_group_id, patient_id) where deleted_at is null;
comment on column lab_visit.patient_id is 'patient.id (คนไข้ซ้ำในนัดหมายเดียวกันไม่ได้ นับเฉพาะแถวที่ยังไม่ถูกลบ)';

-- sort_order 8: ให้กลุ่ม "Lab" อยู่ระหว่าง "ลงทะเบียนผู้ป่วย" (5) กับกลุ่ม "สิทธิ์" (10) — ลำดับกลุ่มในเมนู
-- มาจากหน้าแรกของกลุ่มตาม sort_order
insert into pages (label, icon, route, sort_order, group_label) values
    ('นัดหมายแล็ป', 'calendar-clock', '/lab/appointments', 8, 'Lab');

insert into page_permissions (page_id, permission)
select id, 'patient.read' from pages where route = '/lab/appointments';
