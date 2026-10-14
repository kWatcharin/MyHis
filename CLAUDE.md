# my-his

เครื่องมือช่วยงานสำหรับพยาบาล ใช้งานส่วนตัวภายในครอบครัว ไม่ใช่ระบบของโรงพยาบาล
เอกสารนี้เป็นบริบทหลักของโปรเจกต์ อ่านก่อนเริ่มงานทุกครั้ง

## สถานะปัจจุบัน

กำลังทำ **Milestone 1: ระบบ authentication และการจัดการสิทธิ์**
ฟีเจอร์ด้านงานพยาบาลจริง (ตารางเวร, checklist, คำนวณยา) จะเริ่มหลังจาก auth เสร็จ

## Stack

| ส่วน | เลือกใช้ | หมายเหตุ |
|---|---|---|
| Backend | Rust + Axum | async runtime คือ Tokio |
| Database driver | SQLx | เขียน SQL เอง ใช้ compile-time check |
| Database | PostgreSQL 16 | |
| Reverse proxy | Caddy | จัดการ TLS อัตโนมัติ |
| Frontend | ยังไม่ตัดสินใจ | เน้นใช้งานบนมือถือ |
| Deploy | Docker Compose บนเซิร์ฟเวอร์เดียว | |

Cache และ message queue **ยังไม่ใช้** อย่าเพิ่มเข้ามาจนกว่าจะมีปัญหาจริงที่แก้ด้วยวิธีอื่นไม่ได้

## โครงสร้างไดเรกทอรี

```
my-his/
├─ CLAUDE.md                   ไฟล์นี้
├─ docker-compose.yml
├─ .env.example
├─ docs/
│  ├─ rust-guidelines.md       กฎการเขียน Rust ของโปรเจกต์ (อ่านก่อนเขียนโค้ด)
│  ├─ auth-design.md           ดีไซน์ระบบ auth ฉบับละเอียด
│  └─ infrastructure.md        การวาง container, backup, ความปลอดภัยของเครื่อง
├─ ops/                        Caddyfile, สคริปต์ backup/restore
├─ frontend/
└─ backend/
   ├─ Cargo.toml
   ├─ Dockerfile
   ├─ migrations/              ไฟล์ของ sqlx migrate
   ├─ .sqlx/                   query metadata สำหรับ offline build (commit ขึ้น git)
   ├─ tests/                   เทสระดับ API
   └─ src/
      ├─ main.rs               อ่าน config, ต่อ DB, รัน server
      ├─ app.rs                ประกอบ router ของทุกโมดูล + middleware กลาง
      ├─ config.rs             อ่าน environment variable
      ├─ crypto.rs             เข้ารหัสคอลัมน์ข้อมูลผู้ป่วย (AES-256-GCM) + blind index สำหรับค้นหา
      ├─ token.rs              สุ่ม token + SHA-256 (session cookie, ลิงก์ลงทะเบียน) ใช้ร่วมกันหลายโมดูล
      ├─ turnstile.rs          ตรวจ token Cloudflare Turnstile กับ siteverify ตอน login (HTTP ขาออกที่เดียว, reqwest)
      ├─ state.rs              AppState (pool, config)
      ├─ error.rs              AppError และการแปลงเป็น HTTP response
      ├─ db.rs                 สร้าง connection pool, รัน migration ตอนเริ่ม
      ├─ domain/               ชนิดข้อมูลกลาง: UserId, User, Role, Permission
      ├─ auth/                 ดู docs/auth-design.md
      ├─ users/                จัดการบัญชีผู้ใช้/role/เพจ (สิทธิ์ `user.*` / `role.*` / `page.*`) — หน้า "ผู้ใช้งาน" /users กลุ่มเมนูลงทะเบียน
      │                        สร้างบัญชี/รีเซ็ตรหัส = ออกลิงก์ลงทะเบียน (ไม่มีรหัสผ่านชั่วคราว)
      ├─ registration/         ลงทะเบียน/ตั้งรหัสผ่านผ่านลิงก์ (สาธารณะ, หน้า /register#<token>) ผู้ใช้กรอกข้อมูลตัว + ตั้งรหัสเอง
      ├─ budget_year/          ตั้งค่าปีงบประมาณ (หน้า master แรก สิทธิ์ `budget_year.*`)
      ├─ disease/              ตั้งค่าโรค (หน้า master, สิทธิ์ `disease.*`)
      ├─ prename/ nationality/ patient_group/ lab/   master อื่น ๆ โครงเดียวกับ disease/
      ├─ patient/              ลงทะเบียนผู้ป่วย + โรคประจำตัว (สิทธิ์ `patient.*`, เข้ารหัสด้วย crypto.rs)
      ├─ lab_visit/            นัดหมายแล็ป (lab_visit_group + lab_visit) สิทธิ์ `lab_visit.*` — คืนแค่ uuid คนไข้ ไม่แตะข้อมูลระบุตัว
      └─ audit/                บันทึกว่าใครทำอะไร (แต่ละโมดูลเขียน audit_log เอง) + API อ่านประวัติ สิทธิ์ `audit.view`:
                               `GET /audit/auth-events` (login/logout พร้อม IP/User-Agent) และ `GET /audit/activities`
                               (เพิ่ม/แก้ไข/ลบ/ส่งออก ทุกหน้า — แถวผู้ป่วยไม่มีชื่อรายการ)
```

## กฎการเขียนโค้ดที่ต้องยึด

มาตรฐานการเขียน Rust ฉบับเต็มอยู่ใน `docs/rust-guidelines.md` **อ่านก่อนเขียนหรือแก้โค้ดทุกครั้ง**
ข้างล่างนี้คือกฎระดับโครงสร้างที่เฉพาะกับโปรเจกต์นี้

**แบ่งโมดูลตามฟีเจอร์ ไม่ใช่ตามชนิดไฟล์** ทุกอย่างของ auth อยู่ใน `auth/` ไม่มีโฟลเดอร์ `handlers/` หรือ `services/` ที่รวมทุกฟีเจอร์

**ทิศทางการพึ่งพาเป็นทางเดียว** routes → handlers → service → repository ห้ามย้อนกลับ
`domain`, `error`, `state` เรียกใช้ได้จากทุกชั้น

**`service.rs` ห้าม import ชนิดข้อมูลของ Axum** ไม่มี `StatusCode`, `Json`, `HeaderMap`
ชั้นนี้คืนค่าเป็นชนิดของโดเมนหรือ `AppError` แล้วให้ handler แปลงเป็น HTTP
เช่นเดียวกัน `repository.rs` ห้ามรู้เรื่อง HTTP

**แยก DTO ออกจาก struct ของตาราง** แม้ช่วงแรกจะหน้าตาเหมือนกัน เพื่อไม่ให้คอลัมน์ใหม่รั่วออก API โดยไม่ตั้งใจ โดยเฉพาะ `password_hash`

**โมดูลฟีเจอร์ไม่เรียกกันเองข้ามโมดูล** ยกเว้น `auth::CurrentUser` ที่ถือเป็นโครงสร้างพื้นฐาน
ถ้าพบว่าโมดูลหนึ่งต้องเรียก service ของอีกโมดูล ให้ย้ายตรรกะนั้นลง `domain/` แทน

**แต่ละโมดูลเปิดฟังก์ชันเดียวที่คืน `Router` ของตัวเอง** และ `app.rs` เป็นที่เดียวที่รู้จัก router ทั้งหมด

**เวลาเก็บเป็น UTC เสมอ** ใช้ `timestamptz` ในฐานข้อมูล แปลงเป็นเวลาไทยตอนแสดงผลเท่านั้น
เรื่องนี้สำคัญเพราะเวรดึกคร่อมเที่ยงคืน

**ลบข้อมูลแบบ soft delete** ใช้คอลัมน์สถานะ ไม่ลบแถวจริง

## ข้อห้าม

- อย่าแยกเป็น microservice ระบบนี้เป็น backend เดียวโดยตั้งใจ
- อย่าเปลี่ยนไปใช้ JWT เหตุผลอยู่ใน `docs/auth-design.md`
- อย่า publish พอร์ตของ postgres ออกนอกเครื่อง
- อย่า log รหัสผ่าน, session token, หรือค่า cookie ไม่ว่ากรณีใด รวมถึงตอนเกิด error
- อย่าเรียก argon2 ตรง ๆ ใน async context ต้องห่อด้วย `tokio::task::spawn_blocking`
- อย่าเพิ่ม dependency ใหม่โดยไม่ถามก่อน
- อย่าเก็บข้อมูลที่ระบุตัวผู้ป่วยได้ **นอกคอลัมน์ที่ data dict กำหนดและเข้ารหัสไว้** และห้ามเก็บเลขบัตร
  ประชาชน ดูหัวข้อถัดไป
- อย่า log หรือใส่ใน `audit_log.metadata`/error message ซึ่ง plaintext หรือ ciphertext ของข้อมูลผู้ป่วย
  และ key เข้ารหัส (`DATA_ENCRYPTION_KEY`, `BLIND_INDEX_KEY`)
- **อย่า `git commit` จนกว่าผู้ใช้จะบอกให้ commit** แก้ไฟล์/`git add` ได้ตามปกติ แต่ห้ามสั่ง commit เองจนกว่าจะได้รับคำสั่งชัดเจน

## ขอบเขตข้อมูล

ระบบนี้เป็นเครื่องมือช่วยงานส่วนตัว ไม่ใช่เวชระเบียนทางการ แต่ตาราง `patient` ใน
`docs/my-his-data-dict.xlsx` เก็บข้อมูลที่ระบุตัวผู้ป่วยได้ (HN, เลขประจำตัวคนไข้, ชื่อ, สกุล) —
เจ้าของโปรเจกต์ตัดสินใจเก็บแบบนี้ (2026-10-04) โดยมีเงื่อนไขที่ต้องรักษาเสมอ:

- คอลัมน์ที่ data dict ระบุ `encrypt = yes` ต้องเข้ารหัสที่ชั้น app ผ่าน `crypto.rs` **ก่อนลง DB เสมอ**
  (เก็บเป็น `bytea`) — ห้ามเขียน plaintext ลง DB, ห้ามใช้ argon2 กับคอลัมน์เหล่านี้ (ค้นหาไม่ได้/ถอดกลับไม่ได้)
- ค้นหาด้วย HN/เลขประจำตัวคนไข้ใช้ blind index (`*_idx`) เท่านั้น ห้ามถอดรหัสทั้งตารางมา filter ใน Rust
- ห้ามเก็บ **เลขบัตรประชาชน** (national ID) — คอลัมน์สัญชาติคือ `nationality_id` ซึ่งเป็น FK ไป `nationality`
  ไม่ใช่เลขบัตร ถ้ามีคำขอเก็บข้อมูลระบุตัวตนเพิ่มจาก data dict ให้ทักท้วงก่อนลงมือทำ
- key เข้ารหัสอยู่ใน `.env` เท่านั้น และต้องสำรองแยกจาก DB dump (docs/infrastructure.md ข้อ 5)
- API ส่งข้อมูลที่ถอดรหัสแล้วเฉพาะ endpoint ที่จำเป็น และไม่ส่ง id int ภายในออกนอก backend (ใช้ `uuid`)

ส่วนที่เกี่ยวกับการคำนวณทางคลินิก ต้องแสดงสูตรและหน่วยที่ใช้ควบคู่กับผลลัพธ์เสมอ
เพื่อให้ผู้ใช้ตรวจซ้ำได้ด้วยตาก่อนนำไปใช้

## คำสั่งที่ใช้บ่อย

```bash
# ยกฐานข้อมูลสำหรับพัฒนา
docker compose up -d db

# migration
cargo sqlx migrate add <ชื่อ>
cargo sqlx migrate run
cargo sqlx prepare          # อัปเดต .sqlx ก่อน commit ทุกครั้งที่แก้ query

# พัฒนาและทดสอบ
cargo run
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt
```

## การทดสอบ

เทสที่สำคัญที่สุดของ Milestone นี้คือเทสระดับ API ใน `backend/tests/` ที่ยิงผ่าน router จริง
ใช้ `#[sqlx::test]` เพื่อให้แต่ละเทสได้ฐานข้อมูลสะอาดของตัวเอง
กรณีที่ต้องมีเทสครอบคลุม: login สำเร็จ, รหัสผ่านผิดกับผู้ใช้ไม่มีอยู่ต้องได้ผลลัพธ์เหมือนกัน,
session หมดอายุ, logout แล้วใช้ cookie เดิมไม่ได้, ผู้ใช้ที่ไม่มีสิทธิ์ถูกปฏิเสธ

## เอกสารอื่น

- `docs/rust-guidelines.md` — กฎการเขียน Rust: error, async, SQLx, log, เทส, สิ่งที่ยังไม่ต้องทำ
- `docs/auth-design.md` — โมเดล session, ตารางฐานข้อมูล, API, flow, การจัดการสิทธิ์
- `docs/infrastructure.md` — การวาง container, network, backup, การตั้งค่าเครื่อง
