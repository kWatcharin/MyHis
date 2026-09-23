//! กฎทั้งหมด: lockout, ตรวจรหัสผ่าน, ออก/เพิกถอน session
//! ห้าม import ชนิดข้อมูลของ Axum (StatusCode, Json, HeaderMap เป็นต้น)
//! ขั้นตอน login แบบ timing-safe: docs/auth-design.md ข้อ 6
