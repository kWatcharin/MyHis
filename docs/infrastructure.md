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
- `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `Permissions-Policy`
- ซ่อน header `Server`, response ของ `/api/*` มี `Cache-Control: no-store` (มีข้อมูลคนไข้)
- เพิ่ม script/style/รูปจากโดเมนอื่นเมื่อไร ต้องแก้ CSP ด้วย ไม่งั้นเบราว์เซอร์บล็อก (ดูได้ใน console)

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
