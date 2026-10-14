-- สิทธิ์แยกตามเพจ: ดู / เพิ่ม / แก้ไข / ลบ (+ ส่งออกเอกสารของนัดหมายแล็ป) แทน permission รวมเดิม 6 ตัว
-- (user.manage, patient.read, patient.write, shift.read, shift.write, audit.read) คีย์ใหม่ `<resource>.<action>`
-- ต้องตรงกับตาราง PERMISSIONS ใน backend/src/domain/mod.rs เป๊ะ (docs/auth-design.md ข้อ 10)
--
-- ไม่เปลี่ยน schema — แปลงข้อมูลอย่างเดียว ความสามารถเดิมของทุก role (รวม role ที่ admin สร้างเอง) คงอยู่ครบ
-- และเมนูที่แต่ละ role เห็นไม่เปลี่ยน:
--   patient.read  → ดูผู้ป่วย + ดู/ส่งออกนัดหมายแล็ป (เดิมนัดหมายแล็ปใช้ patient.read)
--   patient.write → เพิ่ม/แก้ไข/ลบ ผู้ป่วยและนัดหมายแล็ป
--   user.manage   → ทุก action ของ ผู้ใช้งาน, การตั้งค่าสิทธิ์, การตั้งค่าเพจ และหน้า master 6 หน้า
--   shift.read / shift.write / audit.read → shift.view / shift.update / audit.view (ยังไม่มีหน้า แค่เปลี่ยนชื่อ)

create temporary table permission_map (old_key text not null, new_key text not null) on commit drop;

insert into permission_map (old_key, new_key) values
    ('patient.read', 'patient.view'),
    ('patient.read', 'lab_visit.view'),
    ('patient.read', 'lab_visit.export'),
    ('patient.write', 'patient.create'),
    ('patient.write', 'patient.update'),
    ('patient.write', 'patient.delete'),
    ('patient.write', 'lab_visit.create'),
    ('patient.write', 'lab_visit.update'),
    ('patient.write', 'lab_visit.delete'),
    ('user.manage', 'user.view'),
    ('user.manage', 'user.create'),
    ('user.manage', 'user.update'),
    ('shift.read', 'shift.view'),
    ('shift.write', 'shift.update'),
    ('audit.read', 'audit.view');

-- user.manage → view/create/update/delete ของ role, เพจ และ master 6 หน้า
insert into permission_map (old_key, new_key)
select 'user.manage', resource || '.' || action
from unnest(array['role', 'page', 'budget_year', 'disease', 'prename', 'nationality', 'patient_group', 'lab'])
         as r(resource)
cross join unnest(array['view', 'create', 'update', 'delete']) as a(action);

insert into role_permissions (role_id, permission)
select rp.role_id, m.new_key
from role_permissions rp
join permission_map m on m.old_key = rp.permission
on conflict do nothing;

delete from role_permissions where permission in (select old_key from permission_map);

-- page_permissions (ใครเห็นเพจไหนในเมนู): คีย์เก่าของแต่ละเพจ → สิทธิ์ "ดู" ของเพจนั้น ตาม route — role ที่เคยมี
-- คีย์เก่าได้คีย์ใหม่ตัวนี้จาก role_permissions ข้างบนเสมอ เมนูจึงเหมือนเดิม แตะเฉพาะแถวที่เป็นคีย์เก่า
insert into page_permissions (page_id, permission)
select pp.page_id,
       case
           when pp.permission = 'patient.read' and p.route = '/lab/appointments' then 'lab_visit.view'
           when pp.permission = 'patient.read' then 'patient.view'
           when pp.permission = 'patient.write' and p.route = '/lab/appointments' then 'lab_visit.update'
           when pp.permission = 'patient.write' then 'patient.update'
           when pp.permission = 'user.manage' then
               case p.route
                   when '/users' then 'user.view'
                   when '/settings/roles' then 'role.view'
                   when '/settings/pages' then 'page.view'
                   when '/settings/budget-years' then 'budget_year.view'
                   when '/settings/diseases' then 'disease.view'
                   when '/settings/prenames' then 'prename.view'
                   when '/settings/nationalities' then 'nationality.view'
                   when '/settings/patient-groups' then 'patient_group.view'
                   when '/settings/labs' then 'lab.view'
                   -- เพจที่ admin สร้างเองแล้วผูก user.manage: ให้คนที่จัดการสิทธิ์ได้เห็นต่อ
                   else 'role.view'
               end
           when pp.permission = 'shift.read' then 'shift.view'
           when pp.permission = 'shift.write' then 'shift.update'
           when pp.permission = 'audit.read' then 'audit.view'
       end
from page_permissions pp
join pages p on p.id = pp.page_id
where pp.permission in (select old_key from permission_map)
on conflict do nothing;

delete from page_permissions where permission in (select old_key from permission_map);

-- กันแอปล่ม: backend parse permission ทุก request (auth) และทุกครั้งที่คำนวณเมนู ค่าที่ไม่รู้จัก = error ทันที
-- ถ้ายังเหลือคีย์นอกชุดใหม่ ให้ migration ทั้งก้อน rollback แทน
do $$
declare
    unknown text;
begin
    select string_agg(distinct permission, ', ')
    into unknown
    from (select permission from role_permissions
          union all
          select permission from page_permissions) used
    where permission not in (
        'patient.view', 'patient.create', 'patient.update', 'patient.delete',
        'lab_visit.view', 'lab_visit.create', 'lab_visit.update', 'lab_visit.delete', 'lab_visit.export',
        'user.view', 'user.create', 'user.update',
        'role.view', 'role.create', 'role.update', 'role.delete',
        'page.view', 'page.create', 'page.update', 'page.delete',
        'budget_year.view', 'budget_year.create', 'budget_year.update', 'budget_year.delete',
        'disease.view', 'disease.create', 'disease.update', 'disease.delete',
        'prename.view', 'prename.create', 'prename.update', 'prename.delete',
        'nationality.view', 'nationality.create', 'nationality.update', 'nationality.delete',
        'patient_group.view', 'patient_group.create', 'patient_group.update', 'patient_group.delete',
        'lab.view', 'lab.create', 'lab.update', 'lab.delete',
        'shift.view', 'shift.update',
        'audit.view'
    );
    if unknown is not null then
        raise exception 'permission keys outside the new set remain: %', unknown;
    end if;
end $$;
