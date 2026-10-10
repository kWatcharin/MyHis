-- กลุ่มเมนู "ลงทะเบียน": หน้า "ผู้ป่วย" (/patients เดิม) + หน้าใหม่ "ผู้ใช้งาน" (/users — จัดการบัญชีผู้ใช้
-- ผ่าน API /api/users ที่มีอยู่แล้วในโมดูล users/) ไม่เปลี่ยน schema ตาราง แค่จัดแถวใน pages
--
-- แตะเฉพาะแถวที่เกี่ยวข้อง: /patients เปลี่ยนป้าย+กลุ่ม (ไม่แตะ sort_order ที่ admin อาจปรับผ่านหน้าการตั้งค่า
-- เพจไว้แล้ว) การแก้เมนูอื่นที่ทำผ่านหน้าเว็บคงอยู่ใน DB ตามเดิม
update pages
set label = 'ผู้ป่วย', group_label = 'ลงทะเบียน', updated_at = now()
where route = '/patients';

-- sort 6 = ถัดจาก /patients (seed 5) และก่อนกลุ่ม Lab (8) — ลำดับกลุ่มในเมนูมาจากหน้าแรกของกลุ่ม
-- ต้องมี user.manage (สิทธิ์เดียวกับ API /users) — nurse เห็นกลุ่ม "ลงทะเบียน" แค่หน้า "ผู้ป่วย"
insert into pages (label, icon, route, sort_order, group_label) values
    ('ผู้ใช้งาน', 'user-cog', '/users', 6, 'ลงทะเบียน');

insert into page_permissions (page_id, permission)
select id, 'user.manage' from pages where route = '/users';
