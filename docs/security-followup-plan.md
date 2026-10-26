# แผน: ปิด ‘ข้อควรรู้’ หลังแก้ผล security audit รอบที่ 2

> **สถานะ:** ข้อ 1–3 ยังไม่เริ่มทำ — รอเจ้าของสั่ง; ข้อ 3 แตะ DB `my_his` จริง (สำรองก่อนเสมอ)
> **ข้อ 5 (2026-10-26):** commit 1–4 ทำแล้วตามตาราง (backend, compose/Caddy, SPA security, docs) ยังไม่ push;
> commit 5 (UI วันที่ไทย/ค้นหาสถานะ) และ `docs/inventory-design.md` ยังไม่ commit — รอเจ้าของสั่ง
> ผล audit และสถานะการแก้อยู่ใน `docs/security-audit.md`

## Context

งานแก้ 4 กลุ่ม (A Cloudflare+IP, B compose, C เพดาน backend, D confirmed low) เสร็จในโค้ด/เอกสารแล้ว และผ่าน fmt/clippy/เทสทุกไฟล์/build offline/Caddy จริง/compose ทดลอง
ยังไม่ commit ไม่ deploy ข้อควรรู้ที่เหลือ:

| ข้อควรรู้ | ทำอะไรได้ในรอบนี้ |
|---|---|
| migration `…100300` ยังไม่ถูกเทสกับแถว audit รูปแบบเก่า | **เขียนเทสถาวร** (ข้อ 1) |
| stack dev ที่รันอยู่ยังเป็นโครงเดิม | **ใช้กับ dev stack** พร้อม pg_dump สำรอง (ข้อ 3) — เจ้าของอนุมัติแล้ว |
| งานฝั่งเครื่องจริง (Cloudflare/firewall/IP จริง) ทดสอบจากเครื่องนี้ไม่ได้ | **เครื่องมือตรวจแบบอ่านอย่างเดียว** 2 ตัว + ไฟล์ตัวอย่าง override (ข้อ 2) — เจ้าของอนุมัติแล้ว |
| N3 เซลล์เดียวขนาดมากถูก buffer ได้หนึ่งครั้ง | **ยอมรับความเสี่ยง** (ไม่มีทางแก้ที่ถูกต้องโดยไม่เพิ่ม dependency) บันทึกไว้แล้ว — ข้อ 4 |
| `docker-compose.override.yml` แก้เฉพาะเครื่องนี้ (git-ignore) | ตัวอย่างที่ขึ้น git (ข้อ 2) |
| working tree ปนไฟล์ที่ไม่ใช่งานนี้ (UI วันที่ไทย/ค้นหาสถานะ, `docs/inventory-design.md`) | **แผน commit แยก รอคำสั่ง** (ข้อ 5) |
| Docker Desktop ที่สตาร์ทไว้ | คงเปิดไว้ (dev stack ใช้) — แจ้งในรายงาน |

กฎเดิมที่ยึด: **ไม่ commit/ไม่ push จนกว่าจะสั่ง**, ไม่เพิ่ม dependency, ไม่อ่าน `.env` (compose/Docker อ่านเอง; สคริปต์ที่เขียนพิมพ์ ‘ชื่อ’ ตัวแปรเท่านั้น ไม่พิมพ์ค่า),
ไม่แตะ DB `my_his` ยกเว้นข้อ 3 ที่เจ้าของอนุมัติ (backup ก่อน), ซิงก์เอกสาร/data dict, รายงานเป็นภาษาไทย

## 1. เทสถาวรของ migration `20261021100300` (เล็ก ทำก่อน)

- ไฟล์ใหม่ `backend/tests/audit_cleanup_migration.rs` ต่อยอดจากเทสที่ audit รอบสองเตรียมไว้ (`…\run-2\agents\h10-migrations\scratch\h10_audit.rs` ฟังก์ชันแรก
  `cleanup_migration_touches_only_unknown_name_login_failed_rows`) ใช้ `common::run_migration_files(&pool, |v| v < CLEANUP)` สร้างแถวรูปแบบเก่า (ชื่อดิบ, SHA-256,
  NULL อยู่แล้ว, login_failed ของบัญชีจริง, login_succeeded, แถวของ action อื่น) → รัน `== CLEANUP` → ยืนยันว่าแตะเฉพาะ `login_failed` ที่ `actor_user_id is null and target_id is not null`,
  `metadata`/`ip` ไม่ถูกแตะ และรันซ้ำแล้วผลเท่าเดิม (`#[sqlx::test(migrations = false)]`)
- ไม่ใช้ส่วนที่สองของไฟล์ h10 (ผูกกับ keyword แบบเก่า) — เรื่อง keyword มีเทสแล้วใน `sql_console_limits.rs`
- แก้คอมเมนต์ที่ล้าสมัยในหัวไฟล์ `…100100` (ยังเขียนว่า ‘รอเจ้าของอนุมัติ’ ทั้งที่ `…100300` ทำแล้ว) — **แก้ไฟล์ migration ที่ยังไม่เคยรันบน `my_his` ได้**; ถ้ามีเครื่องที่รัน migration นั้นไปแล้ว
  checksum ของ sqlx จะไม่ตรง → ตรวจ `_sqlx_migrations` บน `my_his_migtest` ก่อนแก้ ถ้า checksum ต่างให้ **ไม่แก้ไฟล์** แต่ย้ายคำอธิบายไปไว้ใน `docs/auth-design.md` แทน

## 2. เครื่องมือตรวจฝั่งเครื่องจริง (อ่านอย่างเดียว ไม่มี dependency ใหม่)

POSIX `sh` (รันบน Ubuntu และ Git Bash) ไม่มี bashism, `set -eu`, ไม่พิมพ์ค่าใน `.env`, ไม่แก้อะไร, ทุกข้อพิมพ์ `OK` / `FAIL` / `WARN` / `SKIP` + exit code ≠ 0 เมื่อมี FAIL

**`ops/check-deploy.sh [project]`** (ปริยาย project = ชื่อโฟลเดอร์; ข้าม service ที่ไม่ได้รัน เช่น caddy บน dev ด้วย `SKIP`):
1. `backend` และ `db` สถานะ `healthy`
2. พอร์ตที่ publish มีเฉพาะ caddy 80/443 (ใช้ `docker ps --format '{{.Names}} {{.Ports}}'`; dev override ที่ publish 127.0.0.1 → `WARN` ไม่ใช่ FAIL)
3. network: caddy ไม่อยู่ network เดียวกับ db หรือ backend (`docker inspect` → คำนวณส่วนร่วมใน shell)
4. ชื่อตัวแปรของ `db` ไม่มี `DATA_ENCRYPTION_KEY|BLIND_INDEX_KEY|DATABASE_URL|SQL_CONSOLE_DB_PASSWORD|TURNSTILE_SECRET_KEY|INITIAL_ADMIN_PASSWORD`; ของ `caddy` มีแค่ `DOMAIN` + ตัวแปรของ image (ใช้ `docker exec … env | cut -d= -f1`)
5. limit: `Memory`/`PidsLimit` ของ backend/caddy/frontend ไม่เป็น 0
6. log ของ backend (อ่านดิบ): ‘Turnstile uses Cloudflare test keys’ / ‘Turnstile disabled’ → `WARN`; ‘non-public client address’ → **FAIL** (proxy chain ผิด); ‘table is full’ → `WARN`
7. ถ้ามี `DOMAIN` ที่ไม่ใช่ localhost (อ่านด้วย `docker exec caddy printenv DOMAIN` — ชื่อโดเมนไม่ใช่ความลับ): `dig +short AAAA`, มี header `cf-ray` ไหม (Cloudflare proxy ทำงาน), `Cache-Control: no-store` + `Content-Security-Policy` ของ `/api/auth/login-config`
8. พิมพ์ ‘ขั้นตอนมือ’ ที่สคริปต์ทำแทนไม่ได้: firewall 80/443 เฉพาะ Cloudflare, SSL Full (strict), ล็อกอินจริงทั้ง IPv4/IPv6 แล้วดู IP ในหน้าประวัติ

**`ops/check-cloudflare-ips.sh`**: ดึง `https://www.cloudflare.com/ips-v4` และ `/ips-v6` (curl) เทียบกับรายการหลัง `trusted_proxies static` ใน `ops/Caddyfile` (sort + diff) → `OK` ถ้าตรง, ไม่งั้นแสดงรายการที่เพิ่ม/หายและ exit 1 — ใช้ก่อน deploy/ทุกปี; ดึงไม่ได้ = `SKIP` (ไม่ใช่ FAIL)

**`docker-compose.override.example.yml`** (ขึ้น git): เนื้อหาเดียวกับ override ของเครื่องนี้ (publish 127.0.0.1 ของ db/frontend/backend + network `dev` สำหรับ db) พร้อมคอมเมนต์ ‘คัดลอกเป็น `docker-compose.override.yml`’ — ตรวจว่าชื่อนี้ไม่ถูก `.gitignore` กิน

**เอกสาร:** `docs/infrastructure.md` (checklist ก่อนขึ้นเครื่องจริงอ้างสคริปต์ทั้งสอง + วิธีใช้ override example), `CLAUDE.md` (ผัง `ops/` เพิ่มสองบรรทัด)

**ทดสอบสคริปต์:** `sh -n` ทั้งสอง; รัน `check-deploy.sh` กับ dev stack (หลังข้อ 3) ควรเห็น caddy `SKIP`, db/backend healthy, FAIL 0; รัน `check-cloudflare-ips.sh` กับ Caddyfile ปัจจุบันควร `OK` (ดึงไว้ 2026-10-23); ทดลองทำให้ FAIL แบบควบคุมได้ (สำเนา Caddyfile ใน scratchpad ลบ CIDR หนึ่งบรรทัด) เพื่อยืนยันว่าตรวจเจอ

## 3. ใช้กับ dev stack (`my-his-*`) — ต้องทำตามลำดับ ห้ามข้าม

ผลกระทบที่เจ้าของรับทราบ: backend ใหม่รัน migration 7 ไฟล์กับ `my_his` จริงตอนเริ่ม (รวม `…100300` ล้าง `audit_log.target_id` ของ `login_failed` ที่ไม่มีผู้กระทำ ~23 แถว — อนุมัติไว้ 2026-10-21)

1. **ตรวจก่อน:** `docker compose config --quiet` (เงียบ ไม่พิมพ์ค่า); `df` ดิสก์ว่างพอ; ยืนยัน `my-his-db-1` healthy
2. **สำรอง:** `docker exec my-his-db-1 sh -c 'pg_dump -U "$POSTGRES_USER" -Fc my_his'` > `C:\Users\PHP012025\backups\my_his-<วันที่>.dump` (นอก repo; ตัวแปรอ่านจากภายใน container ไม่ผ่าน shell host) →
   `pg_restore --list` นับ object ได้ > 0; **ไฟล์นี้มีข้อมูลผู้ป่วยที่เข้ารหัส + audit จริง ห้าม commit/ส่งต่อ** บอกที่เก็บในรายงาน ให้เจ้าของลบเองเมื่อมั่นใจ
3. **ติดป้าย image เดิม:** `docker tag` image ของ backend/frontend เป็น `:pre-round4` (ไว้ย้อนกลับ)
4. **สร้างใหม่เฉพาะที่ dev ใช้:** `docker compose up -d --build db backend frontend` (**ไม่รวม caddy** — พอร์ต 80/443 ชนกับ XAMPP; dev เปิดผ่าน frontend `127.0.0.1:3000`) แล้ว `docker network rm my-his_default` (เมื่อไม่มี container ใช้)
5. **ตรวจหลังทำ:** (ก) backend/db `healthy` (ข) log ดิบมี ‘Applied 2026102…’ ครบ 7 ไฟล์ ไม่มี error (ค) `psql` ใน container db: `select count(*) from _sqlx_migrations` เพิ่มขึ้น 7; `select count(*) from audit_log where action='login_failed' and actor_user_id is null and target_id is not null` = 0;
   `\d registration_links` มี `purpose`; index `sessions_*_expires_at_idx` มี (ง) `curl http://127.0.0.1:3000/` 200 และ `/api/auth/login-config` 200 (จ) `ops/check-deploy.sh` ไม่มี FAIL (ฉ) ให้เจ้าของล็อกอินจริงบนหน้าเว็บ + เปิดเมนู + หน้า SQL/ประวัติ (ผมไม่รู้รหัสและไม่อ่าน `.env`)
6. **ย้อนกลับ:** schema ย้อนไม่ได้ (migration เดินหน้าอย่างเดียว) — ถ้าข้อมูลผิดปกติ: หยุด backend → `pg_restore --clean --if-exists` จากไฟล์สำรอง → ใช้ image `:pre-round4`; ถ้าแค่ container ผิดปกติ แก้ไปข้างหน้า (compose ใหม่เป็น superset ของเดิม)
7. หลังผ่าน: อัปเดต memory `backend-checks-in-docker` (ยืนยัน `--network my-his_dev` ใช้ได้จริง) และ `docs/security-audit.md` ข้อ 6/7 ใส่ ‘ใช้กับ dev stack แล้ว <วันที่>’

## 4. ความเสี่ยงที่ยอมรับ (ไม่แก้โค้ด)

- **N3 เซลล์เดียวใหญ่มาก:** sqlx อ่านทั้ง DataRow เข้าหน่วยความจำก่อนคืนให้เรา เซลล์เดียวถึง 1 GB ในทางทฤษฎีภายใน `statement_timeout` 5 วินาทีถูก buffer ได้หนึ่งครั้งก่อนถูกปฏิเสธ —
  แก้ให้เด็ดขาดต้องเขียน/ครอบ protocol ของ Postgres เอง (หรือเพิ่ม dependency) ซึ่งเกินสัดส่วนกับความเสี่ยง (ผู้ถือ `sql.view` = admin เท่านั้น, backend มี `mem_limit: 1g` + `restart: unless-stopped`) —
  บันทึกแล้วใน `docs/infrastructure.md` และ `docs/security-audit.md` ข้อ 8; **เงื่อนไขทบทวน:** ถ้าแจก `sql.view` ให้คนอื่นนอก admin
- ที่เหลือที่ต้องทำบนเครื่องจริงมีแค่รายการใน `docs/security-audit.md` ข้อ 5 (ข้อ 3, 4) — ตรวจด้วยสคริปต์ข้อ 2

## 5. แผน commit (ทำเมื่อเจ้าของสั่งเท่านั้น)

working tree: ~120 path (backend 60+, `.sqlx` 18, frontend 32 + ใหม่ 2, docs 5, compose/ops/Dockerfile ~8) — ปนงานที่ไม่ใช่ security: ชุด UI วันที่ไทย/ค้นหาสถานะ
(`frontend/src/lib/{date,search}.ts`, `components/layout/SearchForm.tsx`, `components/ui/thai-date-input.tsx`, `BudgetYear*`, `Diseases*`, `Master*`, `configs.tsx`, `types.ts`, `Activity/AuthHistoryPage`,
`LabVisitGroupFormDialog`, `SqlConsolePage`/`SchemaPanel` บางส่วน ฯลฯ) และ `docs/inventory-design.md` (ไม่ใช่ของผม → ไม่ stage เว้นแต่เจ้าของสั่ง)

กฎ: **แต่ละ commit ต้อง build ได้ด้วยตัวเอง** — โค้ดที่ใช้ `sqlx::query!` ต้องไปพร้อม `.sqlx` และ migration ของมัน จึง **ไม่แยก backend เป็นหลาย commit** (ไฟล์ซ้อนกัน เช่น `auth/service.rs` มีทั้งรอบ 1–4)

| # | commit | เนื้อหา |
|---|---|---|
| 1 | `fix(security): harden auth, SQL console, registration and page limits` | `backend/**` ทั้งหมด (src + tests + migrations 7 + `.sqlx`) + `Cargo.toml` |
| 2 | `fix(security): segment compose networks, trust Cloudflare client IP, add deploy checks` | `docker-compose.yml`, `docker-compose.override.example.yml`, `ops/Caddyfile`, `ops/check-*.sh`, `frontend/Caddyfile`, `frontend/Dockerfile`, `backend/Dockerfile`, `.dockerignore` x2, `.env.example` |
| 3 | `fix(security): safe path segments and limits in the SPA` | `frontend/src/lib/{api,uuid}.ts`, `*Api.ts`, `features/auth/{menu,loginError}.ts`, `PageFormDialog.tsx`, `sqlConsoleApi.ts`, `LabVisit*` (เฉพาะส่วน isUuid/seg) |
| 4 | `docs: security audit rounds 1-2 results and design updates` | `docs/{security-audit,auth-design,infrastructure}.md`, `docs/my-his-data-dict.xlsx`, `CLAUDE.md` |
| 5 (ถ้าเจ้าของสั่ง) | `feat(ui): Thai date input and status search` | ไฟล์ UI ที่ไม่ใช่ security ข้างบน |

ขั้นตอน (ก่อนสั่ง commit จริง): (1) จำแนกไฟล์ frontend ทีละไฟล์จาก `git diff` (กลุ่ม 3 vs 5) ไฟล์ที่ปนกันแยกระดับ hunk ด้วย patch + `git apply --cached` (ไม่ใช้ `git add -p` เพราะ interactive ไม่ได้) หรือถามเจ้าของว่าให้ลง commit ไหน
(2) เสนอตารางไฟล์ต่อ commit ให้เจ้าของดูก่อน stage (3) หลัง stage แต่ละ commit ตรวจว่า build ได้ใน worktree ชั่วคราว (`git worktree add`, backend `cargo check --offline`, frontend `bun run build`) แล้วลบ worktree
(4) ข้อความ commit ภาษาอังกฤษแบบ conventional + ท้ายข้อความ `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (5) **ไม่ push**

## ลำดับทำ และการตรวจ

ลำดับ: **1 → 2 → 3 → 4(เอกสาร) → รายงาน** (ข้อ 5 รอคำสั่ง) — ข้อ 3 ทำหลังข้อ 2 เพื่อให้ `check-deploy.sh` ใช้ยืนยันผลได้

ตรวจ:
- ข้อ 1: เทสใหม่ผ่านใน `run_lt.sh test "--test audit_cleanup_migration"`; เทสชุดเดิมที่เกี่ยวกับ audit (`audit`, `login_throttle`) ยังผ่าน; `cargo fmt` + `clippy -D warnings` สะอาด
- ข้อ 2: `sh -n` ผ่าน; รันกับ dev stack ผลตามที่คาด; ทดลอง FAIL แบบควบคุมได้; สคริปต์ไม่พิมพ์ค่าใด ๆ จาก `.env` (ตรวจ output ด้วยตา + ค้นรูปแบบ key/password ในผลลัพธ์)
- ข้อ 3: ตามรายการตรวจหลังทำ (5) ทุกข้อ; `docker ps` แสดง 3 container ใหม่ healthy; ไม่มี container `satrial*`/`sa-*` ค้าง
- เอกสารซิงก์: ตัวเลข/ชื่อไฟล์ตรงกัน; `git status` ไม่มีไฟล์แปลกปลอม (สำเนา dump/สคริปต์ทดลองอยู่นอก repo)
- รายงานผลเป็นภาษาไทย: สิ่งที่ทำ, ผลตรวจ, ที่เก็บไฟล์สำรอง, ความเสี่ยงที่ยอมรับ, สถานะ commit (ยังไม่ทำ), งานเจ้าของที่เหลือ
