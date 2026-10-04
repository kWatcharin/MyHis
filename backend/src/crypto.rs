//! เข้ารหัสคอลัมน์ที่ data dict ระบุ encrypt = yes (patient.hn, nap_no, first_name, last_name) ที่ชั้น app
//! ก่อนลง DB — AES-256-GCM (ใช้ crate `aes-gcm` ไม่เขียน algorithm เอง ตาม docs/rust-guidelines.md ข้อ 16)
//! และ blind index (HMAC-SHA256) ไว้ค้นหาแบบ "เท่ากับ" โดย DB ไม่เห็น plaintext
//!
//! ทำไมไม่ใช้ argon2 แบบเดียวกับ password: argon2 สุ่ม salt ทุกครั้ง จึงค้นด้วย `WHERE col = ?` ไม่ได้ และ
//! ถอดค่ากลับมาแสดงไม่ได้ — ข้อมูลที่ต้อง "แสดงกลับ" ใช้ encrypt, ที่ต้อง "ค้นหา" เพิ่ม blind index
//!
//! รูปแบบ blob ที่เก็บใน DB: `version (1 byte) || nonce (12) || ciphertext+tag` — byte แรกไว้หมุน key
//! ภายหลัง `aad` (additional authenticated data, เช่น "patient.hn") ผูก ciphertext กับคอลัมน์ ถ้าถูกสลับ
//! ข้ามคอลัมน์ decrypt จะล้มเหลว ฟังก์ชันในไฟล์นี้ไม่รู้จัก HTTP/DB
//!
//! **ห้าม log plaintext, ciphertext หรือ key** — `Crypto` เขียน Debug เองให้ redact ค่า key

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::Sha256;
use thiserror::Error;

const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 12;
const BLOB_VERSION: u8 = 1;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("encryption key must be 64 hex characters (32 bytes)")]
    InvalidKey,
    #[error("encryption key and blind index key must be different")]
    KeysMustDiffer,
    #[error("failed to encrypt value")]
    Encrypt,
    #[error("failed to decrypt value")]
    Decrypt,
}

#[derive(Clone)]
pub struct Crypto {
    encryption_key: [u8; KEY_LEN],
    blind_index_key: [u8; KEY_LEN],
}

impl std::fmt::Debug for Crypto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Crypto")
            .field("encryption_key", &"[redacted]")
            .field("blind_index_key", &"[redacted]")
            .finish()
    }
}

fn parse_hex_key(hex: &str) -> Result<[u8; KEY_LEN], CryptoError> {
    let hex = hex.trim();
    if hex.len() != KEY_LEN * 2 || !hex.is_ascii() {
        return Err(CryptoError::InvalidKey);
    }
    let mut out = [0u8; KEY_LEN];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte =
            u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).map_err(|_| CryptoError::InvalidKey)?;
    }
    Ok(out)
}

impl Crypto {
    /// รับ key เป็น hex 64 ตัวอักษร (32 byte) 2 ตัวที่ต้องต่างกัน — สร้างได้ด้วย `openssl rand -hex 32`
    pub fn from_hex_keys(
        encryption_key_hex: &str,
        blind_index_key_hex: &str,
    ) -> Result<Self, CryptoError> {
        let encryption_key = parse_hex_key(encryption_key_hex)?;
        let blind_index_key = parse_hex_key(blind_index_key_hex)?;
        if encryption_key == blind_index_key {
            return Err(CryptoError::KeysMustDiffer);
        }
        Ok(Crypto {
            encryption_key,
            blind_index_key,
        })
    }

    pub fn encrypt(&self, aad: &str, plaintext: &str) -> Result<Vec<u8>, CryptoError> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.encryption_key).map_err(|_| CryptoError::InvalidKey)?;

        let mut nonce_bytes = [0u8; NONCE_LEN];
        OsRng.fill_bytes(&mut nonce_bytes);

        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce_bytes),
                Payload {
                    msg: plaintext.as_bytes(),
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| CryptoError::Encrypt)?;

        let mut blob = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
        blob.push(BLOB_VERSION);
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);
        Ok(blob)
    }

    pub fn decrypt(&self, aad: &str, blob: &[u8]) -> Result<String, CryptoError> {
        if blob.len() <= 1 + NONCE_LEN || blob[0] != BLOB_VERSION {
            return Err(CryptoError::Decrypt);
        }
        let (nonce_bytes, ciphertext) = blob[1..].split_at(NONCE_LEN);

        let cipher =
            Aes256Gcm::new_from_slice(&self.encryption_key).map_err(|_| CryptoError::InvalidKey)?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(nonce_bytes),
                Payload {
                    msg: ciphertext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| CryptoError::Decrypt)?;

        String::from_utf8(plaintext).map_err(|_| CryptoError::Decrypt)
    }

    /// blind index สำหรับค้นหาแบบ `=` — deterministic (ค่าเดียวกัน → ได้ค่าเดิม) แต่ย้อนกลับไม่ได้
    /// และไม่ใช้ key เดียวกับการเข้ารหัส ตัดช่องว่างหัวท้ายก่อนคำนวณ ผูกกับ `aad` ของคอลัมน์
    /// เพื่อให้ index ของ hn กับ nap_no ที่ค่าเหมือนกันไม่ชนกัน
    pub fn blind_index(&self, aad: &str, value: &str) -> Vec<u8> {
        // new_from_slice รับ key ความยาวเท่าไหร่ก็ได้สำหรับ HMAC จึงไม่ล้มเหลวกับ 32 byte
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.blind_index_key)
            .expect("HMAC accepts keys of any length");
        mac.update(aad.as_bytes());
        mac.update(&[0u8]);
        mac.update(value.trim().as_bytes());
        mac.finalize().into_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY_A: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    const KEY_B: &str = "ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100";

    fn crypto() -> Crypto {
        Crypto::from_hex_keys(KEY_A, KEY_B).unwrap()
    }

    #[test]
    fn encrypt_then_decrypt_round_trips() {
        let c = crypto();
        let blob = c.encrypt("patient.hn", "66471").unwrap();
        assert_eq!(c.decrypt("patient.hn", &blob).unwrap(), "66471");
    }

    #[test]
    fn round_trips_thai_text() {
        let c = crypto();
        let blob = c.encrypt("patient.first_name", "สมชาย").unwrap();
        assert_eq!(c.decrypt("patient.first_name", &blob).unwrap(), "สมชาย");
    }

    #[test]
    fn same_plaintext_encrypts_differently_each_time() {
        let c = crypto();
        assert_ne!(
            c.encrypt("patient.hn", "66471").unwrap(),
            c.encrypt("patient.hn", "66471").unwrap()
        );
    }

    #[test]
    fn blob_does_not_contain_plaintext() {
        let blob = crypto().encrypt("patient.hn", "66471").unwrap();
        assert!(!blob.windows(5).any(|w| w == b"66471"));
    }

    #[test]
    fn tampered_blob_fails_to_decrypt() {
        let c = crypto();
        let mut blob = c.encrypt("patient.hn", "66471").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(c.decrypt("patient.hn", &blob).is_err());
    }

    #[test]
    fn different_aad_fails_to_decrypt() {
        let c = crypto();
        let blob = c.encrypt("patient.hn", "66471").unwrap();
        assert!(c.decrypt("patient.nap_no", &blob).is_err());
    }

    #[test]
    fn wrong_key_fails_to_decrypt() {
        let blob = crypto().encrypt("patient.hn", "66471").unwrap();
        let other = Crypto::from_hex_keys(KEY_B, KEY_A).unwrap();
        assert!(other.decrypt("patient.hn", &blob).is_err());
    }

    #[test]
    fn unknown_version_or_short_blob_is_rejected() {
        let c = crypto();
        let mut blob = c.encrypt("patient.hn", "66471").unwrap();
        blob[0] = 2;
        assert!(c.decrypt("patient.hn", &blob).is_err());
        assert!(c.decrypt("patient.hn", &[]).is_err());
        assert!(c.decrypt("patient.hn", &[BLOB_VERSION; 5]).is_err());
    }

    #[test]
    fn blind_index_is_deterministic_and_trims() {
        let c = crypto();
        assert_eq!(
            c.blind_index("patient.hn", "66471"),
            c.blind_index("patient.hn", "  66471 ")
        );
        assert_ne!(
            c.blind_index("patient.hn", "66471"),
            c.blind_index("patient.hn", "66472")
        );
    }

    #[test]
    fn blind_index_depends_on_column_and_key() {
        let c = crypto();
        assert_ne!(
            c.blind_index("patient.hn", "66471"),
            c.blind_index("patient.nap_no", "66471")
        );
        let other = Crypto::from_hex_keys(KEY_A, "11".repeat(32).as_str()).unwrap();
        assert_ne!(
            c.blind_index("patient.hn", "66471"),
            other.blind_index("patient.hn", "66471")
        );
    }

    #[test]
    fn invalid_keys_are_rejected() {
        assert!(matches!(
            Crypto::from_hex_keys("abcd", KEY_B),
            Err(CryptoError::InvalidKey)
        ));
        assert!(matches!(
            Crypto::from_hex_keys(&"zz".repeat(32), KEY_B),
            Err(CryptoError::InvalidKey)
        ));
        assert!(matches!(
            Crypto::from_hex_keys(KEY_A, KEY_A),
            Err(CryptoError::KeysMustDiffer)
        ));
    }

    #[test]
    fn debug_does_not_leak_keys() {
        let printed = format!("{:?}", crypto());
        assert!(!printed.contains("0001020304"));
        assert!(printed.contains("[redacted]"));
    }
}
