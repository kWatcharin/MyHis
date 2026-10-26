-- ลิงก์ลงทะเบียน/ตั้งรหัสใหม่ต้องผูกกับ ‘การกระทำที่ออกลิงก์’ (security audit รอบ 2):
--   create = สร้างบัญชี (สิทธิ์ user.create)    reset = รีเซ็ตรหัสผ่าน (สิทธิ์ user.update)
-- เดิมตรวจซ้ำตอนใช้ลิงก์ว่าผู้ออกยังถือ user.create *หรือ* user.update โดยไม่ดูว่าลิงก์ออกด้วยการกระทำไหน → ลิงก์รีเซ็ตที่ออกไว้แล้ว
-- ยังใช้ได้หลังผู้ออกถูกถอดเฉพาะ user.update (ผู้ออกที่ถือ user.create อย่างเดียวรีเซ็ตใครไม่ได้อยู่แล้ว)
-- ตอนใช้ลิงก์ (registration/repository.rs) reset ต้องการ user.update ส่วน create รับ user.create หรือ user.update
alter table registration_links
    add column purpose text not null default 'reset' check (purpose in ('create', 'reset'));

-- ลิงก์ที่ยังค้างอยู่ตอนนี้: เป้าหมายยังไม่ลงทะเบียน (registered_at ว่าง) → ถือเป็น create (รวมลิงก์ reset ที่ออกให้คนที่ยังไม่ลงทะเบียน —
-- ผ่อนกว่าจนหมดอายุ ≤ 3 วัน) ที่เหลือ → reset; แถวที่ใช้แล้ว/ถูกเพิกถอนใช้อีกไม่ได้ จึงปล่อยเป็นค่าปริยายชั่วคราว
update registration_links l
   set purpose = 'create'
  from users u
 where u.id = l.user_id
   and u.registered_at is null
   and l.used_at is null
   and l.revoked_at is null;

-- ต่อจากนี้ผู้เขียนต้องระบุเอง (ไม่มีค่าปริยายให้ลืมแล้วเงียบ)
alter table registration_links alter column purpose drop default;

comment on column registration_links.purpose is
    'การกระทำที่ออกลิงก์: create (สร้างบัญชี — user.create) หรือ reset (รีเซ็ตรหัส — user.update) ใช้ตัดสินสิทธิ์ของผู้ออกลิงก์ตอนใช้ลิงก์';
