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
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

const MEMORY_KIB: u32 = 19 * 1024; // 19 MiB
const ITERATIONS: u32 = 2;
const PARALLELISM: u32 = 1;
/// คำนวณ argon2 พร้อมกันได้ไม่เกินนี้ทั้ง process (4 × 19 MiB) งานที่เกินรอคิว
const MAX_CONCURRENT: usize = 4;
/// ในช่องข้างบน login (ผู้ที่ยังไม่ยืนยันตัวตน) ใช้พร้อมกันได้ไม่เกินนี้ — เหลืออย่างน้อยหนึ่งช่องเสมอให้งานของผู้ที่ยืนยันตัวแล้ว
/// (เปลี่ยนรหัสผ่าน, สร้าง/รีเซ็ตบัญชี, ลงทะเบียนผ่านลิงก์) ต่อให้มีคนยิง login รัว ๆ
const MAX_CONCURRENT_LOGINS: usize = 3;
/// login ที่ถูกรับเข้าระบบ (กำลังคำนวณ + รอคิว) รวมกันไม่เกินนี้ — เกินแล้วปฏิเสธทันที (`PasswordError::Busy`) แทนที่จะรอจน
/// request หมดเวลา: ค่านี้กว้างกว่าที่คนใช้จริงมาก (ทีมพยาบาลหลักสิบคน) แต่จำกัดงานค้างที่ผู้ไม่ระบุตัวตนสร้างได้
const MAX_LOGIN_QUEUE: usize = 64;

/// ตัวจำกัดงาน argon2 — แยกเป็นชนิดเพื่อให้เทสสร้างตัวค่าเล็กได้ (ตัวจริงเป็น static ข้างล่าง ใช้ร่วมทั้ง process)
struct Limiter {
    /// ช่องคำนวณทั้งหมด (ทุกผู้เรียก)
    slots: Arc<Semaphore>,
    /// ช่องเฉพาะ login ที่กำลังคำนวณ/รอช่องคำนวณ
    logins: Arc<Semaphore>,
    /// จำนวน login ที่อยู่ในระบบ (กำลังคำนวณ + รอคิว)
    login_queue: Arc<Semaphore>,
}

/// ใบอนุญาตให้ login หนึ่งครั้งคำนวณ argon2 ได้ — ถือไว้จนคำนวณเสร็จ (ย้ายเข้าไปในงาน จึงไม่คืนช่องถ้า request ถูกตัดทิ้งกลางงาน)
pub struct LoginAdmission {
    _queue: OwnedSemaphorePermit,
    _login: OwnedSemaphorePermit,
}

impl Limiter {
    fn new(slots: usize, logins: usize, login_queue: usize) -> Self {
        Self {
            slots: Arc::new(Semaphore::new(slots)),
            logins: Arc::new(Semaphore::new(logins)),
            login_queue: Arc::new(Semaphore::new(login_queue)),
        }
    }

    /// รับ login เข้าระบบ: คิวเต็ม = `Busy` ทันที ไม่งั้นรอช่อง login (ยกเลิกได้ปลอดภัย — permit คืนเอง)
    async fn admit_login(&self) -> Result<LoginAdmission, PasswordError> {
        let queue = Arc::clone(&self.login_queue)
            .try_acquire_owned()
            .map_err(|_| PasswordError::Busy)?;
        let login = Arc::clone(&self.logins)
            .acquire_owned()
            .await
            .map_err(|_| PasswordError::Hash)?;
        Ok(LoginAdmission {
            _queue: queue,
            _login: login,
        })
    }

    /// รอช่องคำนวณแล้วรันงานใน spawn_blocking — permit (และใบอนุญาต login ถ้ามี) ย้ายเข้าไปในงาน คืนตอนคำนวณเสร็จจริง
    /// (request ที่ถูก timeout ตัดทิ้งระหว่างคำนวณไม่ทำให้มีงานเกินโควตา)
    async fn run<T: Send + 'static>(
        &self,
        admission: Option<LoginAdmission>,
        job: impl FnOnce() -> Result<T, PasswordError> + Send + 'static,
    ) -> Result<T, PasswordError> {
        let permit = Arc::clone(&self.slots)
            .acquire_owned()
            .await
            .map_err(|_| PasswordError::Hash)?;
        tokio::task::spawn_blocking(move || {
            let result = job();
            drop(permit);
            drop(admission);
            result
        })
        .await
        .map_err(|_| PasswordError::Hash)?
    }
}

static LIMITER: LazyLock<Limiter> =
    LazyLock::new(|| Limiter::new(MAX_CONCURRENT, MAX_CONCURRENT_LOGINS, MAX_LOGIN_QUEUE));

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("failed to hash password")]
    Hash,
    #[error("failed to parse stored password hash")]
    InvalidHash,
    /// login ค้างคิวเต็ม — ปฏิเสธทันที ไม่ได้ลองรหัสใด ๆ
    #[error("too many logins in progress")]
    Busy,
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

/// `hash` แบบ async (รอคิว + spawn_blocking)
pub async fn hash_async(password: String) -> Result<String, PasswordError> {
    LIMITER.run(None, move || hash(&password)).await
}

/// `verify` แบบ async (รอคิว + spawn_blocking) สำหรับผู้ที่ยืนยันตัวตนแล้ว (เปลี่ยนรหัสผ่าน) — login ใช้ `admit_login` + `verify_admitted`
pub async fn verify_async(password: String, stored_hash: String) -> Result<bool, PasswordError> {
    LIMITER
        .run(None, move || verify(&password, &stored_hash))
        .await
}

/// ขอเข้าคิว argon2 ของ login (ผู้ที่ยังไม่ยืนยันตัวตน) — `Err(Busy)` เมื่อมี login ค้างเต็มแล้ว
/// เรียก **ก่อน** จองครั้งตรวจรหัสในฐานข้อมูล เพื่อให้การปฏิเสธเพราะคิวเต็มไม่เผาโควตาครั้งผิดของใคร
pub async fn admit_login() -> Result<LoginAdmission, PasswordError> {
    LIMITER.admit_login().await
}

/// `verify` ของ login ที่ได้ใบอนุญาตแล้ว (ใบอนุญาตคืนตอนคำนวณเสร็จ)
pub async fn verify_admitted(
    admission: LoginAdmission,
    password: String,
    stored_hash: String,
) -> Result<bool, PasswordError> {
    LIMITER
        .run(Some(admission), move || verify(&password, &stored_hash))
        .await
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
        assert_eq!(LIMITER.slots.available_permits(), MAX_CONCURRENT);
    }

    /// login ที่ค้างคิวเต็มถูกปฏิเสธทันที (ไม่รอจน request หมดเวลา) และคิวกลับมารับได้เมื่อมีที่ว่าง
    #[tokio::test]
    async fn a_full_login_queue_rejects_immediately() {
        let limiter = Arc::new(Limiter::new(1, 1, 2));
        let first = limiter.admit_login().await.unwrap();
        // ตัวที่สองเข้าคิวแล้วรอช่อง login (ช่องเดียวถูกตัวแรกถืออยู่)
        let second = {
            let limiter = Arc::clone(&limiter);
            tokio::spawn(async move { limiter.admit_login().await })
        };
        while limiter.login_queue.available_permits() != 0 {
            tokio::task::yield_now().await;
        }
        assert!(matches!(
            limiter.admit_login().await,
            Err(PasswordError::Busy)
        ));
        drop(first);
        let second = second.await.unwrap().unwrap();
        drop(second);
        assert_eq!(limiter.login_queue.available_permits(), 2);
        assert_eq!(limiter.logins.available_permits(), 1);
    }

    /// login ไม่ยึดช่องคำนวณสุดท้าย: ระหว่างที่ login ถือช่องของตัวเองจนเต็ม งานของผู้ที่ยืนยันตัวแล้วยังเสร็จได้ และ login ตัวถัดไปต้องรอ
    #[tokio::test]
    async fn logins_leave_a_compute_slot_for_authenticated_work() {
        let limiter = Arc::new(Limiter::new(2, 1, 8));
        let admission = limiter.admit_login().await.unwrap();
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let running = {
            let limiter = Arc::clone(&limiter);
            tokio::spawn(async move {
                limiter
                    .run(Some(admission), move || {
                        wait.recv().unwrap();
                        Ok::<_, PasswordError>(1)
                    })
                    .await
            })
        };
        while limiter.slots.available_permits() != 1 {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            limiter
                .run(None, || Ok::<_, PasswordError>(7))
                .await
                .unwrap(),
            7
        );
        let waiting =
            tokio::time::timeout(std::time::Duration::from_millis(100), limiter.admit_login())
                .await;
        assert!(
            waiting.is_err(),
            "the second login must wait for the login slot"
        );

        release.send(()).unwrap();
        assert_eq!(running.await.unwrap().unwrap(), 1);
        let next = limiter.admit_login().await.unwrap();
        drop(next);
        assert_eq!(limiter.slots.available_permits(), 2);
        assert_eq!(limiter.login_queue.available_permits(), 8);
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
