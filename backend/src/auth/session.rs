//! สร้าง session token และจัดการ cookie (HttpOnly, Secure, SameSite=Lax, Path=/)
//! ไม่ตั้ง Max-Age เพื่อให้เป็น session cookie — อายุจริงคุมที่ idle/absolute_expires_at ใน DB
//! (docs/auth-design.md ข้อ 7)

use axum_extra::extract::cookie::{Cookie, SameSite};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};

pub const COOKIE_NAME: &str = "my_his_session";

/// สุ่ม session token 256 บิต เข้ารหัส hex ก่อนใส่ cookie
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    to_hex(&bytes)
}

/// hash ค่า token ด้วย SHA-256 ก่อนเก็บ/ค้นใน DB (ไม่เก็บ token ดิบ) — docs/auth-design.md ข้อ 4
pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    to_hex(&hasher.finalize())
}

pub fn build_cookie(token: String) -> Cookie<'static> {
    Cookie::build((COOKIE_NAME, token))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .path("/")
        .build()
}

/// cookie เปล่าไว้ให้ handler เรียก `jar.remove(session::removal_marker())` ตอน logout —
/// axum-extra จัดการ set ค่า expire ให้ถูกต้องเอง ไม่ต้องคำนวณ Max-Age เองที่นี่
pub fn removal_marker() -> Cookie<'static> {
    Cookie::from(COOKIE_NAME)
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
