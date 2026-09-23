//! argon2id hash/verify — ต้องอยู่ใน tokio::task::spawn_blocking เสมอ
//! พารามิเตอร์ตาม OWASP: memory 19 MiB, iterations 2, parallelism 1 (docs/auth-design.md ข้อ 8)
