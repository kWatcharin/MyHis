# สรุป security audit (2026-10-21)

เอกสารนี้สรุปผลตรวจความปลอดภัยทั้งโปรเจกต์ สิ่งที่แก้แล้ว และสิ่งที่ยังเหลือให้ฝั่งเจ้าของโปรเจกต์ทำตอนติดตั้งจริง
รายละเอียดเชิงออกแบบของแต่ละข้อแก้อยู่ใน `docs/auth-design.md` และ `docs/infrastructure.md` (อ้างข้อไว้ในตารางข้างล่าง) ไฟล์นี้ไม่ซ้ำเนื้อหานั้น

## 1. ขอบเขตและวิธีตรวจ

| หัวข้อ | รายละเอียด |
|---|---|
| เครื่องมือ | [cloudflare/security-audit-skill](https://github.com/cloudflare/security-audit-skill) โปรไฟล์ `standard` (hunter 2 รอบ, critic, verifier อิสระต่อทุก candidate, ตรวจ record ซ้ำ) |
| ขอบเขต | `backend/`, `frontend/`, `ops/`, `docker-compose.yml`, Caddyfile, Dockerfile, `.env.example`; `docs/` อ่านเพื่อดูเจตนาออกแบบเท่านั้น |
| ซอร์สที่ตรวจ | commit `02d6c71` พร้อมงานที่ยังไม่ commit ณ วันนั้น (22 path) |
| วิธีรันโค้ด | sandbox เท่านั้น (Docker ไม่มี internet, Postgres ชั่วคราวข้อมูลหลอก) ไม่แตะ `my_his`, `.env`, container ที่รันอยู่ |
| ข้อจำกัด | audit ไม่ใช่การรับรองว่า ‘ส่วนที่ไม่พบคือปลอดภัย’ — ผลครอบคลุมเฉพาะ 52 หน่วยโค้ดที่ตรวจ |

ผลดิบ (REPORT, FINDINGS-DETAIL, NEEDS-VALIDATION, FIX-STATUS) อยู่นอก repo ที่
`C:\Users\PHP012025\security-audit-skill\my-his\run-1\` (เครื่องอื่นอาจไม่มี)
ผลรอบที่สอง (ตรวจซ้ำเฉพาะส่วนที่แก้) อยู่ที่ `…\run-2\` และสรุปในข้อ 8

## 2. ภาพรวมผล

**ไม่พบ critical หรือ high** พบ confirmed 11 ข้อ (medium 2, low 9) และ needs_validation 9 ข้อ
ทั้งหมดแก้ในโค้ด/config แล้ว (ข้อ 1 ของ needs_validation ตัดออกตามเจ้าของ; งานฝั่งเจ้าของตอนติดตั้งอยู่ในข้อ 5)

กลุ่มปัญหา:

1. credential/ลิงก์ระหว่างเหตุการณ์พร้อมกัน (race) และการตรวจอำนาจเฉพาะตอนออกลิงก์
2. ขอบเขตสิทธิ์ของข้อมูลผู้ป่วยและหน้า SQL (ได้สัญญาณว่า HN/โรคมีอยู่โดยไม่มี `patient.view`)
3. ความพร้อมใช้งาน: งาน CPU ไม่มีเพดาน, คิว argon2 ร่วมก่อน login, ตารางโตไม่จำกัด
4. ขอบ deploy: ingress, timeout, header ความปลอดภัย

### สิ่งที่ตรวจแล้วดี (รักษาไว้)

- ทุก query นอกหน้า SQL เป็น `sqlx::query!` ผูกพารามิเตอร์
- หน้า SQL ใช้ role DB แยก (ไม่ใช่ superuser), grant ระดับคอลัมน์, transaction อ่านอย่างเดียว, timeout, เพดาน connection
- session เป็น token สุ่ม 256 บิต เก็บเฉพาะ SHA-256, คุกกี้ HttpOnly/Secure/SameSite=Lax, ตรวจ `is_active` + สิทธิ์ใหม่ทุกคำขอ
- POST ที่มี body ผ่าน `Json` extractor เท่านั้น (form/text ได้ 415), DTO ใช้ `deny_unknown_fields`
- service ตรวจสิทธิ์ก่อนค้นข้อมูล (403 ก่อน 404), ชุดสิทธิ์ปิดตายตัว
- ข้อมูลผู้ป่วย AES-256-GCM nonce สุ่ม + AAD ต่อคอลัมน์, key ไม่ออก log/error, audit เก็บเฉพาะชื่อฟิลด์
- หน้าเว็บไม่มี HTML sink ที่รับค่าผู้ใช้, CSP ไม่มี `unsafe-inline` สำหรับสคริปต์, token ลงทะเบียนอยู่ใน URL fragment

## 3. confirmed 11 ข้อ — แก้ครบ (มีเทสกันกลับมาทุกข้อ)

| ระดับ | ปัญหา | การแก้ | เทส |
|---|---|---|---|
| medium | ลิงก์ลงทะเบียน/รีเซ็ตที่ค้างอยู่ยังใช้ได้หลังเป้าหมายถูกเพิ่มสิทธิ์เกินผู้ออกลิงก์ หรือผู้ออกถูกปิดบัญชี | ตรวจอำนาจผู้ออกซ้ำตอน lookup และ complete | `tests/registration.rs` |
| medium | ถอดรหัสแบบฝังในผลหน้า SQL กิน CPU ไม่จำกัดบน async runtime | งบ 200,000 ครั้งถอด/คำขอ, ข้าม hex run > 4,096, ย้ายไป `spawn_blocking` | `embedded.rs`, `tests/sql_console.rs` |
| low | change-password ที่ค้างอยู่เขียนทับผลรีเซ็ตของ admin | compare-and-swap บน `password_hash` เดิม | `tests/credential_races.rs` |
| low | login ที่ค้างอยู่รอดจากการรีเซ็ต (session ไม่ผูก credential) | `INSERT … SELECT … FOR SHARE` ผูกกับ hash ที่ตรวจแล้ว | `tests/credential_races.rs` |
| low | ตรวจรหัสปัจจุบันตอนเปลี่ยนรหัสไม่จำกัดครั้ง | `sessions.password_check_count` จองก่อน verify เกิน 5 ลบ session | `tests/credential_races.rs` |
| low | `audit_log.target_id` เก็บ hash ของชื่อที่พิมพ์ตอน login ผิด (อาจเป็นรหัสผ่านพิมพ์ผิดช่อง) | ชื่อที่ไม่มีในระบบ → `target_id` เป็น NULL + migration ล้างแถวเก่า | `tests/login_throttle.rs` |
| low | `:hn()` / `:nap_no()` ในหน้า SQL เป็น oracle โดยไม่มี `patient.view` | ต้องมี `patient.view` | `tests/sql_console.rs` |
| low | create/replace ผู้ป่วยเป็น oracle ของ HN ที่มีอยู่แล้ว | ต้องมี `patient.view` ด้วย | `tests/patients.rs` |
| low | PUT ผู้ป่วยเป็น oracle ของโรค inactive ที่ผูกอยู่ | เช่นเดียวกัน | `tests/patients.rs` |
| low | dedupe id แบบ O(n²) ก่อนตรวจเพดานจำนวน | cap ก่อนวน, dedupe ด้วย `HashSet`, body ≤ 256 KiB | `tests/input_limits.rs`, `tests/patients.rs` |
| low | ถอดค่าวันที่/เวลานอกช่วงของหน้า SQL แล้ว panic ทำให้ไม่มี audit | checked arithmetic, panic กลายเป็น error ที่ยังบันทึก audit | `value.rs`, `tests/sql_console.rs` |

กฎที่ตามมาเป็นข้อบังคับกับโค้ดใหม่: endpoint เขียนที่ตอบต่างกันตามข้อมูลที่ยังไม่ได้ดูสิทธิ์ ต้องมี `patient.view` ด้วย
(เหตุผล: ผู้ที่เขียนได้แต่ดูไม่ได้ต้องไม่รู้ว่ามีข้อมูลอยู่หรือไม่)

## 4. needs_validation 9 ข้อ

| # | ปัญหา | การแก้ | อ้างอิงเอกสาร |
|---|---|---|---|
| 1 | checkout อยู่ใน webroot ของ XAMPP | **ตัดออก** — เครื่องนี้เป็นเครื่อง dev โค้ดจะติดตั้งที่อื่น | — |
| 2 | login throttle ตรวจล็อกแล้วค่อยนับ (ไม่ atomic): login ผิดพร้อมกันผ่านด่าน verify ได้เกิน 5 | จองตัวนับแบบ atomic **ก่อน** verify (SQL คำสั่งเดียว) ผู้จองเลข 6 ตั้งล็อกเอง | auth-design ข้อ 9 |
| 3 | คิว argon2 ร่วมระหว่าง login ที่ยังไม่พิสูจน์ตัวกับงานที่ยืนยันตัวแล้ว | login ใช้ ≤ 3 จาก 4 ช่อง, ค้างรวม ≤ 64 เกินแล้ว `503 server busy` | auth-design ข้อ 5, 8 |
| 4 | แถว `login_failed` โตตามชื่อสุ่มที่ผู้โจมตีส่งได้ไม่จำกัด | index `login_throttle(updated_at)`; audit ของชื่อที่ไม่มีในระบบจำกัด **60 แถว/นาที** ทั้งระบบ (ชื่อที่มีจริงบันทึกครบ); **เพดาน login ไม่สำเร็จ 20 ครั้ง/10 นาที/IP** (รอบ 3) | auth-design ตาราง `audit_log`, ข้อ 9.2 |
| 5 | ingress ไม่มีขีดจำกัดระดับ connection | Caddy ทั้งสองชั้นตั้ง `timeouts` + `max_header_size`; pin `caddy:2.11.7`; `restart: unless-stopped`; backend ห่อ `Listener` (`conn_guard.rs`, รอบ 3): ≤ 512 connection, ตัด connection ที่ไม่คืบหน้า 5 นาที (> keepalive 2 นาทีของ Caddy) | infrastructure ข้อ 1, 6 |
| 6 | `ops/Caddyfile` ข้ามไป backend ตรง (ไม่ผ่าน header ความปลอดภัยของ frontend) | ingress → `frontend:8080` → backend; `DOMAIN` ผ่าน `environment:` ไม่ใช้ `env_file`; IP จริงส่งผ่าน `X-Real-IP` → `X-Forwarded-For` | infrastructure ข้อ 2, 6 |
| 7 | `pages.route` เป็นค่าอะไรก็ได้ (`//host`, `javascript:`) แล้วเรนเดอร์เป็นลิงก์ | ต้องเป็น path ในแอป ตรวจทั้ง backend และฟอร์ม เมนูที่ผิดกฎไม่แสดงเป็นลิงก์ | auth-design ข้อ 11 |
| 8 | id จาก URL หน้าเว็บต่อเข้า path API ตรง ๆ (`..%2Fauth%2Flogout-all%3F` หลุดไป endpoint อื่นด้วยคุกกี้ผู้ใช้) | `seg()` ห่อทุก id ที่ต่อเข้า path API และปฏิเสธ `.`/`..`/ว่าง; `isUuid` กั้นหน้าที่รับ id จาก URL | `lib/api.ts`, `lib/uuid.ts` |
| 9 | POST ที่ไม่มี body พึ่ง SameSite=Lax อย่างเดียว | middleware กลางใน `app.rs`: `Sec-Fetch-Site` → ไม่มีก็เทียบ `Origin` กับ `Host` → ไม่มีทั้งคู่ผ่าน (curl/เทส) | auth-design ข้อ 7 |

บทเรียนจากข้อ 8: `encodeURIComponent('..')` คืน `..` เหมือนเดิม และเบราว์เซอร์ถือ `%2e` เป็น dot segment —
การ encode อย่างเดียวไม่พอ ต้องปฏิเสธค่าเหล่านี้เอง

## 5. ที่ยังเหลือ

ปิดแล้วในรอบ 3 (2026-10-21): เพดาน login ไม่สำเร็จต่อ IP ในแอป (`auth/ip_budget.rs`, auth-design ข้อ 9.2),
ด่านระดับ connection ของ backend โดยห่อ `Listener` ไม่เพิ่ม dependency (`conn_guard.rs`), tag `caddy:2.11.7` / `-alpine` ดึงจริงได้ (`docker pull` สำเร็จ 2026-10-24),
คำเตือนใน log เมื่อ Turnstile ยังใช้ test key (มีอยู่แล้วใน `main.rs`)

ปิดแล้วในรอบ 4 (2026-10-24, หลัง audit รอบสอง — ดูข้อ 8): ข้อพบทั้ง 6 + 6 ของ audit รอบสองแก้ในโค้ด/config แล้ว ยกเว้นส่วนที่ต้องยืนยันบนเครื่องจริง (ด้านล่าง)

งานฝั่งเจ้าของตอนติดตั้ง:
1. **Turnstile key จริง** (+ `TURNSTILE_HOSTNAME`) และทดสอบกับโดเมน/TLS จริง (checklist ใน `docs/infrastructure.md`)
2. **ห้ามเปิดพอร์ต backend/frontend ออกนอก network ภายใน** — เพดานต่อ IP เชื่อ `X-Forwarded-For` ค่าขวาสุดที่ Caddy เติม
3. **Cloudflare (proxy ส้ม) + IP จริง** (audit รอบสอง N1/N2 — แก้ใน `ops/Caddyfile` แล้ว ยังต้องยืนยันบนเครื่องจริง):
   - firewall เปิด 80/443 เฉพาะช่วง IP ของ Cloudflare (หรือ Authenticated Origin Pulls), SSL/TLS = Full (strict)
   - ล็อกอิน 1 ครั้งผ่านโดเมนจริงทั้ง IPv4 และ IPv6 แล้วดูหน้าประวัติ login ว่าได้ IP ของ client จริง ไม่ใช่ IP ของ Cloudflare/container
     (backend เตือนใน log หนึ่งครั้งถ้าเห็นที่อยู่ที่ไม่ใช่ public) — รายละเอียดในหัวข้อ Cloudflare ของ `docs/infrastructure.md`
   - ตรวจรายการช่วง IP ของ Cloudflare ใน `ops/Caddyfile` ปีละครั้ง (ดึงเมื่อ 2026-10-23 จาก `cloudflare.com/ips-v4`, `/ips-v6`)
4. (ทางเลือก) Configuration Rule ‘request body buffering = Full’ ของ Cloudflare — ลดผลของ request ที่ body ค้างอีกชั้น (N4; ยังไม่ได้ทดสอบ)

ทางเลือกเสริม (ไม่บังคับ):
5. `hyper-util` เป็น dependency ตรงเพื่อให้ backend มี `header_read_timeout` จริง (ตอนนี้ `conn_guard` แยก ‘ว่างระหว่าง request’ กับ
   ‘ค้างกลาง header’ ไม่ได้ — ชั้นหน้า Caddy `read_header 10s` ตัดให้) กฎโปรเจกต์: ถามก่อนเพิ่ม dependency
6. rate limit ของ request ทั่วไปต่อ IP ระดับเครื่อง (plugin rate-limit ของ Caddy หรือ fail2ban — ถ้าใช้ Cloudflare ตั้งที่ Cloudflare ได้ ไม่ต้อง build Caddy เอง)

## 6. ตอน deploy ครั้งถัดไป

- migration ใหม่ 7 ไฟล์ รันเองตอน backend เริ่ม (ผ่าน dry-run บน `my_his_migtest` แล้ว ยังไม่เคยรันบน `my_his`):
  `20261021100000_session_password_checks`, `20261021100100_audit_target_id_null_for_unknown_names`,
  `20261021100200_login_throttle_updated_at_index`, `20261021100300_clear_unknown_name_login_failed_target_ids`,
  `20261024100000_sessions_expiry_indexes` (index 2 ตัวบน `sessions`), `20261024100100_registration_link_purpose` (คอลัมน์ `registration_links.purpose` +
  backfill ลิงก์ที่ค้างอยู่), `20261024100200_pages_text_limits` (CHECK label/icon แบบ `NOT VALID` — ไม่ตรวจแถวเดิม)
- migration `…100300` ตั้ง `audit_log.target_id = NULL` ของ `login_failed` ที่ไม่มีผู้กระทำ — ตรวจบน dev ก่อนแล้ว **23 แถว**
  (เจ้าของอนุมัติ 2026-10-21; เป็นข้อยกเว้นครั้งเดียวของกฎ ‘audit_log เขียนอย่างเดียว’) ยังไม่มีเทสที่รันกับแถวรูปแบบเก่าจริง (เทสใช้ตารางว่าง)
- `.sqlx/` สร้างใหม่แล้ว (174 ไฟล์) และ build แบบ offline ผ่าน
- **`docker compose up -d` ต้องสร้าง container ใหม่** (ไม่ใช่แค่ restart): `restart: unless-stopped`, network 3 วง (`edge`/`app`/`data`), `db` ได้ environment
  เฉพาะ 3 ตัว, limit/healthcheck/log rotation ใหม่ล้วนมีผลตอนสร้างใหม่ — network ใหม่ทำให้ network `my-his_default` เดิมไม่ถูกใช้แล้ว
  (dev: `docker-compose.override.yml` ที่ไม่ขึ้น git ต่อ `db` เข้า network `dev` เพิ่มเพื่อ publish `127.0.0.1:55432`; เช็ก backend ใน container ใช้ `--network my-his_dev`)
- ตัวแปร `DOMAIN` ใน `.env` ต้องมี (ค่าปริยาย `localhost` ใช้ได้เฉพาะ dev); `RUST_LOG` ไม่ตั้งก็ได้ (ปริยาย `info`)
- ขึ้นรอบนี้ `cargo build` ใน Dockerfile ใช้ `--locked` — `Cargo.lock` ต้องตรงกับ `Cargo.toml` เสมอ

## 7. การตรวจสอบหลังแก้

รอบ 1–3 (2026-10-21):
- backend: `cargo clippy --all-targets -- -D warnings`, `cargo fmt` สะอาด; `cargo test` ทุก binary ผ่าน (รันเป็นชุดเล็ก)
- frontend: `bun run build` ผ่าน, `bun run lint` 14 warnings เท่า baseline
- Caddy: `caddy adapt --validate` และ `caddy fmt --diff` ผ่านทั้งสองไฟล์; ทดลอง Caddy 2 ชั้นจริงแล้ว backend เห็น `X-Forwarded-For` เป็น IP จริงตัวเดียว
  (ปลอม `X-Forwarded-For`/`X-Real-IP` ไม่ได้), `Host` รอดตลอดทาง, CSP และ `Cache-Control: no-store` ออกครบ

รอบ 4 (2026-10-24):
- backend: `cargo fmt`, `clippy --all-targets -D warnings` สะอาด; เทสทุก binary (22 ไฟล์ + unit 122 ข้อ) ผ่านเป็นชุดเล็กบน `my_his_migtest`; migration ใหม่ 3 ไฟล์ dry-run ผ่าน;
  build offline (`--network none`) ผ่านหลังสร้าง `.sqlx` ใหม่; เทสใหม่: `login_success_limit`, `sql_console_limits`, `body_timeout`, ส่วนเพิ่มใน `login_ip_limit` /
  `registration` / `lab_visits` / `pages` / unit ของ `ip_budget`, `user_budget`, `domain::username`, `value`
- frontend: `bun run build` ผ่าน, `bun run lint` 14 warnings เท่า baseline
- Caddy 2.11.7 จริง: `caddy validate` + `caddy fmt --diff` ผ่านทั้งสองไฟล์; ทดลอง `{client_ip}` ด้วย `trusted_proxies` = subnet ของ docker network —
  peer ที่เชื่อส่ง `CF-Connecting-IP` (IPv4 และ IPv6) ได้ค่านั้น, ไม่ส่ง header ได้ IP ของ peer, `X-Real-IP` ปลอมถูกทับ, peer ที่ไม่เชื่อส่ง `CF-Connecting-IP` ปลอมถูกเมิน
- compose ใหม่ (โปรเจกต์ทดลองค่า dummy ชื่อ `satrial` ลบทิ้งแล้ว): backend `healthy`, `caddy` ไม่เห็น `db`/`backend` (resolve ไม่ได้), `frontend`→`backend` ใช้ได้,
  backend ออกเน็ตได้ / `db` ออกเน็ตไม่ได้ (`data` internal), `db` environment มีแค่ `POSTGRES_*` + ตัวแปรของ image, login ผ่าน Caddy จริงเห็นคำเตือน ‘non-public client address’
  ใน log ที่ระดับ `info` (ปริยาย)
- ยังไม่ได้ทดสอบ: โดเมน/TLS จริง, Cloudflare จริง (IPv4/IPv6), ผลของ `Configuration Rule` ฝั่ง Cloudflare, migration `…100300` กับแถว audit เก่า, memory/โหลดจริงของ host

## 8. audit รอบที่สอง (scoped, 2026-10-23) และการแก้ (2026-10-24)

ตรวจซ้ำ **เฉพาะโค้ดที่เปลี่ยนหลังรอบแรก** (ผลของรอบ 1–3, per-IP budget, conn guard, migration 4 ไฟล์) — เป็น **partial coverage**
ส่วนที่ไม่เปลี่ยนไม่ได้ถูกตรวจซ้ำ ไม่ใช่ ‘ตรวจแล้วปลอดภัย’ ใช้เครื่องมือเดิม (profile `standard`, ใช้ agent 40 ครั้ง; เพดานที่ผู้ประสานงานเสนอเอง 35 ขยายเป็น 45 ระหว่างรัน)
ผลดิบอยู่ที่ `C:\Users\PHP012025\security-audit-skill\my-his\run-2\` (`REPORT.md`, `FINDINGS-DETAIL.md`, `NEEDS-VALIDATION.md`)

**ผลตรวจซ้ำ run-1 (20 records):** 15 ข้อยืนยันว่า fix ยังตั้งอยู่, 4 ข้อเหลือ residual ที่แคบลง (ข้างล่าง C3, C5, N3, N4), 1 ข้อ (XAMPP webroot) เจ้าของตัดสินใจไม่ดำเนินการ
**ไม่พบ medium/high/critical** ยืนยัน 6 ข้อ (low ทั้งหมด) และ needs_validation 6 ข้อ — **แก้ครบแล้วเมื่อ 2026-10-24 (ยังไม่ commit/deploy)** ตามตาราง

คำตอบของเจ้าของที่กำหนดทิศทางการแก้: deploy จริงเป็น Ubuntu + Docker Engine, มี Cloudflare proxy (ส้ม) หน้า Caddy, โดเมน IPv4 อย่างเดียว (แต่ Cloudflare ส่งผู้เยี่ยมชม IPv6
มาที่ origin IPv4 ได้ จึงนับ IPv6 ที่ /56), `sql.view` เฉพาะ admin, RAM 4 GB; ตัดสินใจ: เพดาน login สำเร็จต่อผู้ใช้ 30 ครั้ง/10 นาที, ลิงก์ผูกกับการกระทำที่ออก,
ตารางเต็มใช้ถังรวม 300 ครั้ง/10 นาที

### confirmed 6 ข้อ (low) — แก้แล้ว

| # | ปัญหา | การแก้ | เทส |
|---|---|---|---|
| C1 | ล็อกอินสำเร็จไม่มีเพดาน: session และ audit row `login_succeeded` สะสมไม่จำกัด + sweep สแกนทั้งตาราง | `UserBudget` 30/10 นาที/ผู้ใช้ไม่คืนโควตา (`auth/user_budget.rs`, ข้อ 9.3) + index `sessions.*_expires_at` | `login_success_limit.rs` |
| C2 | ผู้ถือ `page.create` ทำให้ `GET /menu` ของทุกคนหนักขึ้นได้ไม่จำกัด | `label ≤ 100`, `icon ≤ 64`, เพจ ≤ 300 + CHECK `NOT VALID` | `pages.rs` |
| C3 | SQL console: คำสั่งที่ถูกยกเลิกไม่มี audit row | รัน + audit ใน `tokio::spawn` (ไม่ผูกกับ request) | `sql_console_limits.rs` |
| C4 | SQL console: ‘keyword’ ที่ผู้ใช้พิมพ์ได้ถึง 20,000 ตัวเข้า audit/400/หน้าประวัติ | allowlist ชื่อคำสั่ง ที่เหลือ `OTHER` | `sql_console_limits.rs` |
| C5 | ลิงก์รีเซ็ตรหัสยังใช้ได้หลังผู้ออกถูกถอนเฉพาะ `user.update` | คอลัมน์ `purpose` (reset ต้อง `user.update`) | `registration.rs` |
| C6 | `add_visits` ตอบสถานะ `is_active` ให้ผู้ที่ไม่มี `patient.view` | ต้องมี `patient.view` (+ แก้ fixture ของเทส) | `lab_visits.rs` |

### needs_validation 6 ข้อ — แก้ในโค้ด/config แล้ว บางข้อรอยืนยันบนเครื่องจริง

| # | ปัญหา | การแก้ | ที่ยังต้องยืนยัน |
|---|---|---|---|
| N1 | Cloudflare proxy → Caddy เห็น IP ของ edge ทุกคนใช้ key เดียว (ล็อก login ทั้งระบบได้ด้วย 20 คำขอ) | `ops/Caddyfile`: `trusted_proxies` = ช่วง IP ของ Cloudflare + `client_ip_headers CF-Connecting-IP` + `X-Real-IP {client_ip}`; backend เตือนเมื่อเห็นที่อยู่ที่ไม่ใช่ public | ล็อกอินจริงทั้ง IPv4/IPv6 แล้วดูหน้าประวัติ; firewall origin รับเฉพาะ Cloudflare |
| N2 | IPv6 นับที่ /64 + ตารางเต็มแล้วปล่อยไม่จำกัด | key IPv6 = /56; ตารางเต็ม → ถังรวม 300 ครั้ง/10 นาที | — |
| N3 | ผลลัพธ์ SQL ไม่จำกัด byte/array/คอลัมน์ | อ่านทีละแถวหยุดที่ 16 MiB + งบ JSON 16 MiB (จ่ายก่อนจอง) + ≤ 200 คอลัมน์ + `mem_limit` | เซลล์เดียวขนาดมากถูก buffer ได้หนึ่งครั้งก่อนถูกปฏิเสธ (sqlx) — ขอบเขตสุดท้ายคือ `mem_limit` |
| N4 | client เดียวถือ permit ทั้ง 512 ด้วย request body ค้าง | `RequestBodyTimeoutLayer` 3 วินาที/การอ่าน (หน่วงจาก 10 วินาที) | Cloudflare buffering + firewall origin; ยังไม่มีโควตาต่อ client |
| N5 | `db` ได้ `.env` ทั้งไฟล์ | `environment:` เฉพาะ `POSTGRES_*` | — (ตรวจใน container ทดลองแล้ว) |
| N6 | ทุก service อยู่ network เดียว | network `edge` / `app` / `data` (internal) | `docker network inspect` บนเครื่องจริงหลัง `up -d` |

### Hardening ที่ทำพร้อมกัน

- ปฏิเสธชื่อ login ที่ไม่ตรงรูปแบบ (`domain::username`) โดยไม่ค้น DB (NUL เคยเป็น 500 ที่ไม่ถูกนับเพดาน) + `username` ≤ 64, รหัสผ่าน ≤ 1024 ที่ DTO
- `docker-compose.yml`: log rotation, `mem_limit`/`pids_limit`, healthcheck ของ backend; `RUST_LOG` ปริยาย `info`; `--locked` ใน `backend/Dockerfile`; `.env*` ใน `.dockerignore`
- รายการ hardening อื่นที่ยังไม่ทำ (ไม่บังคับ) อยู่ใน `REPORT.md` ข้อ 6 ของ run-2: HMAC ของ `login_throttle.username_hash`, cookie `__Host-`, AAD ผูกแถวของ `crypto.rs`,
  `bangkok_midnight_utc` panic กับวันที่สุดขอบ, TOCTOU เล็ก ๆ ของ lab_visit ฯลฯ
