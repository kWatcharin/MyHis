-- หน้า "ประวัติการใช้งาน" (GET /api/audit/activities) อ่าน audit_log ตามช่วงเวลาโดยไม่ระบุ action
-- (ทุก action ยกเว้นเข้า/ออกระบบ) index (action, created_at) เดิมช่วยไม่ได้ จึงเพิ่ม index ตามเวลาอย่างเดียว
-- ไม่เปลี่ยนคอลัมน์/ข้อมูล
create index audit_log_created_at_idx on audit_log (created_at);

comment on column audit_log.target_id is
    'uuid ของรายการที่ถูกกระทำ (ยกเว้น login ไม่สำเร็จ = ชื่อที่พิมพ์มา ซึ่ง API ไม่ส่งออก) — แถวผู้ป่วยมีแค่ uuid';
comment on column audit_log.metadata is
    'ข้อมูลเพิ่มเติม: {"userAgent"} ของการเข้า/ออกระบบ, {"label"} ชื่อ role/เพจ ณ ตอนนั้น, {"document","format"} ของการส่งออก — ห้ามใส่ความลับ/ข้อมูลผู้ป่วย';
