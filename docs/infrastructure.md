# Infrastructure (my-his)

ทั้งระบบรันบนเซิร์ฟเวอร์เครื่องเดียวด้วย Docker Compose เครื่องขนาด 2 vCPU / 4 GB RAM เพียงพอ

## 1. การวาง container

| Container | หน้าที่ | เปิดพอร์ตออกนอกเครื่อง |
|---|---|---|
| `caddy` | reverse proxy, TLS, จำกัด request ต่อ IP | 80, 443 |
| `backend` | Axum API (เสิร์ฟไฟล์ frontend ไปด้วยได้ในช่วงแรก) | ไม่ |
| `db` | PostgreSQL | ไม่ |

Cache และ queue ยังไม่มีในระบบ ถ้าวันหนึ่งต้องมีงานเบื้องหลัง (ส่งแจ้งเตือน, สร้างไฟล์สรุป)
ให้เริ่มจากตาราง job ใน Postgres โดยใช้ `SELECT ... FOR UPDATE SKIP LOCKED` ก่อน
ซึ่งเพียงพอกับปริมาณงานระดับนี้และไม่ต้องเพิ่ม container

## 2. Network

แยกเป็นสองวง

- วงหน้าบ้าน: `caddy` กับ `backend`
- วงหลังบ้าน: `backend` กับ `db`

`caddy` ต้องไม่เห็น `db` และ `db` ต้องไม่ publish พอร์ต 5432 ออกจากเครื่อง
ถ้าต้องต่อฐานข้อมูลจากเครื่องส่วนตัว ให้ใช้ SSH tunnel

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
- firewall เปิดเฉพาะพอร์ต 22, 80, 443
- เปิดการอัปเดตความปลอดภัยอัตโนมัติของระบบปฏิบัติการ
- บังคับ HTTPS ตั้งแต่วันแรก ไม่มีโหมด http สำหรับใช้งานจริง
- container รันด้วยผู้ใช้ที่ไม่ใช่ root (backend = `nobody`, frontend = Caddy เป็น `nobody` ฟังพอร์ต 8080)

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
  (EXPLAIN/SHOW รันตรง), ถอดรหัสเฉพาะเมื่อมี `patient.view`, บันทึก `audit_log` โดยไม่เก็บตัว SQL
  และไม่ log SQL/ข้อความ error ของ DB
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
- **`ops/Caddyfile`** (Caddy หน้าบ้านที่ publish 80/443) ยังไม่มีไฟล์ — ตอนสร้าง: proxy ไปที่ `frontend:8080` (ไม่ใช่ :80
  — container frontend ฟัง 8080 เพราะรันเป็น nobody), TLS อัตโนมัติ, `trusted_proxies` ถ้ามี proxy
  ซ้อน (ประวัติการเข้าออกใช้ค่าขวาสุดของ `X-Forwarded-For`), rate limit ที่ `/api/auth/login` (ต้องใช้ plugin
  rate-limit ของ Caddy หรือ fail2ban)
- เปลี่ยนรหัส admin ที่สร้างจาก `INITIAL_ADMIN_PASSWORD` ผ่านหน้าเว็บ แล้วเปลี่ยนค่าใน `.env` ไม่ให้ตรงกับรหัสที่ใช้อยู่
- **Cloudflare Turnstile key จริง** — ตอนนี้ใช้ test key (ไม่ได้ป้องกันจริง) เมื่อมีโดเมนแล้ว: Cloudflare dashboard →
  Turnstile → Add widget (hostname = โดเมนจริง ไม่ใส่ localhost, โหมด Managed) แล้วใส่ `TURNSTILE_SITE_KEY` /
  `TURNSTILE_SECRET_KEY` (+ `TURNSTILE_HOSTNAME` = โดเมน) ใน `.env` แล้ว restart backend — backend ต้องออกเน็ตไป
  `challenges.cloudflare.com:443` ได้ (siteverify) ถ้า firewall ขาออกถูกจำกัด — เปลี่ยนเป็น key จริงแล้วเปิด /login
  ดูว่า widget ผ่าน ถ้าขึ้น error เรื่องโดเมน ให้ผ่อน `Referrer-Policy` จาก `no-referrer` (test key ใช้ได้ทุกโดเมนจึงยังพิสูจน์ไม่ได้)

## 7. Log และการเฝ้าระวัง

- ตั้งการหมุนเวียน log ของ Docker ไม่ให้ดิสก์เต็ม
- ทุก log ของ backend ควรมี request id
- ห้ามมีรหัสผ่าน token หรือ cookie อยู่ใน log
- ใช้บริการตรวจสอบ uptime แบบฟรีคอยยิง `/health` ก็เพียงพอสำหรับโปรเจกต์ขนาดนี้

## 8. การ deploy

pull โค้ดจาก git แล้ว `docker compose up -d --build` บนเครื่อง
ไม่ต้องมี staging สำหรับโปรเจกต์ขนาดนี้ แต่ควรทำ backup ก่อน deploy ที่มี migration ทุกครั้ง

หมายเหตุสำหรับการ build: SQLx ต้องมีโฟลเดอร์ `.sqlx` ที่อัปเดตแล้ว (สั่ง `cargo sqlx prepare`)
ไม่อย่างนั้น build ใน Docker จะล้มเหลวเพราะไม่มีฐานข้อมูลให้เชื่อมต่อตอนคอมไพล์
