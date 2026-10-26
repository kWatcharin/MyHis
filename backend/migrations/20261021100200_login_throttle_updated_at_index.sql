-- ตัวเก็บกวาดของ login_throttle (ลบแถวที่ไม่ได้แตะเกิน 30 วัน — auth/repository.rs) กรองด้วย updated_at ซึ่งเดิมไม่มี index
-- ทำให้ทุก login ที่ล้มเหลวด้วยชื่อที่ไม่มีในระบบสแกนทั้งตาราง เมื่อมีคนยิงชื่อสุ่มรัว ๆ ตารางโตและการสแกนช้าลงตามไปด้วย
-- index นี้ทำให้คำสั่งลบเป็น index range scan (ไม่เปลี่ยนคอลัมน์/ข้อมูล)
create index login_throttle_updated_at_idx on login_throttle (updated_at);
