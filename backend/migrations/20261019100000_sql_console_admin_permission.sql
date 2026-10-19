-- หน้า SQL (backend/src/sql_console/, permission sql.view): ให้ role ระบบ admin มีสิทธิ์นี้
--
-- ต้องทำด้วย migration เพราะกติกาห้ามให้สิทธิ์เกินตัวเอง (docs/auth-design.md ข้อ 10): ใส่ sql.view ให้ role ผ่านหน้าเว็บ
-- ได้เฉพาะคนที่ถือ sql.view อยู่แล้ว — ก่อน migration นี้ไม่มีใครถือ จึงไม่มีใครติ๊กให้ใครได้เลย
-- แตะเฉพาะแถวของ admin (role ระบบที่ seed ไว้) — role อื่นผู้ดูแลติ๊กเองในหน้าตั้งค่า role, ถอดออกจาก admin ภายหลังได้
-- ไม่แตะเพจ "SQL" ที่ผู้ดูแลสร้างเองผ่านหน้าเว็บ (ผูก sql.view กับเพจในหน้าตั้งค่าเพจเอง)

insert into role_permissions (role_id, permission)
select r.id, 'sql.view'
from roles r
where r.name = 'admin' and r.is_system
on conflict do nothing;
