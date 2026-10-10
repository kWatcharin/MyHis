//! จัดการ cookie ของ session (HttpOnly, Secure, SameSite=Lax, Path=/)
//! ไม่ตั้ง Max-Age เพื่อให้เป็น session cookie — อายุจริงคุมที่ idle/absolute_expires_at ใน DB
//! (docs/auth-design.md ข้อ 7)
//!
//! การสุ่ม/hash token ย้ายไปที่ `crate::token` แล้ว (users/registration ใช้ร่วมกับลิงก์ลงทะเบียน) — re-export ไว้
//! ที่นี่ให้จุดที่เรียกในโมดูล auth ใช้ชื่อเดิมต่อได้

use axum_extra::extract::cookie::{Cookie, SameSite};

pub use crate::token::{generate_token, hash_token};

pub const COOKIE_NAME: &str = "my_his_session";

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
