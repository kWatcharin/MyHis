-- pages.ref_id: อ้างอิงเพจหลัก (main page) แทนข้อความ group_label — เมนู 2 ชั้น
--   เพจหลัก (ref_id null) = เมนูชั้นบน มี route = ลิงก์เดี่ยว (เช่น Dashboard) หรือไม่มี route = หัวข้อกลุ่ม
--   ที่กดพับ/กางดูเพจย่อย (เห็นในเมนูเมื่อมีเพจย่อยที่ผู้ใช้เห็นได้อย่างน้อยหนึ่งหน้า)
--   เพจย่อย (ref_id = id ของเพจหลักที่ไม่มี route) ต้องมี route เสมอ
-- DB กันแค่ที่ตรวจในแถวเดียวได้ + ลบเพจหลักที่ยังมีเพจย่อยไม่ได้ (restrict) ส่วนกฎ "ชั้นเดียว / เพจหลักที่มีเพจย่อย
-- ต้องไม่มี route" ตรวจใน service (backend/src/users/service/pages.rs)

alter table pages
    add column ref_id integer references pages (id) on delete restrict,
    alter column route drop not null,
    add constraint pages_ref_not_self check (ref_id <> id),
    add constraint pages_sub_page_has_route check (ref_id is null or route is not null);

comment on column pages.ref_id is 'pages.id ของเพจหลัก (null = เป็นเพจหลักเอง) — เมนู 2 ชั้น เพจหลักที่มีเพจย่อยไม่มี route';
comment on column pages.route is 'path ของหน้า เช่น /settings/roles (null = เพจหลักที่เป็นหัวข้อกลุ่มเมนู ไม่มีหน้าของตัวเอง)';

-- แปลงข้อมูลเดิม: กลุ่มเมนู (group_label) → แถวเพจหลักไม่มี route หนึ่งแถวต่อชื่อกลุ่ม แล้วผูกเพจในกลุ่มด้วย ref_id
-- ทำจากข้อมูลจริงใน DB ไม่ hardcode ชื่อกลุ่ม เพราะ admin ตั้งชื่อ/ย้ายกลุ่มเองผ่านหน้าการตั้งค่าเพจได้

-- Dashboard เป็นเพจหลัก (ลิงก์ชั้นบนสุด ไม่อยู่ในกลุ่มไหน) — แตะแถวนี้แถวเดียว
update pages
set group_label = null, updated_at = now()
where route = '/dashboard' and group_label is not null;

-- ชื่อกลุ่มตัดช่องว่างหัวท้าย และ '' = ไม่อยู่ในกลุ่ม (ฟอร์มเดิมส่ง '' ตอนไม่เลือกกลุ่ม) กันได้หัวข้อซ้ำ/หัวข้อว่าง
-- sort_order ของเพจหลัก = ค่าน้อยสุดในกลุ่ม ลำดับกลุ่มในเมนูจึงเหมือนเดิม (เดิมเรียงตามหน้าแรกของกลุ่ม)
insert into pages (label, route, sort_order)
select btrim(group_label), null, min(sort_order)
from pages
where nullif(btrim(group_label), '') is not null
group by btrim(group_label);

-- ก่อน migration นี้ทุกแถวมี route — แถว route null ทั้งหมดคือเพจหลักที่เพิ่งสร้างข้างบน จับคู่ด้วยชื่อได้ไม่กำกวม
update pages p
set ref_id = m.id, updated_at = now()
from pages m
where m.route is null
  and m.label = btrim(p.group_label)
  and p.route is not null;

alter table pages drop column group_label;
