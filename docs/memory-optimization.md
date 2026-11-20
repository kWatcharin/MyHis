# การใช้หน่วยความจำ (RAM) ของ backend — ผลสแกนและข้อเสนอ

สแกนเมื่อ 2026-11-15 (อ่านโค้ด ไม่ได้รัน/วัดจริง) ครอบคลุม `backend/src` ทั้งหมด ~22k บรรทัด
ทุกข้อในเอกสารนี้อ้าง `ไฟล์:บรรทัด` ของโค้ด ณ วันที่สแกน — ถ้าโค้ดขยับ ให้ตรวจบรรทัดใหม่ก่อนลงมือ

**ข้อสรุป:** โค้ดของ backend จำกัดขอบเขตหน่วยความจำไว้ดีอยู่แล้ว (body limit, semaphore argon2, เพดานผลหน้า SQL,
เพดาน audit, budget map มี cap) ไม่มี cache ที่โตไม่จำกัด — **แต่ allocator ของ glibc ไม่ได้คืนหน่วยความจำของ argon2**:
วัดกับ backend ตัวจริง (2026-11-18, ข้อ 3.1) RSS ขึ้นจาก 27 MiB เป็น ~276 MiB ตั้งแต่ login 40 ครั้งแรกแล้วค้างอยู่อย่างนั้น
(โปรแกรมจำลองที่โหลดต่างกันค้างถึง ~970 MiB ซึ่งชนเพดาน container 1 GiB) → แก้แล้วใน `backend/Dockerfile`: เหลือ ~11 MiB
ที่เหลือ (3.2, หัวข้อ 5) แก้แล้วในรอบเดียวกัน เป็นการเปลี่ยนเล็กที่ไม่ทำให้แย่ลงได้ หัวข้อ 4: วัดหน้า SQL แล้วไม่ต้องรื้อ (4.1),
ไม่ทำ pagination (4.2), จำกัด connection ระหว่าง Caddy กับ backend แล้ว (4.3)

### สถานะ (อัปเดต 2026-11-18, branch `perf/memory-optimization`)
| ข้อ | สถานะ |
|---|---|
| 3.1 glibc + argon2 | **แก้แล้ว** — `ENV MALLOC_MMAP_THRESHOLD_=1048576` ใน `backend/Dockerfile` (วัดทั้งโปรแกรมจำลองและ backend ตัวจริง ดูข้อ 3.1) |
| 3.2 หน้า SQL ปล่อย `PgRow` | แก้แล้ว |
| 4.1 แปลงผล SQL ระหว่าง stream | **วัดแล้ว ไม่ทำ** — peak หน้า SQL +132 MiB ที่ 4 คำขอใกล้เพดาน ต่ำกว่าเกณฑ์ |
| 4.2 pagination | **ไม่ทำ** — ขัดกับการค้นหาชื่อผู้ป่วยที่เข้ารหัส, ข้อมูลจริงยังเล็ก (ผู้ป่วย 38 แถว) มีเกณฑ์ทบทวน |
| 4.3 จำกัด connection | **แก้แล้ว** — backend 128 + Caddy `max_conns_per_host 64` × 2 |
| 5.1–5.9 | แก้แล้วทุกข้อ (5.8 ตามตารางในข้อนั้น) |

---

## 1. ภาพรวม

| | |
|---|---|
| ประเภท | web service เดียว: Axum 0.8 + Tokio multi-thread + SQLx/PostgreSQL 16 |
| ขนาดการใช้งาน | ระดับครอบครัว — ข้อมูลหลักร้อยถึงหลักพันแถวต่อตาราง |
| runtime image | `debian:bookworm-slim` → allocator คือ **glibc malloc** (ไม่มี jemalloc/mimalloc) |
| เพดาน container | `mem_limit: 1g`, `pids_limit: 1024` (`docker-compose.yml`) — เครื่องจริง RAM 4 GB |
| Tokio | `#[tokio::main]` ค่าปริยาย: worker = จำนวน core, blocking pool ≤ 512 thread (ไม่ได้ตั้ง `max_blocking_threads`) |
| `[profile.release]` | ไม่มี (ใช้ค่าปริยายของ cargo) |

---

## 2. สิ่งที่ดีอยู่แล้ว (ไม่ต้องแตะ)

- **body ขาเข้า** ≤ 256 KiB (`app.rs:23`), อ่าน body timeout 3 s (`app.rs:28`), request timeout 10 s
- **argon2** m = 19 MiB, t = 2, p = 1 (`password.rs:23-25`) + semaphore 4 slot (login ใช้ได้ 3) + คิว login 64 (`password.rs:27-33`)
  → working set argon2 สูงสุด 4 × 19 = **76 MiB** (ถ้าไม่มี semaphore: blocking pool 512 × 19 MiB ≈ 9.5 GiB)
- **หน้า SQL** อ่านแบบ stream (`sql_console/repository.rs:120` `.fetch(...)`) พร้อมเพดาน wire 16 MiB (`repository.rs:123`),
  JSON budget 16 MiB (`service.rs:519`), ≤ 500 แถว (wrap `limit` ฝั่ง server, `service.rs:251`), ≤ 200 คอลัมน์,
  role DB จำกัด 4 connection (`setup.rs:58`)
- **audit** ≤ 5,000 แถว ช่วงวันที่ ≤ 366 วัน (`audit/service.rs:13-14`)
- **budget ของ login** — `ip_budget.rs:26` และ `user_budget.rs:25` cap 10,000 รายการ (< 1 MB), `audit_budget` ขนาดคงที่
- **`AppState`** (`state.rs:13`) ทุก field เป็น `Arc`/`PgPool` → axum clone ต่อ request = เพิ่ม refcount ~8 ครั้ง ไม่มี deep clone
- **`reqwest::Client`** สร้างครั้งเดียว (`turnstile.rs:67`), pool DB = 10 (`db.rs:11`)
- ไม่มี `include_str!`/`include_bytes!`, const array ทุกตัวเล็ก (`sql_console/embedded.rs` ไม่ได้ฝังข้อมูล — ชื่อหมายถึง ciphertext ที่ฝังอยู่ในผลลัพธ์ SQL)
- request ที่มี cap จำนวนรายการฝั่ง service แล้ว: ใบรับเข้า ≤ 100 บรรทัด, หน่วยแปลง ≤ 10, role ต่อผู้ใช้ ≤ 100,
  ผู้ป่วยต่อคำขอแล็ป ≤ 500, โรคต่อผู้ป่วย ≤ 100, เรียงลำดับเพจ ≤ 500 (ยกเว้นรายการ permission — ดูข้อ 5.9)

### ขอบบนเชิงทฤษฎี (คำนวณจากค่าคงที่ — ไม่ได้วัด)

| ส่วน | สูตร | ขอบบน |
|---|---|---|
| argon2 | 4 slot × 19 MiB | 76 MiB |
| body ที่ buffer | 512 connection (`conn_guard.rs:33`) × 256 KiB | 128 MiB |
| ผลหน้า SQL | 4 คำขอ × (16 MiB wire + 16 MiB JSON + body ที่ serialize) | ~130–200 MiB |
| map ของ budget | 2 × 10,000 รายการ | < 1 MiB |
| **รวม** | + pool, runtime, stack ของ thread | **ต่ำกว่า 1 GiB** |

---

## 3. High impact + Low effort (ทำก่อน — แต่วัดก่อน/หลังทุกครั้ง)

### 3.1 glibc เก็บหน่วยความจำของ argon2 ค้างไว้ใน RSS (วัดแล้ว — ยืนยัน, แก้แล้ว)

**ผลวัด (2026-11-18)** — โปรแกรมจำลอง `password.rs` (argon2 0.5.3, m = 19 MiB, t = 2, p = 1, semaphore 4, `spawn_blocking`)
build `--release` รันใน `rust:1.98.1-bookworm` (glibc 2.36 ตัวเดียวกับ runtime `debian:bookworm-slim`) verify ทีละรอบ 40 ครั้ง:

| ช่วง | ค่าปริยาย VmRSS | `MALLOC_ARENA_MAX=2` VmRSS | `MALLOC_MMAP_THRESHOLD_=1048576` VmRSS / VmHWM |
|---|---|---|---|
| หลัง hash ครั้งแรก | 3 MiB | 1 MiB | 1 MiB / 20 MiB |
| หลัง verify 40 ครั้ง | **763 MiB** | 742 MiB | 3 MiB / 79 MiB |
| idle 15 วินาที (blocking thread ตายแล้ว) | 763 MiB | 742 MiB | 1.4 MiB / 79 MiB |
| รอบที่ 2 (80 ครั้ง) | 915 MiB | 363 MiB (HWM 838) | 1.4 MiB / 79 MiB |
| รอบที่ 3–8 (120–320 ครั้ง) | **ค้างที่ ~972 MiB (HWM 1009)** | — | 2.9 MiB / 79 MiB |

**ผลวัดกับ backend ตัวจริง (2026-11-18)** — image เดิม (build 2026-10-28) เทียบ image ใหม่ที่มี ENV นี้ รันชี้ไปที่ `my_his_migtest`
ปิด Turnstile ยิง login ด้วยชื่อที่ไม่มีอยู่จริง (ยังรัน argon2 กับ dummy hash — ทุกคำขอได้ 401) พร้อมกันทีละ 4, `X-Forwarded-For` ไม่ซ้ำกัน:

| ช่วง | image เดิม VmRSS / VmHWM | image ใหม่ VmRSS / VmHWM |
|---|---|---|
| หลังเริ่ม (มี dummy hash ตอนเริ่ม 1 ครั้ง) | 26.8 / 26.8 MiB | 8.3 / 23.5 MiB |
| login 40 ครั้ง | **275.8** / 294.8 MiB | 10.0 / 67.0 MiB |
| login 120 ครั้ง | 275.8 / 294.8 MiB | 11.2 / 68.2 MiB |
| login 240 ครั้ง | 276.3 / 295.3 MiB | 11.2 / 68.2 MiB |
| idle 30 วินาที | 276.6 / 295.3 MiB | 11.2 / 68.2 MiB |

(peak ของ image ใหม่ ~68 MiB = login ใช้ argon2 ได้ 3 slot × 19 MiB + ฐาน — ตรงกับที่ออกแบบไว้)

สรุป: ค่าปริยายไม่คืนก้อน 19 MiB ระดับที่ค้างขึ้นกับรูปแบบโหลด — backend จริงค้าง ~276 MiB, โปรแกรมจำลองค้างถึง ~970 MiB
(ชน `mem_limit: 1g`) จึงมีความเสี่ยงถูก OOM kill ในบางรูปแบบโหลด `MALLOC_ARENA_MAX` อย่างเดียวไม่พอ ส่วนตรึง mmap threshold
ทำให้ peak เท่ากับ working set จริง (4 × 19 MiB) และคืน OS หมดหลังใช้ — ตั้งทั้งสองค่าได้ผลเท่ากับตรึง threshold อย่างเดียว
จึงใช้แค่ `MALLOC_MMAP_THRESHOLD_` (ใส่เป็น `ENV` ใน stage runtime ของ `backend/Dockerfile` ให้ติดไปกับ image ไม่ต้องพึ่ง compose)

ข้อความเดิมก่อนวัด (เก็บไว้เป็นบันทึกเหตุผล):

**หลักฐาน**
- argon2 0.5.3 `src/lib.rs:230`: `let mut blocks = vec![Block::default(); self.params.block_count()];`
  → จองก้อน ~19 MiB ใหม่ทุกครั้งที่ hash/verify บน thread ของ `spawn_blocking` (`password.rs:82-91`)
- runtime เป็น glibc และไม่มี `MALLOC_*` ตั้งไว้ที่ใดเลย (ไม่พบใน `docker-compose*.yml`, `Dockerfile`, `docs/`)

**กลไก**
glibc ปรับ mmap threshold แบบ dynamic: เมื่อ free ก้อนที่ได้มาจาก mmap (19 MiB) threshold จะขยับขึ้นเท่าขนาดนั้น
(สูงสุด 32 MiB บน 64-bit) ก้อน 19 MiB ครั้งถัดไปจึงถูกจองจาก arena ของ thread แทน mmap และหลัง free อาจยังค้างใน RSS
เพราะ glibc ไม่ trim กลาง heap ส่วน arena จะไม่ถูกคืนแม้ blocking thread ตายไปแล้ว
→ RSS อาจค้างที่ ~19 MiB × จำนวน arena ที่เคยรัน argon2 (สูงสุดราว 76 MiB) ทั้งที่ไม่มี login อยู่เลย

**ทางแก้** — ไม่แตะโค้ด ไม่เพิ่ม dependency แค่ตั้ง env ใน service `backend` ของ `docker-compose.yml`

```yaml
# ก่อน: ไม่มี

# หลัง
    environment:
      # ตายตัว 1 MiB → ปิด dynamic threshold: ก้อน ≥ 1 MiB (argon2) ใช้ mmap และคืน OS ทันทีที่ free เสมอ
      MALLOC_MMAP_THRESHOLD_: "1048576"
      # จำกัดจำนวน arena (ค่าปริยาย 8 × core) ลด fragmentation จาก thread จำนวนมาก
      MALLOC_ARENA_MAX: "2"
```

ข้อแลกเปลี่ยน: mmap/munmap ทุกครั้งที่ hash มีต้นทุน syscall + page fault ราว 4,900 หน้า (เล็กน้อยเมื่อเทียบกับเวลา argon2 เอง)
`MALLOC_ARENA_MAX` ต่ำอาจเพิ่ม lock contention ของ malloc เล็กน้อย — ระดับโหลดของระบบนี้ไม่น่ามีผล

**วัด**
1. ยก stack dev, รอ idle → `docker exec <backend> cat /proc/1/status | grep -E 'VmRSS|VmHWM'`
2. login ต่อเนื่อง ~20 ครั้ง (บางครั้งพร้อมกัน 3–4 คำขอ) → บันทึกอีกครั้ง
3. idle 1–2 นาที → บันทึก `VmRSS` อีกครั้ง
4. ทำซ้ำหลังตั้ง env — **ถ้า `VmRSS` หลัง idle ไม่ต่างกัน ให้ตัดข้อนี้ทิ้ง**

**impact ที่คาด (ก่อนวัด — ประเมินต่ำไปมาก):** RSS steady state ลดลงได้สูงสุดหลายสิบ MiB, peak (`VmHWM`) ไม่เปลี่ยน
**impact จริง (backend ตัวจริง):** RSS หลังใช้งานลดจาก ~276 MiB เหลือ ~11 MiB, peak ลดจาก ~295 MiB เหลือ ~68 MiB

### 3.2 หน้า SQL ถือ `PgRow` ทั้งหมดไว้ขณะสร้าง JSON

**หลักฐาน** — `sql_console/service.rs:506` `let mut rows = executed.rows;` แล้ว `:524` `for row in &rows {`
`rows` (สูงสุด 16 MiB wire) จึงอยู่ครบจนจบฟังก์ชัน พร้อมกับ `out` (สูงสุด 16 MiB JSON) ทั้งที่ไม่ถูกใช้หลังลูป
(`truncated` คำนวณไว้แล้วที่ `:507`, ค่า `Value` ที่สร้างจาก `try_get_raw` เป็นของตัวเองทั้งหมด ไม่ยืม `row`)

```rust
// ก่อน (service.rs:524)
for row in &rows {

// หลัง — PgRow แต่ละแถวถูก drop ทันทีหลังแปลงเป็น cell
for row in rows {
```

**impact ที่คาด (ก่อนวัด):** peak ต่อคำขอลดได้สูงสุด ~16 MiB (× 4 คำขอพร้อมกัน)
**ต้องยืนยันด้วย heaptrack/dhat ก่อนสรุป** เพราะ `DataRow` ของ sqlx อาจแชร์ buffer กันระหว่างแถว — drop ทีละแถวอาจยังไม่คืนหน่วยความจำจริง
ผู้ใช้หน้านี้มีแค่ admin (`sql.view`) จึงเป็นความเสี่ยงต่ำในทางปฏิบัติ

**ผลวัดจริง (2026-11-18, ดูตารางในข้อ 4.1):** peak ของผล 15 MB × 4 คำขอเพิ่มขึ้นพอ ๆ กันทั้ง image เดิม (+131 MiB) และใหม่ (+132 MiB)
→ ข้อนี้**ไม่ได้ลด peak ที่วัดได้** (สอดคล้องกับข้อกังวลเรื่อง buffer ของ sqlx) ที่ต่างชัดคือหลังคำขอจบ image ใหม่คืนหน่วยความจำ
(RSS 27 MiB เทียบ 197 MiB) ซึ่งมาจากข้อ 3.1 — คงการเปลี่ยนนี้ไว้เพราะไม่มีผลเสีย

---

## 4. High impact + High effort (ทำเมื่อวัดแล้วเจอปัญหาจริงเท่านั้น)

### 4.1 แปลงผลหน้า SQL ระหว่าง stream — **วัดแล้ว ไม่ทำ**
ตอนนี้ repository เก็บ `Vec<PgRow>` ทั้งหมด (`repository.rs:117-129`) แล้ว service ค่อยแปลง
การแปลงทีละแถวใน stream ทำให้ไม่ต้องมี `Vec<PgRow>` เลย และเปิดทางให้นับขนาด body ที่ serialize
(ตอนนี้ไม่มี budget ไหนนับ — `value.rs:155-167` นับขนาดประมาณการ และเซลล์ที่ถอดรหัสแล้ว `service.rs:528-541` ไม่ผ่าน budget)
แต่ต้องส่งแถวผ่าน channel ไปยัง blocking thread (การแปลง/ถอดรหัสเป็นงาน CPU ทำบน async worker ไม่ได้ — `service.rs:440-446`)
ข้ามเส้น repository → service และต้องจัดการ error สองฝั่ง

**ผลวัด (2026-11-18)** — backend ตัวจริงชี้ไปที่ `my_his_migtest`, admin ทดสอบ, ยิง `POST /api/sql/query` พร้อมกัน 4 คำขอ
(= `CONNECTION_LIMIT` ของ role SQL) reset peak ด้วย `/proc/1/clear_refs` ก่อนแต่ละรูปแบบ:

| รูปแบบ (× 4 พร้อมกัน) | ขนาด response ต่อคำขอ | image เดิม peak เพิ่ม / RSS หลังจบ | image ใหม่ peak เพิ่ม / RSS หลังจบ |
|---|---|---|---|
| `repeat('x', 30000)` × 500 แถว (ใกล้เพดาน 16 MiB) | 15.0 MB | +131 MiB / 197 MiB | +132 MiB / 27 MiB |
| 500 แถว × 200 คอลัมน์ตัวเลข | 0.41 MB | +8 MiB / 205 MiB | +19 MiB / 22 MiB |
| idle 30 วินาทีหลังจบ | — | RSS 187 MiB | RSS 22.5 MiB |

ราว 33 MiB ต่อคำขอ ≈ 2 เท่าของ response (wire + JSON/serialize) ต่ำกว่าเกณฑ์ที่ตั้งไว้ (peak เพิ่ม > 200 MiB สำหรับ 4 คำขอ)
→ **ปิดข้อนี้** ขอบบนจริงของหน้า SQL ≈ 130 MiB และคืนหน่วยความจำหลังจบแล้ว ทบทวนอีกครั้งถ้าเพิ่ม `MAX_RESULT_BYTES` หรือ `CONNECTION_LIMIT`

### 4.2 Pagination ของ list endpoint — **ไม่ทำ**
`fetch_all` ที่ไม่มี `LIMIT`:
- `patient/repository.rs:137` — ถอดรหัส 4 ช่องต่อแถว (`patient/service.rs:270-275`)
- `lab_visit/repository.rs:153` (กลุ่ม), `:201` (นัดในกลุ่ม)
- `stock/repository/receipts.rs:96` (ใบรับเข้า), `:707` (ล็อต)
- master ทุกตัว (`disease/repository.rs:47` และ prename/nationality/patient_group/lab/item_category/item_unit/budget_year)
- `item/repository.rs:72`, `users/repository/users.rs:80`, `pages.rs:38`, `roles.rs:26`

**เหตุผลที่ไม่ทำ:** หน้าผู้ป่วยค้นหาชื่อโดยกรองฝั่งเบราว์เซอร์ (`frontend/src/features/patients/PatientsPage.tsx:123`)
เพราะชื่อเข้ารหัสแบบ nonce สุ่ม — server ค้นได้แค่ HN/เลขประจำตัวคนไข้แบบตรงทั้งค่าผ่าน blind index ถ้าแบ่งหน้าฝั่ง server
การค้นหาชื่อจะใช้ไม่ได้ ต้องออกแบบการค้นหาใหม่ทั้งหมด หน้าอื่น ๆ ก็กรอง/เรียงฝั่งเบราว์เซอร์แบบเดียวกัน

**ขนาดข้อมูลจริง (`my_his`, 2026-11-18, `count(*)`):** `patient` 38, `lab_visit` 38, `lab_visit_group` 3, `stock_receipt` 1,
`stock_lot` 1, `item` 1, `users` 4, `audit_log` 522 (audit มีเพดาน 5,000 แถวต่อคำขออยู่แล้ว)

**เกณฑ์ทบทวน:** `patient` เกิน ~5,000 แถว หรือ response ของ `/api/patients` เกิน ~2 MB หรือหน้ารายการโหลดช้าจนรู้สึกได้บนมือถือ
— ถึงตอนนั้นค่อยออกแบบการค้นหาชื่อใหม่ (เช่น ส่งเฉพาะคอลัมน์ที่ใช้ค้น หรือทำ index ค้นหาแบบอื่น) ก่อนแบ่งหน้า

### 4.3 ลด `conn_guard::MAX_CONNECTIONS` — **ทำแล้ว**
`conn_guard.rs:33` = 512 → เพดาน body ที่ buffer พร้อมกันทางทฤษฎี 512 × 256 KiB = 128 MiB
peer ของ backend มีแค่ Caddy ชั้นใน (`frontend/Caddyfile`) ซึ่งโดยปริยายไม่จำกัดจำนวน connection ไป upstream
→ ลด `MAX_CONNECTIONS` เป็น **128** (body เหลือ 32 MiB) และตั้ง `transport http { max_conns_per_host 64 }` ให้ `reverse_proxy`
ทั้งสองตัวใน `frontend/Caddyfile` (รวม 128 พอดี) ให้ Caddy รอคิวคำขอส่วนเกินเอง แทนการไปค้างใน backlog ของ kernel ฝั่ง backend
สองค่านี้ต้องแก้คู่กัน — ใช้งานระดับครอบครัวเปิดพร้อมกันไม่เกินหลักสิบ จึงเหลือเผื่อมาก

---

## 5. Nice-to-have (micro)

ลด allocation ต่อ request ไม่กี่สิบถึงร้อย byte — **ไม่เปลี่ยน peak RSS** ทำเมื่อแตะไฟล์นั้นอยู่แล้วเท่านั้น

### 5.1 hex encode ด้วย `format!` ทีละ byte
`token.rs:26` — ทำงานทุก request ที่ login แล้ว (hash session token), 32 allocation ต่อครั้ง

```rust
// ก่อน
fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// หลัง — allocation เดียว
use std::fmt::Write as _;

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}"); // เขียนลง String ไม่มีทางล้ม
    }
    out
}
```

แพทเทิร์นเดียวกันที่:
- `sql_console/value.rs:302` (`bytea_to_string`) — **ตัวนี้คุ้มสุดในกลุ่ม**: budget ยอมถึง 16 MiB → bytea ใหญ่ทำ allocation ได้หลายล้านครั้งต่อคำขอ (churn + fragmentation)
  `String` ปลายทางจอง `with_capacity` ไว้ถูกแล้ว แก้แค่ `out.push_str(&format!(..))` → `write!(out, ..)`
- `sql_console/sql_text.rs:60`, `sql_console/service.rs:346`

### 5.2 คัดลอก session token โดยไม่จำเป็น
`auth/extractor.rs:50` — `jar` อยู่ตลอดฟังก์ชัน ยืมค่าได้

```rust
// ก่อน
let token = jar
    .get(session::COOKIE_NAME)
    .map(|cookie| cookie.value().to_string())
    .ok_or(AppError::Unauthenticated)?;
let token_hash = session::hash_token(&token);

// หลัง
let cookie = jar.get(session::COOKIE_NAME).ok_or(AppError::Unauthenticated)?;
let token_hash = session::hash_token(cookie.value());
```

### 5.3 ดึง `password_hash` ทุก request
`auth/repository.rs:81` — `find_user_by_id` (เรียกจาก extractor ทุก request ผ่าน `service::current_user_with_permissions`)
select `password_hash` (~100 B) ที่ extractor ไม่ใช้ → แยก query สำหรับ extractor ที่ไม่มีคอลัมน์นี้
(ต้องรัน `cargo sqlx prepare`) ได้ประโยชน์ด้านความปลอดภัยด้วย: hash ไม่ค้างในหน่วยความจำโดยไม่จำเป็น

### 5.4 DTO clone ฟิลด์จาก struct ที่ผู้เรียกเป็นเจ้าของอยู่แล้ว
`auth/dto.rs:85-86`

```rust
// ก่อน
pub fn from_domain(user: &User, permissions: &[Permission]) -> Self {
    UserResponse {
        id: user.public_id,
        username: user.username.clone(),
        display_name: user.display_name.clone(),
        ...

// หลัง — ผู้เรียก (auth/handlers.rs:49, :126) ส่ง User ที่ไม่ใช้ต่อแล้วเข้ามา
pub fn from_domain(user: User, permissions: &[Permission]) -> Self {
    UserResponse {
        id: user.public_id,
        username: user.username,
        display_name: user.display_name,
        ...
```

### 5.5 `menu_for` clone label/icon/route
`users/service/pages.rs:365-367` — `pages` เป็น `Vec` ของตัวเองและไม่ถูกใช้หลังจากนี้ (เรียกทุกครั้งที่โหลดเมนู)

```rust
// ก่อน
Ok(pages
    .iter()
    .filter(|page| ...)
    .map(|page| MenuItem {
        public_id: page.public_id,
        label: page.label.clone(),
        icon: page.icon.clone(),
        route: page.route.clone(),
        ref_id: page.ref_public_id,
    })
    .collect())

// หลัง — move แทน clone (closure ใน filter รับ &page อยู่แล้ว)
Ok(pages
    .into_iter()
    .filter(|page| ...)
    .map(|page| MenuItem {
        public_id: page.public_id,
        label: page.label,
        icon: page.icon,
        route: page.route,
        ref_id: page.ref_public_id,
    })
    .collect())
```

### 5.6 `same_lot` สร้าง lowercase string ในลูป O(n²)
`stock/service/receipts.rs:242-245` ถูกเรียกจากลูปซ้อน `:283-291` → ใบ 100 บรรทัด = 4,950 การเปรียบเทียบ ≈ 9,900 `String`

```rust
// ก่อน
fn same_lot(a: &LineWrite, b: &LineWrite) -> bool {
    a.item_id == b.item_id
        && a.lot_no.as_deref().map(str::to_lowercase) == b.lot_no.as_deref().map(str::to_lowercase)
}
for (index, line) in lines.iter().enumerate() {
    if lines.iter().take(index).any(|earlier| same_lot(earlier, line)) {
        return Err(validation("duplicate lot in receipt"));
    }
}

// หลัง — lowercase ครั้งเดียวต่อบรรทัด, ตรวจซ้ำด้วย HashSet (n allocation, O(n))
let mut seen = HashSet::with_capacity(lines.len());
for line in &lines {
    let key = (line.item_id, line.lot_no.as_deref().map(str::to_lowercase));
    if !seen.insert(key) {
        return Err(validation("duplicate lot in receipt"));
    }
}
```
(`ItemId` derive `Hash`/`Eq` อยู่แล้ว — `domain/mod.rs:72` — ต้องคง semantics ให้ตรงกับ unique index ของ `stock_lot`
ข้อความ error เหมือนเดิมเพราะแบบเดิมก็คืน error ที่บรรทัดซ้ำบรรทัดแรกเช่นกัน)

### 5.7 `domain/changes.rs` copy JSON object ทั้งก้อน
`domain/changes.rs:12` `value.as_object().cloned()` ถูกเรียก 2 ครั้งต่อ audit diff (`:23`)
→ ทำงานกับ `Option<&Map<String, Value>>` แทนการ clone (ใช้ map ว่างคงที่เมื่อไม่ใช่ object)

### 5.8 `with_capacity` ที่รู้ขนาดล่วงหน้า
| ตำแหน่ง | ปัจจุบัน | ขนาดที่รู้ |
|---|---|---|
| `stock/service/receipts.rs:86` | `HashMap::new()` | ≤ จำนวน item ใน `options` |
| `item/service.rs:98` | `HashMap::new()` | ≤ จำนวน item |
| `patient/service.rs:302` | `HashMap::new()` | `rows.len()` (จำนวนผู้ป่วย) |
| `sql_console/value.rs:354` | `String::new()` | `numeric_text_len` คำนวณไว้แล้วที่ `:159` |

ไม่แนะนำ (เคยพิจารณาแล้วตัดออก — จะแย่ลง):
- `sql_console/repository.rs:117` `Vec::new()` → จอง 501 ช่องให้ทุก query แม้ได้ 1 แถว และ repository มองไม่เห็น `MAX_ROWS` (อยู่ใน service)
- `sql_console/embedded.rs:98` `String::new()` → เซลล์ส่วนใหญ่ไม่มี hex ที่ถอดได้และคืน `None` โดยไม่ push เลย การจองล่วงหน้าจะเพิ่ม allocation ทุกเซลล์

### 5.9 permission `Vec` ในคำขอไม่มีเพดาน
`users/dto.rs:131, :140, :172, :201` — จำกัดแค่ body 256 KiB และ `parse_permissions` (`users/service/mod.rs:108-115`)
ปฏิเสธค่าที่ไม่รู้จัก → เพิ่ม `#[validate(length(max = N))]` (N = จำนวน `Permission` ทั้งหมด) — เป็นเรื่อง input hygiene มากกว่า RAM

---

## 6. ภาคผนวก: เจอระหว่างสแกน แต่เป็น CPU/latency ไม่ใช่ RAM

ไม่อยู่ในขอบเขตของเอกสารนี้ — จดไว้เผื่ออ้างอิง
- `crypto.rs:113` สร้าง `Aes256Gcm` (key schedule) ใหม่ทุกครั้งที่ถอดรหัส — หน้า SQL ยอมลองถอดได้ถึง
  200,000 ครั้ง (`embedded.rs:25`) × 4 AAD (`service.rs:511-515`) ต่อคำขอ
- `auth/extractor.rs` ทำ 3 DB round trip ต่อ request (session, user, permissions) ไม่ cache
- `auth/ip_budget.rs:177-180` `retain` O(10,000) ใต้ mutex เมื่อ map เต็ม
- insert ทีละบรรทัดใน transaction: `stock/repository/receipts.rs:344-387` (4–6 statement ต่อบรรทัด),
  `lab_visit/repository.rs:497-519` (≤ 1,000 statement), `users/repository/{users,pages,roles}.rs`
- `users/repository/roles.rs:56-64` `find_role_by_public_id` โหลดทุก role แล้ว `.find()`
- `sql_console/repository.rs:60-73` เปิด `PgConnection` ใหม่ทุกคำขอ (ตั้งใจ — role แยก, ไม่ pool)

---

## 7. เครื่องมือวัดผล

| เครื่องมือ | ใช้ทำอะไร | ต้องเพิ่ม dependency? |
|---|---|---|
| `/proc/1/status` (`VmRSS`, `VmHWM`) ใน container, `docker stats` | RSS ปัจจุบัน / สูงสุดตลอดอายุ process | ไม่ |
| `heaptrack` | allocation hotspot, peak heap, leak — รันไบนารีใต้ heaptrack ใน image สำหรับ debug | ไม่ (ติดตั้งในเครื่อง/image dev) |
| `valgrind --tool=massif` | snapshot heap ตามเวลา | ไม่ |
| `cargo bloat --release` | ขนาดไบนารี / crate ที่กิน text segment | ไม่ (เป็น cargo subcommand) |
| `dhat` crate (`#[global_allocator]` ใต้ feature) | นับ allocation ในเทส — ยืนยันข้อ 3.2 และหัวข้อ 5 | **ใช่ — ต้องขอเจ้าของก่อน** (CLAUDE.md) |
| `tikv-jemallocator` / `mimalloc` | allocator ทางเลือกถ้าข้อ 3.1 ยังไม่พอ, jemalloc มี heap profiling | **ใช่ — ต้องขอเจ้าของก่อน** |

### ขั้นตอนวัดที่แนะนำ
1. ยก stack dev → idle 1 นาที → บันทึก `VmRSS`
2. login 20 ครั้ง (มีช่วงพร้อมกัน 3–4 คำขอ) → บันทึก `VmRSS`/`VmHWM`
3. หน้า SQL: รัน query ผลใหญ่ใกล้เพดาน (เช่น `select repeat('x', 30000) from generate_series(1, 500)`) พร้อมกัน 4 คำขอ → บันทึก
4. idle 2 นาที → บันทึก `VmRSS`
5. ทำซ้ำหลังแก้แต่ละข้อ แล้วกรอกตารางด้านล่าง

### ผลวัดจริง (เว้นไว้กรอก)

ผลวัด backend ตัวจริงบนเครื่อง dev (2026-11-18) — วิธีวัด login ในข้อ 3.1, หน้า SQL ในข้อ 4.1
— แถวสุดท้ายเว้นไว้กรอกผลจากเครื่อง deploy จริง

| สถานการณ์ | ก่อน VmRSS / VmHWM | หลัง VmRSS / VmHWM | ข้อที่แก้ |
|---|---|---|---|
| idle หลังเริ่ม | 26.8 / 26.8 MiB | 8.3 / 23.5 MiB | 3.1 |
| หลัง login 240 ครั้ง | 276.3 / 295.3 MiB | 11.2 / 68.2 MiB | 3.1 |
| idle 30 วินาทีหลัง login | 276.6 / 295.3 MiB | 11.2 / 68.2 MiB | 3.1 |
| SQL ผลใหญ่ × 4 (peak เพิ่ม / RSS หลังจบ) | +131 MiB / 196.9 MiB | +132 MiB / 27.1 MiB | 3.1, 3.2 |
| เครื่อง deploy จริง หลังใช้งาน 1 วัน | | | 3.1 |
