-- เพดานความยาว label/icon ของเพจ (เมนู) — ค่าเหล่านี้ถูกส่งในเมนูของทุกคนและดึงจาก DB ซ้ำตามจำนวนสิทธิ์ของเพจ (list_pages)
-- ผู้ถือ page.create เคยทำให้ GET /menu ของทุกคนหนักได้ไม่จำกัด (security audit รอบ 2) API จำกัดแล้ว (users/dto.rs: label ≤ 100,
-- icon ≤ 64) constraint นี้เป็นด่านชั้นสอง
-- NOT VALID: บังคับกับแถวที่เขียนใหม่/แก้เท่านั้น ไม่ตรวจแถวเดิม (ไม่ให้ migration ล้มถ้ามีแถวเดิมยาวเกิน — เจ้าของแก้เมนูผ่านหน้าเว็บ)
-- เมื่อแน่ใจว่าไม่มีแถวเกินแล้ว ใช้ `alter table pages validate constraint <ชื่อ>` ได้
alter table pages
    add constraint pages_label_length_check check (char_length(label) <= 100) not valid,
    add constraint pages_icon_length_check check (icon is null or char_length(icon) <= 64) not valid;
