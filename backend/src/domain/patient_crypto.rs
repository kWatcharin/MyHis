//! คอลัมน์ของตาราง `patient` ที่เข้ารหัส (data dict: encrypt = yes) และ AAD ของแต่ละคอลัมน์ — ใช้ร่วมกันระหว่าง
//! `patient/` (เข้ารหัสก่อนเขียน ถอดตอนอ่าน) กับ `sql_console/` (ถอดผลของ SQL ที่ผู้ใช้รัน)
//!
//! AAD ผูก ciphertext กับคอลัมน์ ถ้าถูกสลับข้ามคอลัมน์ ถอดรหัสจะล้มเหลว (crypto.rs) ส่วน blind index ของ hn/nap_no
//! ใช้ AAD ตัวเดียวกับคอลัมน์ของมัน — **ห้ามเปลี่ยนค่า** ข้อมูลที่เข้ารหัสไว้แล้วทั้งหมดผูกกับค่าเหล่านี้

pub const AAD_HN: &str = "patient.hn";
pub const AAD_NAP_NO: &str = "patient.nap_no";
pub const AAD_FIRST_NAME: &str = "patient.first_name";
pub const AAD_LAST_NAME: &str = "patient.last_name";

/// ตารางที่มีคอลัมน์เข้ารหัส
pub const TABLE: &str = "patient";

/// (ชื่อคอลัมน์, AAD) ของทุกคอลัมน์ที่เข้ารหัส
pub const ENCRYPTED_COLUMNS: [(&str, &str); 4] = [
    ("hn", AAD_HN),
    ("nap_no", AAD_NAP_NO),
    ("first_name", AAD_FIRST_NAME),
    ("last_name", AAD_LAST_NAME),
];
