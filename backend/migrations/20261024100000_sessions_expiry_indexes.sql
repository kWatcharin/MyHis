-- ตัวเก็บกวาด session (auth/repository.rs delete_expired_sessions) รันทุกครั้งที่ login สำเร็จ ด้วย
--   delete from sessions where idle_expires_at < $1 or absolute_expires_at < $1
-- ซึ่งเดิมไม่มี index ของสองคอลัมน์นี้ → สแกนทั้งตาราง (ของทุกผู้ใช้) ทุก login ต้นทุนโตตามจำนวน session ที่ยังไม่หมดอายุ
-- (security audit รอบ 2: 100,000 แถว = Seq Scan, หลัง index = BitmapOr) ไม่เปลี่ยนคอลัมน์/ข้อมูล
create index sessions_idle_expires_at_idx on sessions (idle_expires_at);
create index sessions_absolute_expires_at_idx on sessions (absolute_expires_at);
