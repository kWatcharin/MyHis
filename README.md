# my-his

เครื่องมือช่วยงานสำหรับพยาบาล ใช้งานส่วนตัวภายในครอบครัว — **ไม่ใช่ระบบของโรงพยาบาลและไม่ใช่เวชระเบียนทางการ**
ใช้บนมือถือเป็นหลัก รันเป็น backend เดียวบนเซิร์ฟเวอร์เครื่องเดียวด้วย Docker Compose

> ผู้พัฒนา (รวมถึง Claude Code) อ่าน [`CLAUDE.md`](CLAUDE.md) ก่อนเริ่มงานทุกครั้ง — กฎโครงสร้าง ข้อห้าม และขอบเขตข้อมูลผู้ป่วยอยู่ที่นั่น

## ทำอะไรได้แล้ว

| กลุ่ม | ความสามารถ |
|---|---|
| บัญชีและสิทธิ์ | login แบบ session cookie (ไม่ใช้ JWT), Cloudflare Turnstile, จำกัดการเดารหัสผ่าน, role/permission, ตั้งค่าเมนู (เพจ) แบบลากวาง, ดูหน้าจอในมุมของ role อื่น |
| ผู้ใช้งาน | สร้างบัญชีและรีเซ็ตรหัสผ่านด้วย **ลิงก์ลงทะเบียน** — ผู้ใช้ตั้งรหัสเอง ไม่มีรหัสผ่านชั่วคราว |
| ผู้ป่วย | ลงทะเบียนผู้ป่วย + โรคประจำตัว คอลัมน์ระบุตัวตนเข้ารหัส AES-256-GCM ที่ชั้นแอป ค้นหาด้วย blind index |
| นัดหมายแล็ป | กลุ่มนัด, รายละเอียดนัด, พิมพ์เอกสาร |
| ข้อมูลตั้งต้น (master) | ปีงบประมาณ, คำนำหน้า, สัญชาติ, กลุ่มผู้ป่วย, โรค, แล็ป, หมวด/หน่วยนับ/รายการคลัง |
| คลัง | รับเข้า + ยอดยกมา, ล็อต, ledger ที่เพิ่มได้อย่างเดียว (เบิก/นับ/ปรับยอด อยู่ระหว่างออกแบบ) |
| ตรวจสอบย้อนหลัง | ประวัติ login/logout และประวัติการเพิ่ม/แก้ไข/ลบ/ส่งออกทุกหน้า |
| หน้า SQL | `/data/script` รัน SQL อ่านอย่างเดียวด้วย role ฐานข้อมูลแยก (admin เท่านั้น) |

แผนถัดไป: ฟีเจอร์งานพยาบาลโดยตรง (ตารางเวร, checklist, คำนวณยา — ผลคำนวณต้องแสดงสูตรและหน่วยเสมอ)

## Stack

| ส่วน | ใช้ |
|---|---|
| Backend | Rust, Axum, Tokio, SQLx (SQL เขียนเอง + compile-time check) |
| Database | PostgreSQL 16 |
| Frontend | React 19, TypeScript, Vite, Tailwind + shadcn/ui, Bun |
| Chatbot (วางโครง) | Python 3.14, FastAPI, uv, gunicorn + uvicorn worker, psycopg 3 — ruff + pyrefly + pytest |
| หน้าบ้าน | Caddy (TLS อัตโนมัติ) หลัง Cloudflare proxy |
| Deploy | Docker Compose บนเซิร์ฟเวอร์เครื่องเดียว |

```
อินเทอร์เน็ต → Cloudflare → caddy (TLS, 80/443) → frontend (static + security header) → /api/* → backend → db
                                                                                              └→ chatbot (เรียกจาก backend เท่านั้น)
```

มี 5 container — publish พอร์ตเฉพาะ `caddy` ส่วน `backend`, `frontend`, `db`, `chatbot` อยู่ใน network ภายใน
(`db` อยู่วง `internal` ออกนอกเครื่องไม่ได้; `chatbot` อยู่วง `chat` ที่มีแค่ `backend`) รายละเอียดใน [`docs/infrastructure.md`](docs/infrastructure.md)

## โครงสร้าง

```
backend/     Rust (Axum) — แบ่งโมดูลตามฟีเจอร์ใน src/, migration ใน migrations/, เทส API ใน tests/
frontend/    React — src/features/<ฟีเจอร์>/ หนึ่งฟีเจอร์ต่อหนึ่งโฟลเดอร์
chatbot/     Python (FastAPI) — src/my_his_chatbot/<ฟีเจอร์>/, เทสใน tests/ (ดู chatbot/README.md)
ops/         Caddyfile หน้าบ้าน, สคริปต์ตรวจ deploy, เครื่องมือถอดรหัสข้อมูลผู้ป่วยนอกแอป, seed สัญชาติ
docs/        เอกสารออกแบบ (ดูด้านล่าง)
```

## เริ่มพัฒนาบนเครื่อง

ต้องมี Docker, Bun (หรือ Node 22) และ `openssl` ไว้สร้าง key — ไม่จำเป็นต้องติดตั้ง Rust บนเครื่อง ใช้ container แทนได้

1. **ตั้งค่า environment**

   ```bash
   cp .env.example .env
   openssl rand -hex 32   # ใส่ DATA_ENCRYPTION_KEY
   openssl rand -hex 32   # ใส่ BLIND_INDEX_KEY (ต้องต่างจากตัวแรก)
   ```

   แก้รหัสผ่านฐานข้อมูลและ `INITIAL_ADMIN_PASSWORD` ด้วย ค่า Turnstile ใน `.env.example` เป็น test key ของ Cloudflare
   (ผ่านทุกครั้ง ใช้ได้เฉพาะ dev) อยากเปิดหน้า SQL ให้ตั้ง `SQL_CONSOLE_DB_PASSWORD` (`openssl rand -hex 24`)

2. **เปิดพอร์ตสำหรับ dev** (ผูก `127.0.0.1` เท่านั้น — ห้ามวางไฟล์นี้บนเครื่องจริง)

   ```bash
   cp docker-compose.override.example.yml docker-compose.override.yml
   ```

   | พอร์ตบน host | ไปที่ |
   |---|---|
   | `55432` | PostgreSQL (สำหรับ `psql`, `sqlx-cli`, GUI client) |
   | `8081` | backend |
   | `3000` | frontend ที่ build แล้ว |
   | `8082` | chatbot (container — หรือ dev server นอก Docker ที่พอร์ตเดียวกัน) |

3. **รัน stack** — ไม่ต้องรัน `caddy` บนเครื่อง dev (80/443 มักชนกับ XAMPP/IIS)

   ```bash
   docker compose up -d --build db backend frontend chatbot
   ```

   backend รัน migration เองตอนเริ่ม (ล้ม = process หยุด) และสร้าง admin คนแรกจาก `INITIAL_ADMIN_*` ถ้ายังไม่มีผู้ใช้

4. **พัฒนา frontend แบบ hot reload** — Vite ที่ `http://localhost:5173` proxy `/api` ไปหา backend ที่พอร์ต 8081

   ```bash
   cd frontend
   bun install
   bun run dev
   ```

## ตรวจโค้ดก่อนส่งงาน

```bash
# backend (ใน backend/)
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test                 # เทส API ใน tests/ ใช้ #[sqlx::test] ได้ DB สะอาดต่อเทส
cargo sqlx prepare         # ทุกครั้งที่แก้ query — .sqlx/ ต้องขึ้น git ไม่งั้น build ใน Docker ล้ม

# frontend (ใน frontend/)
./node_modules/.bin/tsc -b
bun run lint
bun run build

# chatbot (ใน chatbot/) — pyrefly ต้องผ่านก่อนรันทุกครั้ง (docs/python-guidelines.md ข้อ 18)
uv run ruff format
uv run ruff check
uv run pyrefly check
uv run pytest              # ตรวจ pyrefly ให้เองก่อนรันเทส
uv run python scripts/dev.py   # dev server 127.0.0.1:8082 (ตรวจ pyrefly ก่อน start)
```

migration ใหม่: `cargo sqlx migrate add <ชื่อ>` — ตารางหรือคอลัมน์ใหม่ต้องอัปเดต data dict
(`docs/my-his-data-dict.xlsx`) และตัดสินใจว่าหน้า SQL มองเห็นหรือไม่ (`backend/src/sql_console/setup.rs`)

## Deploy

```bash
git pull
docker compose up -d --build
sh ops/check-deploy.sh            # ตรวจ stack ที่รันอยู่เทียบกับที่ออกแบบไว้ (อ่านอย่างเดียว)
sh ops/check-cloudflare-ips.sh    # เทียบช่วง IP Cloudflare ใน Caddyfile กับของจริง
```

- ตั้ง `DOMAIN` ใน `.env` และใช้ Turnstile key จริง
- **ทำ backup ก่อน deploy ที่มี migration ทุกครั้ง**
- **สำรอง `DATA_ENCRYPTION_KEY` และ `BLIND_INDEX_KEY` แยกจาก DB dump** — key หายเท่ากับข้อมูลผู้ป่วยที่เข้ารหัสไว้อ่านไม่ได้ตลอดกาล
- รายการที่ต้องทำบนเครื่องจริงและ Cloudflare (firewall, SSH, ยืนยัน IP จริง) อยู่ใน
  [`docs/security-audit.md`](docs/security-audit.md) และ [`docs/infrastructure.md`](docs/infrastructure.md) ข้อ 6
- สคริปต์ backup/restore อัตโนมัติยังไม่ได้ทำ — ข้อกำหนดอยู่ใน `docs/infrastructure.md` ข้อ 5

## ข้อมูลผู้ป่วย

ระบบเก็บ HN, เลขประจำตัวคนไข้, ชื่อ และสกุล โดย **เข้ารหัสก่อนลงฐานข้อมูลเสมอ** และไม่เก็บเลขบัตรประชาชน
API ส่ง `uuid` ออกไปเท่านั้น ไม่ส่ง id ภายใน ดูข้อมูลที่เข้ารหัสตรงจาก DB ได้ด้วย `ops/patient_decrypt.py`
(วิธีใช้อยู่หัวไฟล์ — ห้ามใช้ pgcrypto หรือใส่ key ในคำสั่ง SQL) กฎทั้งหมดอยู่ใน `CLAUDE.md` หัวข้อ "ขอบเขตข้อมูล"

## เอกสาร

| ไฟล์ | เนื้อหา |
|---|---|
| [`CLAUDE.md`](CLAUDE.md) | บริบทหลัก กฎโครงสร้าง ข้อห้าม |
| [`docs/rust-guidelines.md`](docs/rust-guidelines.md) | กฎการเขียน Rust ของโปรเจกต์ |
| [`docs/auth-design.md`](docs/auth-design.md) | session, ตาราง, API, flow, สิทธิ์ |
| [`docs/inventory-design.md`](docs/inventory-design.md) | ระบบคลัง |
| [`docs/stock-issues-plan.md`](docs/stock-issues-plan.md) | แผนทำเอกสารเบิกคลัง |
| [`docs/infrastructure.md`](docs/infrastructure.md) | container, network, backup, ความปลอดภัยของเครื่อง |
| [`docs/security-audit.md`](docs/security-audit.md) | ผล security audit และสิ่งที่ต้องทำตอน deploy |
| [`docs/my-his-data-dict.xlsx`](docs/my-his-data-dict.xlsx) | data dictionary ของทุกตาราง |
