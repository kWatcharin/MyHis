# Infrastructure (my-his)

ทั้งระบบรันบนเซิร์ฟเวอร์เครื่องเดียวด้วย Docker Compose เครื่องขนาด 2 vCPU / 4 GB RAM เพียงพอ

## 1. การวาง container

| Container | หน้าที่ | เปิดพอร์ตออกนอกเครื่อง |
|---|---|---|
| `caddy` | หน้าบ้าน: TLS อัตโนมัติ, timeout ระดับการเชื่อมต่อ, ส่งต่อไป `frontend` (`ops/Caddyfile`; ปักเวอร์ชัน `caddy:2.11.7`) | 80, 443 |
| `frontend` | Caddy อีกตัวที่เสิร์ฟหน้าเว็บ (SPA) + security header + proxy `/api/*` ไป `backend` (`frontend/Caddyfile`) | ไม่ |
| `backend` | Axum API (ไม่เสิร์ฟหน้าเว็บ ไม่ตั้ง security header เอง) | ไม่ |
| `db` | PostgreSQL | ไม่ |

เส้นทางทราฟฟิก: อินเทอร์เน็ต → `caddy` → `frontend:8080` → (`/api/*`) `backend:8080` → `db`
**ห้ามชี้ `caddy` ไปที่ `backend` ตรง ๆ** — ข้าม CSP/`X-Frame-Options`/`Cache-Control: no-store` ทั้งหมด และไม่มีหน้าเว็บให้เสิร์ฟ

- **`DOMAIN`:** `caddy` ได้ค่า `DOMAIN` ผ่าน `environment:` ใน `docker-compose.yml` (ไม่ตั้ง = `localhost` ซึ่งออกใบรับรองภายในเครื่อง
  ใช้งานจริงไม่ได้ เห็นทันที) **ห้ามใช้ `env_file: .env` กับ `caddy`** — `.env` มี key เข้ารหัสข้อมูลผู้ป่วยและรหัสผ่านฐานข้อมูล
- **IP ผู้ใช้จริง:** `caddy` ส่ง `X-Real-IP` = `{client_ip}` (ทับค่าที่ผู้ใช้ส่งมาเสมอ) → `frontend` เอาไปตั้งเป็น `X-Forwarded-For`
  ตัวเดียวให้ `backend` (ประวัติการเข้าออกและเพดาน login ต่อ IP ใช้ค่าขวาสุด — `auth/handlers.rs` `client_ip`) `{client_ip}` คือ
  `CF-Connecting-IP` เมื่อ TCP peer อยู่ในช่วง IP ของ Cloudflare (`trusted_proxies static …` ใน `ops/Caddyfile`) ไม่งั้นเป็น IP ของ peer เอง
  — ดูหัวข้อ Cloudflare ในข้อ 6 ถ้าเข้า `frontend` ตรงโดยไม่ผ่าน `caddy`
  (dev) ไม่มี `X-Real-IP` ก็ใช้พฤติกรรมปริยายของ Caddy พอร์ต 8080 ของ `frontend` ต้องไม่ publish ออกนอกเครื่องในการใช้งานจริง
  (ใครเข้าถึงพอร์ตนั้นได้จะปลอม IP ในประวัติได้)
- **Host:** ทุกชั้นส่ง `Host` เดิมของผู้ใช้ต่อ (ปริยายของ Caddy) — ด่าน same-origin ของ backend (`docs/auth-design.md` ข้อ 7) เทียบ `Origin` กับ `Host`
- **ขีดจำกัดระดับการเชื่อมต่อ:** ตั้งใน `servers { timeouts { read_header 10s  read_body 30s  write 30s  idle … } max_header_size 16KB }`
  ของทั้ง `ops/Caddyfile` (idle 2 นาที) และ `frontend/Caddyfile` (idle 5 นาที — ยาวกว่าชั้นหน้าเพื่อไม่ปิด connection ที่ชั้นหน้ากำลังนำกลับมาใช้)
- **ด่านระดับ connection ของ backend** (`backend/src/conn_guard.rs` ห่อ `Listener` ของ `axum::serve` ไม่ต้องเพิ่ม dependency):
  connection พร้อมกันไม่เกิน **512** (เต็มแล้วไม่ accept เพิ่ม ที่เหลือรอใน backlog ของ kernel) และ connection ที่ไม่มีความคืบหน้าอ่าน/เขียน
  **5 นาที**ถูกตัด (`TimedOut`) **ลำดับสำคัญ: 5 นาทีต้องมากกว่า `keepalive` ของ upstream ใน Caddy (ปริยาย 2 นาที — ไม่ตั้งทับ)**
  เพื่อให้ Caddy ปิด connection ที่ว่างก่อนเสมอ ไม่งั้น backend อาจปิดตอนที่ Caddy เพิ่งส่ง POST เข้ามา แล้ว Caddy ตอบ 502 กลางการ login
  ช่องที่เหลือ: แยก ‘ว่างระหว่าง request’ กับ ‘ค้างกลาง header’ ไม่ได้ (ต้องใช้ `header_read_timeout` ของ hyper = ต้องเพิ่ม `hyper-util` เป็น dependency ตรง —
  ยังไม่ทำ รอเจ้าของอนุมัติ) คนส่ง header ทีละไบต์จึงถือ connection ได้จนกว่า `read_header 10s` ของ Caddy จะตัด ดังนั้นยัง **ต้องไม่ publish
  `backend` (และ `frontend`) ให้เข้าถึงโดยไม่ผ่าน Caddy**
- `restart: unless-stopped` ทุก service — Docker/เครื่องรีสตาร์ทแล้วระบบขึ้นเอง (ต้อง `docker compose up -d` อีกครั้งหลังแก้ไฟล์นี้เพื่อสร้าง container ใหม่)

Cache และ queue ยังไม่มีในระบบ ถ้าวันหนึ่งต้องมีงานเบื้องหลัง (ส่งแจ้งเตือน, สร้างไฟล์สรุป)
ให้เริ่มจากตาราง job ใน Postgres โดยใช้ `SELECT ... FOR UPDATE SKIP LOCKED` ก่อน
ซึ่งเพียงพอกับปริมาณงานระดับนี้และไม่ต้องเพิ่ม container

## 2. Network

แยกเป็นสามวง (`docker-compose.yml` ประกาศ `networks:` ชัดเจน — รอบ 4 ของ security audit; ก่อนหน้านี้ทุก service อยู่ network `default` เดียว)

| วง | สมาชิก | หมายเหตุ |
|---|---|---|
| `edge` | `caddy`, `frontend` | หน้าบ้าน |
| `app` | `frontend`, `backend` | `frontend` proxy `/api/*` ไป `backend`; **ไม่ใช่ internal** เพราะ `backend` ต้องออกเน็ตไป Cloudflare Turnstile (siteverify) |
| `data` | `backend`, `db` | `internal: true` — ไม่มีทางออกนอกเครื่อง และ publish พอร์ตผ่านวงนี้ไม่ได้ |

`caddy` ต้องไม่เห็น `db` (และไม่เห็น `backend`) และ `db` ต้องไม่ publish พอร์ต 5432 ออกจากเครื่อง
ถ้าต้องต่อฐานข้อมูลจากเครื่องส่วนตัว ให้ใช้ SSH tunnel (dev: `docker-compose.override.yml` ที่ git-ignore ต่อ `db` เข้า network `default`
เพิ่มเพื่อ publish `127.0.0.1:55432` — ไฟล์นี้ไม่ขึ้น production)

`db` ได้ environment เฉพาะ `POSTGRES_USER` / `POSTGRES_PASSWORD` / `POSTGRES_DB` (ไม่ใช้ `env_file: .env`) — `.env` มีคีย์เข้ารหัสข้อมูลผู้ป่วย
และ entrypoint ของ image postgres ลบเฉพาะตัวแปร `POSTGRES_*` ก่อนรัน server ตัวแปรอื่นจึงค้างอยู่ใน environment ของ process ที่ SQL ระดับ
superuser (`COPY ... PROGRAM`) อ่านได้ (ผลตรวจใน security audit รอบ 2)

## 3. Volume และ secret

- ข้อมูล Postgres อยู่ใน named volume
- ข้อมูลใบรับรองของ Caddy อยู่ใน named volume (ไม่อย่างนั้นจะขอใบใหม่ทุกครั้งที่ restart จนติดลิมิต)
- ค่าลับทั้งหมดอยู่ในไฟล์ `.env` ที่ไม่ขึ้น git โดยมี `.env.example` เป็นรายการชื่อตัวแปรพร้อมคำอธิบาย
- ดูคอลัมน์ที่เข้ารหัส (`patient.hn`, `nap_no`, `first_name`, `last_name`) ตรงจาก DB: SQL อย่างเดียวถอดไม่ได้ เพราะ DB
  ไม่มี key และ pgcrypto ไม่มี AES-GCM — ใช้ `ops/patient_decrypt.py` (select เป็น hex ด้วย psql แล้ว pipe เข้าสคริปต์
  ซึ่งอ่าน key จาก `.env` เอง วิธีใช้อยู่หัวไฟล์) **ห้ามติดตั้ง pgcrypto หรือใส่ key ลงในคำสั่ง SQL แทน** (ค้างใน
  `pg_stat_activity`/log/psql history) ทางนี้ไม่ผ่านการตรวจสิทธิ์ `patient.view` ของหน้าเว็บ ใช้เฉพาะตอนจำเป็น

## 4. ลำดับการเริ่มระบบ

ตั้ง healthcheck ให้ `db` และให้ `backend` รอจนฐานข้อมูลพร้อม
backend รัน migration ตอนเริ่มทำงานโดยอัตโนมัติ แต่ต้องล้มทั้ง process ถ้า migration ล้มเหลว
อย่าปล่อยให้แอปทำงานต่อบน schema ที่ไม่ตรง

## 5. Backup

เครื่องเดียวหมายความว่าพังทีเดียวหายหมด backup จึงเป็นสิ่งที่ขาดไม่ได้

- `pg_dump` อัตโนมัติทุกวัน
- เข้ารหัสไฟล์ผลลัพธ์ก่อนส่งออก
- ส่งไปเก็บนอกเครื่อง (S3, Backblaze B2 หรือที่เก็บบนคลาวด์อื่น)
- เก็บย้อนหลังอย่างน้อย 14 วัน
- **สำรอง key เข้ารหัสข้อมูลผู้ป่วย (`DATA_ENCRYPTION_KEY`, `BLIND_INDEX_KEY` ใน `.env`) แยกจาก DB dump**
  เก็บคนละที่กับไฟล์ backup ของฐานข้อมูล — ข้อมูลในคอลัมน์ที่เข้ารหัสอ่านไม่ได้เลยถ้า key หาย และถ้า dump
  กับ key อยู่ที่เดียวกัน การเข้ารหัสก็ไม่มีความหมาย
- **ทดสอบ restore จริงอย่างน้อยหนึ่งครั้ง** และทำซ้ำทุกครั้งที่เปลี่ยนสคริปต์
  คนส่วนใหญ่รู้ว่า backup ใช้ไม่ได้ในวันที่ต้องใช้มันเท่านั้น

สคริปต์ทั้งหมดอยู่ใน `ops/`

## 6. ความปลอดภัยของเครื่อง

- SSH ปิดการ login ด้วยรหัสผ่าน ใช้ key เท่านั้น
- firewall เปิดเฉพาะพอร์ต 22, 80, 443 — เมื่อใช้ Cloudflare proxy ให้เปิด **80/443 เฉพาะช่วง IP ของ Cloudflare** (ดูหัวข้อ Cloudflare ด้านล่าง)
- เปิดการอัปเดตความปลอดภัยอัตโนมัติของระบบปฏิบัติการ
- บังคับ HTTPS ตั้งแต่วันแรก ไม่มีโหมด http สำหรับใช้งานจริง
- container รันด้วยผู้ใช้ที่ไม่ใช่ root (backend = `nobody`, frontend = Caddy เป็น `nobody` ฟังพอร์ต 8080)
- มี limit ต่อ container ใน `docker-compose.yml`: backend `mem_limit: 1g` / `pids_limit: 1024` (tokio ใช้ blocking pool ได้ถึง 512 thread), caddy และ frontend `mem_limit: 256m`
  (db ยังไม่ตั้ง mem_limit — ปรับหลังวัดการใช้งานจริง) และ backend มี `healthcheck` (GET `/health` ผ่าน bash `/dev/tcp` เพราะ image ไม่มี curl)

### Cloudflare (proxy หน้า Caddy)

โดเมนใช้ Cloudflare proxy (เมฆส้ม) ผลคือ TCP peer ที่ `caddy` เห็นเป็น IP ของ Cloudflare edge ไม่ใช่ผู้ใช้ — ถ้าไม่จัดการ ทุกคนใช้ IP เดียวกัน
เพดาน login ต่อ IP (`auth-design.md` ข้อ 9.2) กลายเป็นถังเดียวของทั้งระบบ และหน้าประวัติ login แสดง IP ของ edge

- **Caddy (`ops/Caddyfile`) จัดการแล้ว:** `trusted_proxies static <ช่วง IP ของ Cloudflare>` + `client_ip_headers CF-Connecting-IP`
  และ `X-Real-IP {client_ip}` — เชื่อ `CF-Connecting-IP` เฉพาะเมื่อ peer อยู่ในช่วงของ Cloudflare (ใครยิงตรงเข้า origin แล้วส่ง header ปลอมจะถูกเมิน)
  รายการช่วง IP ดึงจาก `https://www.cloudflare.com/ips-v4` และ `/ips-v6` (ฝังใน Caddyfile เมื่อ 2026-10-23) **ตรวจเทียบรายการปัจจุบันปีละครั้ง**
  ไม่ใช้ plugin เพราะต้อง build image Caddy เอง (supply chain ใหม่)
- **IPv6:** Cloudflare เปิด IPv6 compatibility เป็นค่าปริยาย ผู้เยี่ยมชม IPv6 เข้าต้นทางที่มีแค่ A record ได้ และ `CF-Connecting-IP` เป็น IPv6 ของเขา
  (Pseudo IPv4 ปิดเป็นค่าปริยาย) — เพดานต่อ IP นับ IPv6 ที่ /56 จึงรองรับ (อย่าเปิด Pseudo IPv4 โหมด Overwrite เพราะจะเปลี่ยน `CF-Connecting-IP`
  เป็นค่าที่แปลงแล้ว)
- **งานเจ้าของบนเครื่องจริง (ยังไม่ได้ทำ):**
  1. firewall เปิดพอร์ต 80/443 เฉพาะช่วง IP ของ Cloudflare (หรือใช้ Authenticated Origin Pulls) — ไม่งั้นคนที่รู้ IP ของ origin ข้าม Cloudflare ได้
  2. SSL/TLS mode = **Full (strict)**
  3. (ทางเลือก ลดผลของ request ที่ body ค้าง) Configuration Rule: request body buffering = Full — Cloudflare จะถือ request ที่ยังส่งไม่ครบแทน backend
     (ตัวเลือกนี้ยังไม่ได้ทดสอบกับระบบนี้)
  4. ล็อกอิน 1 ครั้งผ่านโดเมนจริงทั้งจากเครือข่าย IPv4 และ IPv6 (ถ้ามี) แล้วดูหน้าประวัติ login ว่า IP เป็นของผู้ใช้จริง ไม่ใช่ IP ของ Cloudflare
     หรือ container — backend เตือนใน log หนึ่งครั้งถ้าเห็นที่อยู่ที่ไม่ใช่ public (loopback/private/link-local)
  5. ตรวจว่า Caddy ขอ/ต่ออายุใบรับรองผ่าน proxy ได้ (ถ้าไม่ได้ ใช้ Cloudflare Origin CA หรือ DNS-01)

### Security header (ตั้งไว้แล้วใน `frontend/Caddyfile`)

- `Content-Security-Policy` — script ได้เฉพาะไฟล์ของเว็บเอง (ห้าม inline script — ตัวตั้งธีมแยกเป็น
  `public/theme-init.js`), style อนุญาต inline (React/chart), `frame-ancestors 'none'`
  - ยกเว้น `https://challenges.cloudflare.com` ใน `script-src` และ `frame-src` สำหรับ Cloudflare Turnstile หน้า login
    (docs/auth-design.md ข้อ 9.1) — Cloudflare เห็น IP/ข้อมูลเบราว์เซอร์ของคนที่เปิดหน้า login เท่านั้น หน้านั้นไม่มีข้อมูลผู้ป่วย
- `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `Permissions-Policy`
- ซ่อน header `Server`, response ของ `/api/*` มี `Cache-Control: no-store` (มีข้อมูลคนไข้)
- เพิ่ม script/style/รูปจากโดเมนอื่นเมื่อไร ต้องแก้ CSP ด้วย ไม่งั้นเบราว์เซอร์บล็อก (ดูได้ใน console)

### หน้า SQL (`/data/script`, backend/src/sql_console/)

รัน SQL ดิบจากหน้าเว็บแล้วถอดรหัสคอลัมน์ผู้ป่วยให้ (Postgres ถอด AES-GCM ของแอปเองไม่ได้) — backend ต่อ DB ด้วย superuser
SQL ดิบผ่าน pool หลักจะ `COPY ... TO PROGRAM` (รัน shell ใน container db), `pg_read_file`, `RESET ROLE` ได้แม้อยู่ใน
transaction READ ONLY จึงรันด้วย **role แยก `my_his_sql_reader`** เท่านั้น

- เปิดด้วย `SQL_CONSOLE_DB_PASSWORD` ใน `.env` (ยาว ≥ 16 ตัว เช่น `openssl rand -hex 24`) แล้ว restart backend — ไม่ตั้ง =
  ปิดหน้า SQL (API ตอบ 503) host/ฐานข้อมูลใช้ของ `DATABASE_URL`
- **backend สร้าง/อัปเดต role เองตอนเริ่ม** (หลัง migration): LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT
  NOREPLICATION NOBYPASSRLS CONNECTION LIMIT 4, ค่าเริ่มต้น read only + statement_timeout 5 วินาที, ตั้งรหัสตาม `.env`
  ทุกครั้ง (หมุนรหัส = แก้ `.env` แล้ว restart) และ **ไม่ยอมเริ่ม** ถ้า role เป็น superuser/มีสิทธิ์เกิน/เป็นสมาชิก role อื่น
  (เช่น `pg_read_all_data`, `pg_execute_server_program`) — ทำได้เพราะแอปยังเป็น superuser ถ้าเปลี่ยนเป็นบัญชีธรรมดาตาม
  checklist ด้านล่าง ขั้นสร้าง role ต้องย้ายไปทำด้วยมือ (sync grant ยังทำได้เพราะแอปเป็นเจ้าของตาราง)
- **grant ระดับคอลัมน์ sync ใหม่ทุกครั้งที่เริ่ม** ตาม allowlist ใน `sql_console/setup.rs`: อ่านได้เฉพาะ `READABLE_TABLES`
  ยกเว้น `HIDDEN_COLUMNS` (`users.password_hash`, `registration_links.token_hash`) — `sessions`, `login_throttle`,
  `_sqlx_migrations` และ **ตารางใหม่มองไม่เห็นจนกว่าจะเพิ่มเข้า allowlist** คอลัมน์ลับที่เพิ่มในตารางที่อ่านได้ต้องเพิ่มใน
  `HIDDEN_COLUMNS` (ไม่งั้นถูก grant อัตโนมัติ)
- ทุกการรัน: connection ใหม่ (ไม่ใช้ pool), READ ONLY, คำสั่งเดียว, SELECT/WITH/VALUES/TABLE ถูกครอบด้วย `LIMIT 501`
  (EXPLAIN/SHOW รันตรง), ถอดรหัสเฉพาะเมื่อมี `patient.view` (คอลัมน์ตรงตามต้นทาง OID/attnum + ciphertext ที่ฝังใน
  expression เช่น `concat(p.name, pt.first_name)` — `sql_console/embedded.rs` ถอดแค่ค่าในผลลัพธ์ ไม่ส่ง plaintext
  กลับเข้า DB), บันทึก `audit_log` โดยไม่เก็บตัว SQL
  และไม่ log SQL/ข้อความ error ของ DB
- **งานถอดรหัสต่อคำขอมีเพดาน** (กันคำขอที่สร้างงาน CPU ไม่รู้จบ): ลองถอด ciphertext ที่ฝังได้รวมไม่เกิน 200,000 ครั้งต่อคำขอ
  (ใช้ร่วมทุกเซลล์ เกินแล้วค่าที่เหลือแสดงเป็น hex), ข้าม hex run ยาวเกิน 4,096 ตัวอักษร, แปลงผลลัพธ์/ถอดรหัสรันบน
  `spawn_blocking` ไม่ยึด async worker — ตัวเลขอยู่ที่ `embedded.rs` (`MAX_DECRYPT_ATTEMPTS`, `MAX_HEX_RUN`)
- **ขนาดผลลัพธ์ต่อคำขอมีเพดาน** (security audit รอบ 2 — เดิมจำกัดแค่แถว/เวลา/ความยาว SQL ทำให้ `select repeat('x', 67108864)` ยาว 33 ตัวอักษรกิน
  หน่วยความจำ backend ~237 MB): (ก) ไบต์ที่รับจาก Postgres รวม ≤ **16 MiB** — อ่านทีละแถวและหยุดทันทีที่เกิน (`repository::execute`),
  (ข) งบหน่วยความจำของ JSON ที่แปลงแล้ว ≤ 16 MiB — แต่ละช่อง ‘จ่าย’ ก่อนจอง (text = ความยาว, bytea ×2, json ×4, array ×8–16, numeric = ความยาวข้อความ
  ที่ **คำนวณจาก header** ก่อนถอด เพราะ numeric 10 ไบต์ขยายเป็น ~131,000 ตัวอักษร) `value::Budget`, (ค) คอลัมน์ ≤ 200 — ปฏิเสธหลัง prepare ก่อนรัน
  เกิน = `400 query failed` + ข้อความ `result too large …` (ยังบันทึก audit); ขีดจำกัดที่เหลือ: sqlx อ่านทั้งแถวก่อนคืน เซลล์เดียวที่ใหญ่มาก (ได้ถึง 1 GB ในทางทฤษฎี
  ภายใน statement_timeout 5 วินาที) ถูก buffer ได้หนึ่งครั้งก่อนถูกปฏิเสธ — ขอบเขตสุดท้ายคือ `mem_limit` ของ backend และผู้ถือ `sql.view` มีเฉพาะ admin
- **การรัน + บันทึก audit ทำใน task แยก** (`tokio::spawn`) ที่ไม่ผูกกับ request: ผู้ใช้ปิดแท็บหรือครบ 10 วินาทีของ `TimeoutLayer` กลางคำสั่งแล้ว audit ยังถูกเขียน
  เมื่อคำสั่งจบ (ไม่ครอบคลุม process ตายกลางทาง) · คำสั่งที่ถูกบล็อกบันทึก `keyword` เป็นชื่อคำสั่งจากชุดคงที่หรือ `OTHER` เสมอ
- ตัวช่วย `:hn('...')` / `:nap_no('...')` ต้องมี `patient.view` (ไม่มี = `400 helpers need patient.view`) เพราะแปลงเป็น blind
  index ที่เทียบกับ `hn_idx` / `nap_no_idx` ซึ่ง role อ่านได้ — ไม่ใช่สิทธิ์ `sql.view` อย่างเดียว
- คำสั่งที่คำแรกไม่ใช่ SELECT/WITH/VALUES/TABLE/EXPLAIN/SHOW หรือมี `;` คั่นหลายคำสั่ง ไม่ส่งไป DB เลย ตอบ
  `400 statement not allowed` ให้หน้าเว็บเตือน (ยังบันทึก audit status `blocked`) — error ของ Postgres ที่แปลว่าแก้ข้อมูล
  (`25006` read-only, WITH ที่มี DELETE ข้างใน, `SELECT ... INTO`) แปลงเป็นคำเตือนเดียวกัน ด่านจริงยังเป็นสิทธิ์ของ role + READ ONLY
- เทส `backend/tests/sql_console.rs` สร้าง role ชื่อ `my_his_sqlt_*` ต่อเทสแล้วลบตอนจบ — ถ้าเทสล้มกลางทาง role อาจค้างใน
  cluster (ไม่มีสิทธิ์อะไรเหลือ) ลบได้ด้วย `drop role`

### Checklist ก่อนขึ้นเครื่องจริง (จาก security audit 2026-10-12 — ยังไม่ได้ทำ)

- **แยกบัญชี DB ของแอปที่ไม่ใช่ superuser** — ตอนนี้ backend ต่อด้วย `POSTGRES_USER` ซึ่งเป็น superuser ของ image
  postgres (ทำอะไรบนเครื่อง DB ก็ได้ถ้าวันหนึ่งมีช่องโหว่) วิธีทำ: สร้าง role `my_his_app` แบบ `LOGIN NOSUPERUSER
  NOCREATEDB NOCREATEROLE` เป็นเจ้าของ schema/ตารางของแอป (`ALTER ... OWNER TO`, แอปรัน migration เองตอนเริ่มจึงต้องเป็น
  เจ้าของ) แล้วเปลี่ยน `DATABASE_URL` ใน `.env` ให้ใช้บัญชีนี้ — `POSTGRES_USER` เหลือไว้ใช้ backup/ดูแลเท่านั้น
- **HSTS** (`Strict-Transport-Security`) ใส่ที่ Caddy หน้าบ้านเมื่อมี HTTPS จริงแล้วเท่านั้น
- **`ops/Caddyfile`** มีแล้ว (proxy ไป `frontend:8080`, TLS อัตโนมัติ, timeout ระดับการเชื่อมต่อ — ดูข้อ 1)
  **rate limit ต่อ IP ของ login อยู่ในแอปแล้ว** (`auth-design.md` ข้อ 9.2: ไม่สำเร็จเกิน 20 ครั้ง/10 นาที/IP → `429`) ร่วมกับตัวนับต่อชื่อ
  (ล็อก 60 วินาที), คิว argon2 ของ login (≤ 3 พร้อมกัน, ค้างรวม ≤ 64 แล้ว `503`), เพดาน audit ของชื่อที่ไม่มีในระบบ 60 แถว/นาที,
  ขนาด body ≤ 256 KiB (`DefaultBodyLimit` ใน `app.rs`) และจำนวน id ในอาร์เรย์ของคำขอ (โรคต่อผู้ป่วย/role ต่อผู้ใช้ ≤ 100)
  ที่เหลือเป็นทางเลือกเสริมระดับเครื่อง: rate limit ของ request ทั่วไปต่อ IP (plugin rate-limit ของ Caddy ต้อง build image เอง = ที่มา
  ของ supply chain เพิ่ม หรือ fail2ban อ่าน log — บรรทัด `warn` ของ backend เมื่อ IP ชนเพดานมี IP ให้ใช้)
  **เพดานต่อ IP เชื่อ `X-Forwarded-For` ค่าขวาสุด** จึงถูกต้องก็ต่อเมื่อทุกคำขอผ่าน Caddy สองชั้นตามข้อ 1 (ผู้ที่เข้า backend ตรงปลอมค่านี้ได้)
  และเมื่อใช้ Cloudflare ต้องให้ Caddy อ่าน `CF-Connecting-IP` ตามหัวข้อ Cloudflare ด้านบน (ไม่งั้นทุกคนใช้ key ของ Cloudflare edge)
- **ทดสอบ config Caddy ก่อนใช้จริง** (`ops/Caddyfile` ยังไม่เคยรันกับโดเมนจริง): `docker run --rm -e DOMAIN=example.test -v ./ops/Caddyfile:/etc/caddy/Caddyfile:ro
  caddy:2.11.7 caddy adapt --config /etc/caddy/Caddyfile` แล้วเปิดหน้าเว็บผ่านโดเมนจริงตรวจ header ของหน้า HTML (CSP, X-Frame-Options) และ
  `/api/auth/login-config` (`Cache-Control: no-store`) และดูในหน้าประวัติการเข้าออกว่า IP เป็นของผู้ใช้จริง ไม่ใช่ IP ของ container
- เปลี่ยนรหัส admin ที่สร้างจาก `INITIAL_ADMIN_PASSWORD` ผ่านหน้าเว็บ แล้วเปลี่ยนค่าใน `.env` ไม่ให้ตรงกับรหัสที่ใช้อยู่
- **ยืนยัน tag ของ Caddy** — `caddy:2.11.7` และ `caddy:2.11.7-alpine` ดึงและรันจริงได้ (2026-10-24: ทดลอง `caddy validate`, 2 ชั้น และ compose ใหม่ด้วยค่า dummy);
  ยังไม่เคยรันบนเครื่องปลายทางจริง
- **Cloudflare Turnstile key จริง** — ตอนนี้ใช้ test key (ไม่ได้ป้องกันจริง backend เตือนใน log ตอนเริ่มทุกครั้งที่ยังใช้อยู่) เมื่อมีโดเมนแล้ว: Cloudflare dashboard →
  Turnstile → Add widget (hostname = โดเมนจริง ไม่ใส่ localhost, โหมด Managed) แล้วใส่ `TURNSTILE_SITE_KEY` /
  `TURNSTILE_SECRET_KEY` (+ `TURNSTILE_HOSTNAME` = โดเมน) ใน `.env` แล้ว restart backend — backend ต้องออกเน็ตไป
  `challenges.cloudflare.com:443` ได้ (siteverify) ถ้า firewall ขาออกถูกจำกัด — เปลี่ยนเป็น key จริงแล้วเปิด /login
  ดูว่า widget ผ่าน ถ้าขึ้น error เรื่องโดเมน ให้ผ่อน `Referrer-Policy` จาก `no-referrer` (test key ใช้ได้ทุกโดเมนจึงยังพิสูจน์ไม่ได้)

## 7. Log และการเฝ้าระวัง

- การหมุนเวียน log ของ Docker ตั้งแล้วใน `docker-compose.yml` (`json-file` `max-size: 10m`, `max-file: 3` ทุก service) ไม่ให้ดิสก์เต็ม
- backend ใช้ระดับ log `info` เป็นค่าปริยาย (ตั้งทับด้วย `RUST_LOG`) — ก่อนหน้านี้ค่าปริยายคือ `error` ทำให้บรรทัด `warn` ที่ใช้เฝ้าระวัง
  (Turnstile ใช้ test key/ปิด, IP ชนเพดาน login, ตาราง IP เต็ม, เห็นที่อยู่ที่ไม่ใช่ public) เงียบ
- ทุก log ของ backend ควรมี request id
- ห้ามมีรหัสผ่าน token หรือ cookie อยู่ใน log
- ใช้บริการตรวจสอบ uptime แบบฟรีคอยยิง `/health` ก็เพียงพอสำหรับโปรเจกต์ขนาดนี้

## 8. การ deploy

pull โค้ดจาก git แล้ว `docker compose up -d --build` บนเครื่อง
ไม่ต้องมี staging สำหรับโปรเจกต์ขนาดนี้ แต่ควรทำ backup ก่อน deploy ที่มี migration ทุกครั้ง

หมายเหตุสำหรับการ build: SQLx ต้องมีโฟลเดอร์ `.sqlx` ที่อัปเดตแล้ว (สั่ง `cargo sqlx prepare`)
ไม่อย่างนั้น build ใน Docker จะล้มเหลวเพราะไม่มีฐานข้อมูลให้เชื่อมต่อตอนคอมไพล์
