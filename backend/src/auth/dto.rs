//! รูปแบบ JSON เข้า/ออก — แยกจาก struct ของตาราง เพื่อไม่ให้ password_hash รั่วออก API
//! ห้าม derive Serialize ให้ struct ที่มี password_hash หรือ token (docs/rust-guidelines.md ข้อ 12)
