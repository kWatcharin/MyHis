//! token สุ่มสำหรับใช้เป็นความลับชั่วคราว (session cookie, ลิงก์ลงทะเบียน) + hash ก่อนเก็บลง DB
//!
//! อยู่ระดับ crate root (ไม่ใช่ใน auth/) เพราะทั้ง auth (session), users (ออกลิงก์ลงทะเบียน) และ registration
//! (ตรวจลิงก์) ต้องใช้ — โมดูลฟีเจอร์ไม่เรียกกันเอง จึงยกตรรกะร่วมขึ้นมาแบบเดียวกับ password.rs
//! DB เก็บแค่ SHA-256 ของ token เสมอ ไม่เก็บ token ดิบ (docs/auth-design.md ข้อ 4)

use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// สุ่ม token 256 บิต เข้ารหัส hex (64 ตัวอักษร) — เดาไม่ได้ในทางปฏิบัติ
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    to_hex(&bytes)
}

/// hash ค่า token ด้วย SHA-256 ก่อนเก็บ/ค้นใน DB (ไม่เก็บ token ดิบ)
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    to_hex(&hasher.finalize())
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_unique_and_hex() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_is_deterministic() {
        let token = generate_token();
        assert_eq!(hash_token(&token), hash_token(&token));
    }

    #[test]
    fn different_tokens_hash_differently() {
        assert_ne!(hash_token("a"), hash_token("b"));
    }
}
