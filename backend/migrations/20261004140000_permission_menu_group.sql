-- ย้ายหน้า "การตั้งค่าเพจ" และ "การตั้งค่า role" จากกลุ่มเมนู "การตั้งค่า" ไปอยู่ใต้กลุ่ม "สิทธิ์"
-- (กลุ่มเมนูเป็นแค่ข้อความ group_label บน pages — ดู 20260930120000_page_group_label.sql) route และ
-- permission ของหน้าไม่เปลี่ยน ลิงก์/บุ๊กมาร์กเดิม (/settings/pages, /settings/roles) ยังใช้ได้
-- กลุ่ม "การตั้งค่า" ยังมีหน้า master (ปีงบประมาณ, โรค, คำนำหน้า ฯลฯ) อยู่เหมือนเดิม
update pages
set group_label = 'สิทธิ์', updated_at = now()
where route in ('/settings/pages', '/settings/roles');
