-- ตัวนับ login ผิดของชื่อที่ไม่มีในระบบ (ชื่อที่มีอยู่นับใน users.failed_attempts / locked_until เหมือนเดิม)
-- กติกาเดียวกันทั้งสองแบบ: ผิดติดกัน 5 ครั้งล็อก 60 วินาที — คนลองจึงแยกไม่ออกว่าชื่อไหนมีจริง (docs/auth-design.md ข้อ 9)
-- เก็บแค่ SHA-256 ของชื่อ (hex) ไม่เก็บชื่อดิบ เพราะชื่อที่พิมพ์มาอาจเป็นรหัสผ่านที่พิมพ์ผิดช่อง
create table login_throttle (
    username_hash    text primary key,
    failed_attempts  integer not null default 0 check (failed_attempts >= 0),
    locked_until     timestamptz,
    updated_at       timestamptz not null default now()
);

comment on table login_throttle is
    'ตัวนับ login ผิดของชื่อผู้ใช้ที่ไม่มีในระบบ (กันเดาว่าชื่อไหนมีจริง) — แถวที่ไม่ได้แตะเกิน 30 วันถูกลบตอนเขียน';
comment on column login_throttle.username_hash is 'SHA-256 (hex) ของชื่อผู้ใช้ที่พิมพ์มา (ตัวเล็ก ตัดช่องว่าง) — ไม่เก็บชื่อดิบ';
comment on column login_throttle.failed_attempts is 'จำนวนครั้งที่ผิดติดกัน (ครบ 5 = ล็อก, ล็อกหมดอายุแล้วนับใหม่)';
comment on column login_throttle.locked_until is 'ล็อกถึงเวลานี้ (60 วินาทีหลังผิดครั้งที่ 5) null = ไม่ล็อก';
comment on column login_throttle.updated_at is 'ผิดครั้งล่าสุด';

comment on column users.failed_attempts is 'จำนวนครั้งที่ login ผิดติดกัน (ครบ 5 = ล็อก, ล็อกหมดอายุแล้วนับใหม่, login สำเร็จ = 0)';
comment on column users.locked_until is 'ล็อก login ถึงเวลานี้ (60 วินาทีหลังผิดครั้งที่ 5) null = ไม่ล็อก — รีเซ็ตรหัสผ่านปลดล็อก';
