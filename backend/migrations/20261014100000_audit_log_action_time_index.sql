-- หน้า "ประวัติการเข้าออก" (GET /api/audit/auth-events) อ่าน audit_log ตามชนิดเหตุการณ์ + ช่วงเวลา
-- index นี้ให้ query ไม่ต้องไล่ทั้งตารางเมื่อ audit_log โตขึ้น ไม่เปลี่ยนคอลัมน์/ข้อมูล
create index audit_log_action_created_at_idx on audit_log (action, created_at);

comment on column audit_log.ip is
    'IP ของผู้ใช้ (ค่าขวาสุดของ X-Forwarded-For ที่ proxy เติม) — มีตั้งแต่เก็บประวัติการเข้าออก (login/logout)';
comment on column audit_log.metadata is
    'ข้อมูลเพิ่มเติม เช่น {"userAgent": "..."} ของการเข้า/ออกระบบ — ห้ามใส่ความลับ/ข้อมูลผู้ป่วย';
