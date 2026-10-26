-- ล้าง target_id ของ login ที่ล้มเหลวด้วยชื่อที่ไม่มีในระบบ (actor_user_id เป็น NULL) — ข้อยกเว้นครั้งเดียวของ "audit_log เขียนอย่างเดียว"
-- ที่เจ้าของโปรเจกต์อนุมัติ (2026-10-21) เพราะชื่อที่พิมพ์มาอาจเป็นรหัสผ่านที่พิมพ์ผิดช่อง: แถวก่อน 2026-10-12 เก็บเป็นชื่อดิบ
-- และแถวถึง 2026-10-21 เก็บเป็น SHA-256 ของชื่อ (เดารหัสสั้น ๆ กลับได้) ตั้งแต่ 20261021100100 แถวใหม่เป็น NULL อยู่แล้ว
-- ไม่แตะ action อื่น ไม่แตะแถวของบัญชีที่มีอยู่จริง (actor_user_id ไม่ NULL) ซึ่ง target_id คือ username ของบัญชีนั้น
update audit_log
set target_id = null
where action = 'login_failed'
  and actor_user_id is null
  and target_id is not null;
