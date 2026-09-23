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
      ├─ state.rs              AppState (pool, config)
      ├─ error.rs              AppError และการแปลงเป็น HTTP response
      ├─ db.rs                 สร้าง connection pool, รัน migration ตอนเริ่ม
      ├─ domain/               ชนิดข้อมูลกลาง: UserId, User, Role, Permission
      ├─ auth/                 ดู docs/auth-design.md
      ├─ users/                จัดการบัญชีผู้ใช้ (เฉพาะ admin)
      └─ audit/                บันทึกว่าใครทำอะไร
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
- อย่าเก็บข้อมูลที่ระบุตัวผู้ป่วยได้ (ชื่อจริง, HN, เลขบัตรประชาชน) ดูหัวข้อถัดไป
- **อย่า `git commit` จนกว่าผู้ใช้จะบอกให้ commit** แก้ไฟล์/`git add` ได้ตามปกติ แต่ห้ามสั่ง commit เองจนกว่าจะได้รับคำสั่งชัดเจน

## ขอบเขตข้อมูล

ระบบนี้ออกแบบให้เป็นเครื่องมือช่วยงานส่วนตัว ไม่ใช่เวชระเบียน
ถ้าฟีเจอร์ใดต้องอ้างถึงคนไข้ ให้ใช้รหัสที่ผู้ใช้ตั้งเอง เช่น "เตียง 7" เท่านั้น
ถ้ามีคำขอที่ดูเหมือนจะเก็บข้อมูลระบุตัวผู้ป่วย ให้ทักท้วงก่อนลงมือทำ

ส่วนฟีเจอร์ที่เกี่ยวกับการคำนวณทางคลินิก ต้องแสดงสูตรและหน่วยที่ใช้ควบคู่กับผลลัพธ์เสมอ
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
