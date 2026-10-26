-- นับครั้งที่ใส่รหัสผ่านปัจจุบันผิดตอนเปลี่ยนรหัส (POST /api/auth/password) ต่อ session
-- ครบ MAX_FAILED_ATTEMPTS แล้วลบ session นั้น (ไม่ล็อกบัญชี — ผู้ขโมย session จะได้ใช้ล็อกเจ้าของไม่ได้)
-- ตาราง sessions ไม่อยู่ใน READABLE_TABLES ของหน้า SQL คอลัมน์นี้จึงไม่ถูก grant
alter table sessions add column password_check_count integer not null default 0;

comment on column sessions.password_check_count is
    'ครั้งที่ตรวจรหัสผ่านปัจจุบันผิดตอนเปลี่ยนรหัสใน session นี้ — จองก่อนตรวจ, รีเซ็ตเป็น 0 เมื่อรหัสถูก, ครบเพดานแล้วลบ session';
