//! ตรวจ token ของ Cloudflare Turnstile กับ siteverify ฝั่ง server (หน้า login — docs/auth-design.md ข้อ 6)
//! widget ฝั่งหน้าเว็บอย่างเดียวกันบอทไม่ได้ ต้องตรวจ token ที่นี่ทุกครั้ง token ใช้ได้ครั้งเดียว อายุ 300 วินาที
//!
//! ห้าม log token และ secret ไม่ว่ากรณีใด (log ได้แค่ error-codes ที่ Cloudflare ตอบ)

use std::net::IpAddr;
use std::time::Duration;

use serde::Deserialize;

use crate::config::TurnstileConfig;

pub const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";

/// สั้นกว่า timeout ของทั้ง request (10 วินาที app.rs) — Cloudflare ช้าแล้วได้ 503 ของเราเอง ไม่ใช่ 408 จาก layer
/// และยังเหลือเวลาให้ argon2 ที่อาจรอคิวอยู่
const VERIFY_TIMEOUT: Duration = Duration::from_secs(4);

/// secret ทดสอบของ Cloudflare (ผ่านทุก token / ไม่ผ่านทุก token / token ใช้แล้ว) ใช้ได้บน localhost
/// แต่ไม่ได้ป้องกันอะไรจริง — main.rs เตือนตอนเริ่มระบบถ้าใช้อยู่
const TEST_SECRET_KEYS: [&str; 3] = [
    "1x0000000000000000000000000000000AA",
    "2x0000000000000000000000000000000AA",
    "3x0000000000000000000000000000000AA",
];

#[derive(Debug, thiserror::Error)]
pub enum TurnstileError {
    /// Cloudflare ตอบว่า token ใช้ไม่ได้ (ปลอม/หมดอายุ/ใช้ซ้ำ) หรือมาจาก widget/โดเมนอื่น — เก็บแค่ error-codes
    #[error("turnstile rejected: {0:?}")]
    Rejected(Vec<String>),
    /// ติดต่อ siteverify ไม่ได้/timeout/ตอบผิดรูปแบบ — login ถูกปฏิเสธ (fail-closed)
    #[error("turnstile siteverify unavailable: {0}")]
    Unavailable(String),
}

pub struct Turnstile {
    client: reqwest::Client,
    site_key: String,
    secret_key: String,
    hostname: Option<String>,
    verify_url: String,
}

/// เขียน Debug เอง (AppState derive Debug) — secret ห้ามหลุดลง log
impl std::fmt::Debug for Turnstile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Turnstile")
            .field("site_key", &self.site_key)
            .field("secret_key", &"[redacted]")
            .field("hostname", &self.hostname)
            .field("verify_url", &self.verify_url)
            .finish()
    }
}

impl Turnstile {
    pub fn new(config: &TurnstileConfig) -> Result<Self, reqwest::Error> {
        Self::with_verify_url(config, SITEVERIFY_URL)
    }

    /// ชี้ไป siteverify ที่ระบุ — เทสใช้ server ปลอมในเครื่อง (ไม่ยิง Cloudflare จริงใน cargo test)
    pub fn with_verify_url(
        config: &TurnstileConfig,
        verify_url: &str,
    ) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder().timeout(VERIFY_TIMEOUT).build()?;
        Ok(Turnstile {
            client,
            site_key: config.site_key.clone(),
            secret_key: config.secret_key.clone(),
            hostname: config.hostname.clone(),
            verify_url: verify_url.to_string(),
        })
    }

    /// ค่าสาธารณะที่หน้า login ใช้ render widget
    pub fn site_key(&self) -> &str {
        &self.site_key
    }

    pub fn uses_test_key(&self) -> bool {
        TEST_SECRET_KEYS.contains(&self.secret_key.as_str())
    }

    /// ถาม Cloudflare ว่า token นี้ผ่านไหม — `expected_action` คือ action ที่หน้าเว็บตั้งให้ widget
    /// (token ของ widget อื่นใช้แทนกันไม่ได้) `remote_ip` = IP ของผู้ใช้ ถ้ารู้
    pub async fn verify(
        &self,
        token: &str,
        remote_ip: Option<IpAddr>,
        expected_action: &str,
    ) -> Result<(), TurnstileError> {
        let remote_ip = remote_ip.map(|ip| ip.to_string());
        let mut form = vec![("secret", self.secret_key.as_str()), ("response", token)];
        if let Some(ip) = &remote_ip {
            form.push(("remoteip", ip));
        }

        let response = self
            .client
            .post(&self.verify_url)
            .form(&form)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| TurnstileError::Unavailable(e.to_string()))?;
        let body: SiteverifyResponse = response
            .json()
            .await
            .map_err(|e| TurnstileError::Unavailable(e.to_string()))?;

        check(&body, expected_action, self.hostname.as_deref())
    }
}

/// response ของ siteverify เท่าที่ใช้
#[derive(Debug, Deserialize)]
struct SiteverifyResponse {
    success: bool,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
    hostname: Option<String>,
    action: Option<String>,
    #[serde(default)]
    metadata: SiteverifyMetadata,
}

#[derive(Debug, Default, Deserialize)]
struct SiteverifyMetadata {
    #[serde(default)]
    result_with_testing_key: bool,
}

fn check(
    body: &SiteverifyResponse,
    expected_action: &str,
    expected_hostname: Option<&str>,
) -> Result<(), TurnstileError> {
    if !body.success {
        return Err(TurnstileError::Rejected(body.error_codes.clone()));
    }
    // ผลจาก test key ไม่มี action และ hostname เป็น example.com เสมอ (ลองยิงจริงแล้ว 2026-10-14) — test secret ผ่าน
    // ทุก token อยู่แล้ว ตรวจสองช่องนี้ไปก็ไม่ได้อะไร จึงตรวจเฉพาะผลจาก key จริง
    if body.metadata.result_with_testing_key {
        return Ok(());
    }
    if body.action.as_deref() != Some(expected_action) {
        return Err(TurnstileError::Rejected(
            vec!["action-mismatch".to_string()],
        ));
    }
    if let Some(expected) = expected_hostname {
        if body.hostname.as_deref() != Some(expected) {
            return Err(TurnstileError::Rejected(vec![
                "hostname-mismatch".to_string()
            ]));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> SiteverifyResponse {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn real_key_result_must_match_action_and_hostname() {
        let ok = parse(
            r#"{"success":true,"error-codes":[],"hostname":"my-his.example","action":"login"}"#,
        );
        assert!(check(&ok, "login", Some("my-his.example")).is_ok());
        assert!(check(&ok, "login", None).is_ok());
        assert!(matches!(
            check(&ok, "register", None),
            Err(TurnstileError::Rejected(codes)) if codes == ["action-mismatch"]
        ));
        assert!(matches!(
            check(&ok, "login", Some("other.example")),
            Err(TurnstileError::Rejected(codes)) if codes == ["hostname-mismatch"]
        ));
        let no_action = parse(r#"{"success":true,"hostname":"my-his.example"}"#);
        assert!(check(&no_action, "login", None).is_err());
    }

    #[test]
    fn failed_result_keeps_error_codes() {
        let failed = parse(r#"{"success":false,"error-codes":["timeout-or-duplicate"]}"#);
        assert!(matches!(
            check(&failed, "login", None),
            Err(TurnstileError::Rejected(codes)) if codes == ["timeout-or-duplicate"]
        ));
    }

    #[test]
    fn testing_key_result_skips_action_and_hostname() {
        // ตัวอย่างจริงจาก siteverify ด้วย test secret
        let dummy = parse(
            r#"{"challenge_ts":"2026-10-14T06:10:14.705Z","error-codes":[],"hostname":"example.com","metadata":{"result_with_testing_key":true},"success":true}"#,
        );
        assert!(check(&dummy, "login", Some("my-his.example")).is_ok());
        let dummy_fail = parse(
            r#"{"error-codes":["invalid-input-response"],"success":false,"messages":[],"metadata":{"result_with_testing_key":true}}"#,
        );
        assert!(check(&dummy_fail, "login", None).is_err());
    }

    #[test]
    fn debug_output_hides_secret() {
        let turnstile = Turnstile::new(&TurnstileConfig {
            site_key: "public-site-key".to_string(),
            secret_key: "turnstile-secret-789".to_string(),
            hostname: None,
        })
        .unwrap();
        let printed = format!("{turnstile:?}");
        assert!(!printed.contains("turnstile-secret-789"), "{printed}");
        assert!(printed.contains("public-site-key"));
        assert!(!turnstile.uses_test_key());
    }
}
