//! argon2id hash/verify — ต้องอยู่ใน tokio::task::spawn_blocking เสมอ (docs/rust-guidelines.md ข้อ 6)
//! พารามิเตอร์ตาม OWASP: memory 19 MiB, iterations 2, parallelism 1 (docs/auth-design.md ข้อ 8)
//! ตั้งค่าตรง ๆ แทนที่จะพึ่ง Default ของ crate เพื่อให้โค้ดกับเอกสารตรงกันเห็นชัด ไม่เขียน
//! algorithm การเข้ารหัส/hash เองตาม docs/rust-guidelines.md ข้อ 16 — ใช้ crate มาตรฐาน argon2
//!
//! อยู่ระดับ crate root (ไม่ใช่ auth/) เพราะทั้ง auth (login/change-password) และ users
//! (create-user/reset-password) ต้องใช้ เข้าเงื่อนไข "โมดูลฟีเจอร์ไม่เรียกกันเอง...ให้ย้ายตรรกะนั้น
//! ลง domain/ แทน" (docs/rust-guidelines.md) — เป็น infra ร่วมแบบเดียวกับ config/db/error/state
//! ไม่ใช่ตรรกะธุรกิจเฉพาะของ auth
//!
//! โค้ด async เรียก `hash_async` / `verify_async` — รอคิว (คำนวณพร้อมกันได้ไม่เกิน `MAX_CONCURRENT`) แล้วค่อย
//! `spawn_blocking` ถูกยิง login รัว ๆ จะรอคิวจนชน timeout ของ request แทนที่จะเปิด blocking thread ได้ถึง 512 ตัว
//! (ตัวละ 19 MiB) จนหน่วยความจำเครื่องหมด

use std::sync::{Arc, LazyLock};

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use thiserror::Error;
use tokio::sync::Semaphore;

const MEMORY_KIB: u32 = 19 * 1024; // 19 MiB
const ITERATIONS: u32 = 2;
const PARALLELISM: u32 = 1;
/// คำนวณ argon2 พร้อมกันได้ไม่เกินนี้ทั้ง process (4 × 19 MiB) งานที่เกินรอคิว
const MAX_CONCURRENT: usize = 4;

static SLOTS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(MAX_CONCURRENT)));

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("failed to hash password")]
    Hash,
    #[error("failed to parse stored password hash")]
    InvalidHash,
}

fn hasher() -> Argon2<'static> {
    let params = Params::new(MEMORY_KIB, ITERATIONS, PARALLELISM, None)
        .expect("static argon2 params are valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// hash รหัสผ่าน — เรียกจาก spawn_blocking เท่านั้น (บล็อก thread จริงระหว่างคำนวณ)
pub fn hash(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    hasher()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| PasswordError::Hash)
}

/// hash ของรหัสผ่านคงที่ที่ไม่มีความหมาย ใช้ตอน login ไม่พบ username เพื่อให้ยังคำนวณ argon2
/// จริงอยู่ดี (timing-safe — docs/auth-design.md ข้อ 6) เรียกจาก spawn_blocking เท่านั้น
pub fn dummy_hash() -> Result<String, PasswordError> {
    hash("this-is-not-a-real-password-used-only-for-timing-safety")
}

/// ตรวจรหัสผ่านกับ hash ที่เก็บไว้ — เรียกจาก spawn_blocking เท่านั้น
pub fn verify(password: &str, hash: &str) -> Result<bool, PasswordError> {
    let parsed = PasswordHash::new(hash).map_err(|_| PasswordError::InvalidHash)?;
    Ok(hasher()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// รอคิวแล้วรันงาน argon2 ใน spawn_blocking — permit ย้ายเข้าไปในงานเลย จึงคืนช่องตอนคำนวณเสร็จจริง
/// (request ที่ถูก timeout ตัดทิ้งระหว่างคำนวณไม่ทำให้มีงานเกินโควตา)
async fn run_limited<T: Send + 'static>(
    job: impl FnOnce() -> Result<T, PasswordError> + Send + 'static,
) -> Result<T, PasswordError> {
    let permit = Arc::clone(&SLOTS)
        .acquire_owned()
        .await
        .map_err(|_| PasswordError::Hash)?;
    tokio::task::spawn_blocking(move || {
        let result = job();
        drop(permit);
        result
    })
    .await
    .map_err(|_| PasswordError::Hash)?
}

/// `hash` แบบ async (รอคิว + spawn_blocking)
pub async fn hash_async(password: String) -> Result<String, PasswordError> {
    run_limited(move || hash(&password)).await
}

/// `verify` แบบ async (รอคิว + spawn_blocking)
pub async fn verify_async(password: String, stored_hash: String) -> Result<bool, PasswordError> {
    run_limited(move || verify(&password, &stored_hash)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// งานเกินจำนวนช่องต้องรอคิวแล้วได้ผลถูกครบทุกงาน
    #[tokio::test]
    async fn concurrent_verifies_queue_and_all_succeed() {
        let stored = hash_async("a-reasonably-long-password".to_string())
            .await
            .unwrap();
        let jobs: Vec<_> = (0..MAX_CONCURRENT * 2 + 1)
            .map(|i| {
                let stored = stored.clone();
                let attempt = if i % 2 == 0 {
                    "a-reasonably-long-password"
                } else {
                    "wrong-password-attempt"
                };
                tokio::spawn(verify_async(attempt.to_string(), stored))
            })
            .collect();
        for (i, job) in jobs.into_iter().enumerate() {
            assert_eq!(job.await.unwrap().unwrap(), i % 2 == 0);
        }
        assert_eq!(SLOTS.available_permits(), MAX_CONCURRENT);
    }

    #[test]
    fn hash_then_verify_succeeds() {
        let hashed = hash("a-reasonably-long-password").unwrap();
        assert!(verify("a-reasonably-long-password", &hashed).unwrap());
    }

    #[test]
    fn verify_rejects_wrong_password() {
        let hashed = hash("a-reasonably-long-password").unwrap();
        assert!(!verify("something-else-entirely", &hashed).unwrap());
    }

    #[test]
    fn verify_rejects_garbage_hash() {
        assert!(verify("anything", "not-a-real-hash").is_err());
    }
}
