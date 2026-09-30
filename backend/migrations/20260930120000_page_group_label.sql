-- เพิ่ม field จัดกลุ่มเมนูให้ pages — ชั้นเดียว (ไม่มีกลุ่มซ้อนกลุ่ม), free text ที่ admin
-- พิมพ์เอง ไม่ใช่ FK ไป table แยก เพราะเป็นแค่หัวข้อจัดกลุ่มในเมนู ไม่ใช่ page ที่มีเนื้อหาของตัวเอง
alter table pages add column group_label text;

-- ตั้งกลุ่มให้ 2 หน้าตั้งค่าที่ seed ไว้ตั้งแต่ 20260923144933_pages.sql
update pages set group_label = 'การตั้งค่า' where route in ('/settings/pages', '/settings/roles');
