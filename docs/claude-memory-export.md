# Claude memory export: my-his

> ส่งออกเมื่อ 2026-10-31 จาก memory ของ Claude Code (`~/.claude/projects/C--xampp-htdocs-my-his/memory/`)
> และข้อมูลที่เกี่ยวกับ my-his จาก memory ของโปรเจกต์อื่น
> วันที่ในแต่ละบรรทัดคือวันที่ผู้ใช้ให้คำสั่ง ถ้า memory บันทึกไว้ ถ้าไม่มีจะใช้วันที่แก้ memory ครั้งล่าสุด ส่วน `[unknown]` คือไม่ทราบวันที่
> กฎที่อยู่ใน `CLAUDE.md` และ `docs/*.md` แล้วไม่ได้คัดลอกมาซ้ำในไฟล์นี้

## 1. Instructions (คำสั่งที่ผู้ใช้ให้ไว้)

- [2026-10-04] - หลังเพิ่มหรือแก้ตาราง ต้องอัปเดต `docs/my-his-data-dict.xlsx` ให้มี sheet ของ**ทุกตาราง**ที่มีอยู่ ไม่ใช่เฉพาะตารางใหม่ (ผู้ใช้สั่งไว้ชัดเจน)
  - สร้างจาก schema จริง (information_schema + column comment) ไม่พิมพ์เอง
  - รูปแบบ sheet: แถว 1 คือ `table | master/usage/auth | คำอธิบายภาษาไทย` แถว 2 คือ `name | type | comment | key | is null | encrypt | example`
  - ถ้ามีไฟล์ `docs/.~lock.my-his-data-dict.xlsx#` ให้ขอให้ปิด LibreOffice ก่อน
  - อัปเดต `docs/auth-design.md` หัวข้อ 4 ให้ตรงกันด้วย
- [2026-10-04] - เก็บ HN, nap_no, first_name และ last_name ในตาราง `patient` ตาม data dict ได้ โดยมีเงื่อนไข (ผู้ใช้เลือกเอง: "store per spec + encrypt, you edit CLAUDE.md")
  - ต้องเข้ารหัสที่ชั้น app ด้วย AES-256-GCM ใน `crypto.rs`
  - ค้นหาผ่าน blind index เท่านั้น
  - ห้ามเก็บเลขบัตรประชาชน
  - ถ้ามีคำขอเก็บข้อมูลระบุตัวตนนอกเหนือจาก data dict ให้ทักท้วงก่อนลงมือ
- [2026-10-10] - migration ที่เพิ่มหรือย้ายหน้าเมนู (`pages` / `page_permissions`) ให้ "แตะเฉพาะแถวที่เกี่ยวข้อง"
  - ห้ามเอาการแก้เมนูที่ผู้ใช้ทำผ่าน UI ไปฝังไว้ใน migration
  - ห้าม reset label, กลุ่ม หรือ sort_order ที่ผู้ใช้ปรับเอง
  - ขั้นตอน: query แถว `pages` จริงแบบอ่านอย่างเดียว → สำรอง `pages` และ `page_permissions` ไว้ใน scratchpad → dry-run บน `my_his_migtest` → ตรวจโครงเมนู → rebuild backend และ frontend พร้อมกัน
- [2026-10-26] - ถ้า Docker Desktop ปิดอยู่ ต้องถามก่อนเปิด (วันนี้ผู้ใช้อนุญาตให้เปิดได้ครั้งเดียว)

## 2. Identity

- [2026-10-19] - ใช้ภาษาไทยเป็นหลักและคงศัพท์เทคนิคภาษาอังกฤษไว้ ให้ตอบเป็นภาษาไทย
- [unknown] - สร้าง my-his ให้คนในครอบครัวที่เป็นพยาบาลใช้ (อนุมานจากคำอธิบายโปรเจกต์ "เครื่องมือช่วยงานสำหรับพยาบาล ใช้งานส่วนตัวภายในครอบครัว")
- [unknown] - memory ไม่มีข้อมูลชื่อ อายุ หรือการศึกษา

## 3. Career

- [unknown] - memory ของ my-his ไม่มีข้อมูลเรื่องงาน ข้อมูลงานประจำอยู่ในโปรเจกต์อื่นและไม่ได้ส่งออกในไฟล์นี้

## 4. Projects

- [2026-10-04] - my-his: เครื่องมือช่วยงานพยาบาลสำหรับใช้ส่วนตัวภายในครอบครัว ไม่ใช่ระบบของโรงพยาบาล
  - Stack: Rust (Axum, SQLx), PostgreSQL 16, Caddy, Docker Compose บนเซิร์ฟเวอร์เดียว ส่วน frontend ใช้ React + shadcn/ui (Vite) และเน้นใช้งานบนมือถือ
  - การตัดสินใจหลัก: ใช้ session cookie ไม่ใช้ JWT, ยังไม่ใช้ cache หรือ queue, เก็บเวลาเป็น UTC ด้วย `timestamptz` (เวรดึกคร่อมเที่ยงคืน), ลบแบบ soft delete, การคำนวณทางคลินิกต้องแสดงสูตรและหน่วยคู่กับผลลัพธ์
  - [2026-10-04] ข้อมูลผู้ป่วย: เก็บ HN, เลขประจำตัวคนไข้, ชื่อ และสกุล แบบเข้ารหัส AES-256-GCM พร้อม blind index (`*_idx`) สำหรับค้นด้วย `=` ไม่เก็บเลขบัตรประชาชน (`nationality_id` เป็น FK ไปตาราง `nationality`) และเก็บ key ไว้ใน `.env` โดยสำรองแยกจาก DB dump
  - [2026-10-04] รูปแบบ id: ทุกตารางใช้ `id integer identity` เป็น PK ภายใน backend และใช้ `uuid` (default `uuid_v7()`) เป็น id เดียวที่ API, frontend และ audit `target_id` มองเห็น ตาราง users/roles/pages ใช้ uuid เดิมต่อ
  - [2026-10-12] เมนู: กลุ่มเมนูเก็บเป็นแถวใน `pages` แล้ว (`20261012100000_page_ref_id.sql`)
    - หัวกลุ่มมี route เป็น null และหน้าย่อยใช้ `ref_id` ชี้ไปที่หัวกลุ่ม
    - ไม่มี `group_label` และกลุ่ม "Home" อีกแล้ว ส่วน Dashboard เป็นหน้าหลักระดับบนสุด
    - ใน DB จริง `/patients` มี sort_order 2 แต่ seed migration ระบุเป็น 5
  - [2026-10-23] เครื่องที่จะ deploy (เจ้าของโปรเจกต์ยืนยัน)
    - ใช้ Ubuntu + Docker Engine ส่วนเครื่อง dev ตอนนี้คือ Windows + Docker Desktop
    - มี Cloudflare proxy (orange cloud) อยู่หน้า Caddy ตัวนอก IP จริงของ client จึงมาจาก `CF-Connecting-IP` และเชื่อได้เฉพาะเมื่อมาจากช่วง IP ของ Cloudflare (`ops/Caddyfile`)
    - ผู้ใช้ IPv6 ยังเข้าได้ผ่าน Cloudflare โดย login budget นับ IPv6 แบบ /56
    - DNS มีเฉพาะ A record, สิทธิ์ `sql.view` มีแค่ admin, RAM 4 GB
    - ควรตรวจช่วง IP ของ Cloudflare ใน `ops/Caddyfile` ทุกปี
  - สถานะ ณ 2026-10-31
    - ทำแล้ว: auth, role และสิทธิ์หน้า, master data, ลงทะเบียนผู้ป่วย, นัดหมายแล็ป, หน้า SQL แบบอ่านอย่างเดียว, audit log และคลัง (ใบรับเข้าพร้อม ledger `stock_movement` ที่เพิ่มได้อย่างเดียว)
    - ยังเป็นร่าง: เอกสารเบิก นับ และปรับยอด
    - security audit ทำไปแล้ว 2 รอบ รอบสองแก้เสร็จ 2026-10-24
    - งานที่เหลือต้องทำบนเครื่องจริง: Turnstile key จริง, firewall ฝั่ง origin ให้รับเฉพาะ Cloudflare, Full (strict) TLS และตรวจว่าหน้าประวัติ login เห็น IP จริง

## 5. Preferences

- [2026-09-15] - ถ้าผู้ใช้เปลี่ยนใจระหว่างทาง ให้ถือคำสั่งล่าสุดเป็นตัวจริง
- [2026-10-19] - โปรเจกต์ส่วนตัวเลือกใช้ Rust + TypeScript (ไม่ใช้ PHP) และ self-host ด้วย Docker Compose + Caddy + PostgreSQL
- [2026-10-20] - commit, push หรือโพสต์ข้อมูลออกภายนอก ทำเฉพาะเมื่อผู้ใช้สั่ง และคำสั่งให้ commit ไม่ได้รวมถึง push

## 6. บันทึกเชิงเทคนิค (สิ่งที่ Claude เจอเอง ไม่ใช่คำสั่งจากผู้ใช้)

- [2026-10-21] - Bash tool เปลี่ยน `\\` เป็น `\` ใน command และ heredoc แม้จะใช้ `'EOF'` แล้วก็ตาม
  - เคยทำให้ test string `"/\\attacker"` กลายเป็น `"/\attacker"` จน compile ไม่ผ่าน
  - โค้ดหรือ config ที่มี backslash ต้องเขียนด้วย Write หรือ Edit และใน Rust ให้ใช้ raw string `r"..."`
- [2026-10-26] - เครื่องนี้ไม่มี Rust toolchain ต้องรัน cargo fmt, clippy, test และ sqlx prepare ใน container `rust:1.98.1-bookworm`
  - ต้อง mount volume `my-his-rustup:/usr/local/rustup` (ไม่งั้นไม่มี fmt และ clippy), `my-his-cargo-registry` และ `my-his-cargo-tools` (มี sqlx CLI)
  - ใช้ network `my-his_dev` จาก `docker-compose.override.yml` เพราะต้องเข้าถึง db และอินเทอร์เน็ตได้ ห้ามใช้ `my-his_data` ซึ่งเป็น internal network
  - ทดสอบกับฐานข้อมูล `my_his_migtest` เท่านั้น ห้ามใช้ `my_his`
    - mode `clippy` จะรัน `sqlx migrate run` บน `my_his_migtest` ก่อน ใช้เป็น dry-run ของ migration ได้
    - `my_his_migtest` เก่าได้ ต้องสร้างใหม่ด้วยการ dump เฉพาะ schema แล้ว dump ข้อมูลเฉพาะ pages, page_permissions, roles, role_permissions และ `_sqlx_migrations` (ใช้ `--disable-triggers` เพราะ pages มี FK ชี้ตัวเอง)
    - ห้ามมีข้อมูลผู้ป่วยหรือผู้ใช้ในฐานข้อมูลนี้
    - ถ้าแก้ migration ที่รันไปแล้ว checksum จะไม่ตรง ต้องสร้างใหม่
  - RAM เหลือน้อย ต้องรันเทสเป็นชุดเล็กครั้งละ 3–6 binary โดยใช้ `CARGO_BUILD_JOBS=1`
    - ถ้า shell ถูกปิด container ยังรันค้างอยู่ ต้อง `docker stop` เอง
    - ถ้ารันแล้วถูก reaper ฆ่า ห้ามรันซ้ำเองโดยไม่ถาม
  - หลังแก้ `sqlx::query!` ต้องรัน `prepare` แล้วตรวจด้วย offline build (`--network none` และ mount source แบบอ่านอย่างเดียว)
  - เทสที่ต้องคุมลำดับการทำงานใช้ `lock table sessions in share mode` หรือ `for update` ที่แถว user (ดู `tests/credential_races.rs`)
  - frontend
    - ใช้ `./node_modules/.bin/tsc -b` ไม่ใช่ `npx tsc` เพราะ rtk hook พาไปใช้ tsc เวอร์ชันเก่าที่ติดตั้งแบบ global
    - lint ด้วย `npx oxlint`
    - build ลง scratchpad ด้วย `vite build --outDir <scratchpad>/dist-check` เพื่อไม่ให้ทับ `frontend/dist`
  - stack ตอน dev มีแค่ db, backend และ frontend (ไม่มี caddy เพราะพอร์ต 80/443 ชนกับ XAMPP) ตรวจได้ด้วย `sh ops/check-deploy.sh`
  - log ของ backend ต้องอ่านผ่าน path เต็มของ `docker.exe` เพราะ rtk hook แก้ output ของ `docker logs` และ `grep`
  - Python ที่รันด้วย `-I` ต้องเรียก `sys.stdout.reconfigure(encoding="utf-8")` ถึงจะพิมพ์ภาษาไทยได้ (มี openpyxl 3.1.5 ไว้แก้ data dict)
