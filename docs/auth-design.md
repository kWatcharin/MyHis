# ดีไซน์ระบบ Authentication และสิทธิ์ (my-his)

## 1. เป้าหมายและขอบเขต

ระบบมีผู้ใช้ไม่กี่คน (ช่วงแรกคือผู้ดูแลระบบกับพยาบาลหนึ่งคน) ใช้งานผ่านเบราว์เซอร์บนมือถือเป็นหลัก
สิ่งที่ต้องทำให้ได้ใน Milestone นี้

- login / logout ที่ปลอดภัยพอสำหรับใช้งานจริงบนอินเทอร์เน็ต
- รู้ว่า request ไหนมาจากใคร และผู้ใช้คนนั้นมีสิทธิ์อะไร
- admin สร้างบัญชี ปิดบัญชี และรีเซ็ตรหัสผ่านให้ผู้ใช้ได้
- **ลงทะเบียนผ่านลิงก์:** admin สร้างบัญชี/รีเซ็ตรหัสแล้วได้ลิงก์ใช้ครั้งเดียว (อายุ 3 วัน) ผู้ใช้เปิดลิงก์ กรอก
  คำนำหน้า ชื่อ สกุล ตำแหน่ง และตั้งรหัสผ่านเอง — admin ไม่เห็นรหัสผ่านของใคร (เลิกใช้รหัสผ่านชั่วคราวแล้ว ดูหัวข้อ 8)
- มีร่องรอยตรวจสอบย้อนหลังว่าใครทำอะไร

สิ่งที่ **ยังไม่ทำ** ในรอบนี้: สมัครสมาชิกเองจากหน้า login (ทุกบัญชีเริ่มจาก admin), ยืนยันตัวตนสองชั้น (MFA),
ลืมรหัสผ่านผ่านอีเมล (admin ออกลิงก์ตั้งรหัสใหม่ให้แทน), ส่งลิงก์ให้อัตโนมัติ (admin คัดลอกไปส่งเอง), เข้าระบบด้วย
บัญชี Google โครงสร้างที่วางไว้ต้องเติมสิ่งเหล่านี้ได้ทีหลังโดยไม่ต้องรื้อ

## 2. ทางเลือกหลัก: session cookie ไม่ใช่ JWT

**ตัดสินใจ:** ใช้ session token แบบสุ่ม เก็บใน cookie แบบ `HttpOnly` และเก็บสถานะ session ไว้ในตาราง Postgres

เหตุผล

- logout มีผลทันที ปิดบัญชีแล้วผู้ใช้หลุดทันที ไม่ต้องรอ token หมดอายุ
- ไม่ต้องจัดการคู่กุญแจ ไม่ต้องหมุน key ไม่ต้องมี refresh token
- frontend ไม่ต้องเขียนโค้ดเก็บ token เบราว์เซอร์ส่ง cookie ให้เอง และ JavaScript อ่าน token ไม่ได้ ทำให้ความเสียหายจากช่องโหว่ XSS จำกัดกว่า
- backend มีตัวเดียว การถาม DB หนึ่ง query ต่อ request ไม่ใช่ปัญหาที่ขนาดนี้

JWT จะเริ่มคุ้มเมื่อมีหลาย service ที่ต้องตรวจ token เองโดยไม่อยากเรียก DB ซึ่งไม่ใช่สถานการณ์ของโปรเจกต์นี้
ถ้าวันหนึ่งต้องแยก service ให้กลับมาทบทวนหัวข้อนี้ อย่าเปลี่ยนเพราะความเคยชิน

## 3. สถาปัตยกรรมเป็นชั้น

| ชั้น | ไฟล์ | รับผิดชอบ | ห้ามทำ |
|---|---|---|---|
| Router | `auth/routes.rs` | ผูก path กับ handler, กำหนดว่า route ไหนต้อง login | ตรรกะธุรกิจ |
| Extractor | `auth/extractor.rs` | อ่าน cookie, โหลด session, เตรียม `CurrentUser` | query ที่ซับซ้อน |
| Handler | `auth/handlers.rs` | ตรวจรูปแบบ input, แปลงผลลัพธ์และ error เป็น HTTP | แตะ SQL |
| Service | `auth/service.rs` | กฎทั้งหมด: lockout, ตรวจรหัสผ่าน, ออก/เพิกถอน session | import ชนิดข้อมูลของ Axum |
| Repository | `auth/repository.rs` | query SQLx ต่อ `users`, `sessions` | รู้เรื่อง HTTP |

ไฟล์เสริมในโมดูล: `dto.rs` (รูปแบบ JSON), `session.rs` (จัดการ cookie) — ส่วนที่โมดูลอื่นใช้ร่วมอยู่ที่ crate root:
`password.rs` (argon2) และ `token.rs` (สุ่ม token + SHA-256 ใช้ทั้ง session และลิงก์ลงทะเบียน)

## 4. ตารางฐานข้อมูล

**รูปแบบ id ของ entity (users, roles, pages และตารางในไฟล์ `docs/my-his-data-dict.xlsx`):** ทุกตารางมี
`id integer` (identity, primary key — ใช้ join/FK ภายใน backend เท่านั้น ไม่ออกนอกระบบ) และ
`uuid uuid` (unique, default `uuid_v7()` — id ที่ API/frontend เห็น ไม่เปิดเผยลำดับแถว) ตารางกลาง
(`role_permissions`, `user_roles`, `page_permissions`) FK ด้วย `id` ที่เป็น integer ส่วน `sessions`
ใช้ `token_hash` เป็น pk และ `audit_log.id` เป็น bigserial ตามเดิม แถวเดิมของ users/roles/pages คง
ค่า uuid เดิมไว้ตอน migrate (`20261004100000_int_ids_and_uuid_v7.sql`) API จึงไม่เปลี่ยน

`uuid_v7()` เป็นฟังก์ชัน SQL ที่ migration สร้างเอง เพราะ PostgreSQL 16 ยังไม่มี `uuidv7()` built-in

### users

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| id | integer | primary key (identity) — ใช้ภายใน |
| uuid | uuid | unique — id ที่ API เห็น |
| username | text | unique, เก็บเป็นตัวพิมพ์เล็กที่ตัดช่องว่างแล้ว |
| display_name | text | ชื่อที่แสดงในหน้าจอ |
| password_hash | text | argon2id พร้อม salt ในตัว — บัญชีที่ยังไม่ตั้งรหัส (สร้างใหม่/หลังรีเซ็ต) เป็น hash ของค่าสุ่มที่ทิ้งไปแล้ว login ไม่มีทางผ่าน |
| is_active | boolean | ปิดบัญชีโดยไม่ลบข้อมูล |
| must_change_password | boolean | ยังใช้รหัสผ่านชั่วคราว — เหลือแค่ bootstrap admin และบัญชีเดิมก่อนมีลิงก์ลงทะเบียน (การรีเซ็ตไม่ตั้งค่านี้แล้ว) |
| failed_attempts | integer | นับ login ผิดติดกัน (ครบ 5 = ล็อก, ล็อกหมดอายุแล้วนับใหม่, login สำเร็จ = 0) — ดูข้อ 9 |
| locked_until | timestamptz | ล็อก login ถึงเวลานี้ (60 วินาทีหลังผิดครั้งที่ 5) null ถ้าไม่ถูกล็อก — รีเซ็ตรหัสผ่านปลดล็อก |
| prename_id | integer | nullable, FK ไป `prename` — คำนำหน้า (ผู้ใช้กรอกเองตอนลงทะเบียน admin แก้ได้) |
| first_name / last_name | text | nullable — ชื่อ-สกุลเจ้าหน้าที่ (ไม่ใช่ข้อมูลผู้ป่วย จึงไม่เข้ารหัส) |
| position | text | nullable — ตำแหน่ง เช่น พยาบาลวิชาชีพ |
| registered_at | timestamptz | ตั้งรหัสผ่านเองผ่านลิงก์สำเร็จเมื่อ — null = ยังไม่ลงทะเบียน default `now()` (bootstrap admin ถือว่าลงทะเบียนแล้ว) |
| created_at / updated_at | timestamptz | |

role ของผู้ใช้ย้ายไปอยู่ตาราง `user_roles` แล้ว (ดูหัวข้อ 10) เพื่อให้คนหนึ่งมีได้หลาย role

### roles

แคตตาล็อก role ที่ admin จัดการได้เอง (สร้าง/แก้/ลบผ่าน API ในหัวข้อ 5)

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| id | integer | primary key (identity) — ใช้ภายใน |
| uuid | uuid | unique — id ที่ API เห็น |
| name | text | unique, ชื่อ role เช่น `admin`, `nurse`, หรือชื่อที่ admin ตั้งเพิ่มทีหลัง |
| description | text | nullable, อธิบาย role นี้ให้แอดมินคนอื่นเข้าใจ |
| is_system | boolean | true สำหรับ role ที่ seed มาตอน migration แรก (`admin`, `nurse`) — role ที่ `is_system = true` ห้ามลบและห้ามเปลี่ยน `name` |
| created_at / updated_at | timestamptz | |

### role_permissions

เชื่อม role กับ permission ที่ role นั้นมี — แทนที่ฟังก์ชัน compile-time ที่แปลง role เป็นชุด
permission ในดีไซน์เดิม เพราะตอนนี้ role สร้างเพิ่มได้โดยไม่ต้อง deploy ใหม่

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| role_id | integer | FK ไป `roles.id`, ลบตามเมื่อ role ถูกลบ |
| permission | text | ค่าตรงกับ `Permission` enum ฝั่ง Rust เสมอ — parse ตอนอ่านจาก DB ถ้า parse ไม่ผ่านถือเป็นข้อมูลเสีย ให้ error ไม่ใช่ข้าม |

primary key คือ (`role_id`, `permission`)

`Permission` เองยังเป็น enum ปิดตายใน Rust เหมือนเดิม (แต่ละ permission ผูกกับ authorization
check จริงในโค้ด สร้างผ่าน UI ไม่ได้) สิ่งที่เปลี่ยนคือ *role ไหนมี permission ไหน* ย้ายจากโค้ด
ไปเป็นข้อมูลในตารางนี้แทน

### user_roles

เชื่อมผู้ใช้กับ role แบบ many-to-many (หนึ่งคนมีได้หลาย role — ดูกฎ default และ invariant ในหัวข้อ 10)

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| user_id | integer | FK ไป `users.id`, ลบตามเมื่อผู้ใช้ถูกลบ |
| role_id | integer | FK ไป `roles.id`, **ห้ามลบ role นี้** ถ้ายังมีแถวในตารางนี้อ้างถึงอยู่ (ต้องย้ายคนออกก่อนถึงจะลบ role ได้) |
| assigned_at | timestamptz | เวลาที่ได้รับ role นี้ |

primary key คือ (`user_id`, `role_id`) ต้องมี index บน `role_id` เพิ่ม (ใช้ตอนเช็คว่า role หนึ่ง
ยังมีคนใช้อยู่ไหมก่อนลบ)

### pages

แคตตาล็อกหน้า/เมนูที่ admin จัดการเองได้ (สร้าง/แก้/ลบผ่าน API ในหัวข้อ 5) ใช้คู่กับ
`page_permissions` ด้านล่างเพื่อคุมว่าใครเห็นเมนูไหน — ดูรายละเอียดเต็มในหัวข้อ 11

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| id | integer | primary key (identity) — ใช้ภายใน |
| uuid | uuid | unique — id ที่ API เห็น |
| label | text | ชื่อเมนูที่แสดงผล |
| icon | text | nullable, ชื่อ icon ให้ frontend เป็นคนแปลผลเอง |
| route | text | nullable, unique, path ของหน้านี้ เช่น `/users`, `/shifts` — null = เพจหลักที่เป็นหัวข้อกลุ่มเมนู (ไม่มีหน้าของตัวเอง) |
| sort_order | integer | default 0, ลำดับการแสดงผลในเมนู (เทียบกันในชั้นเดียวกัน: เพจหลักกับเพจหลัก, เพจย่อยในเพจหลักเดียวกัน) |
| ref_id | integer | nullable, FK ไป `pages.id` ของ **เพจหลัก** (main page) — null = เป็นเพจหลักเอง (เมนูชั้นบน) ลบเพจหลักที่ยังมีเพจย่อยไม่ได้ (`on delete restrict`) ดูโครงเมนู 2 ชั้นในหัวข้อ 11 |
| created_at / updated_at | timestamptz | |

**เมนู 2 ชั้นผ่าน `ref_id`** (migration `20261012100000_page_ref_id.sql` แทนข้อความ `group_label` เดิม):
เพจหลัก (`ref_id` null) ที่มี route = ลิงก์เดี่ยวชั้นบน เช่น `Dashboard`, ที่ไม่มี route = หัวข้อกลุ่มที่กดพับ/กาง
ดูเพจย่อย เพจย่อย (`ref_id` = เพจหลักที่ไม่มี route) ต้องมี route เสมอ — check constraint กัน `ref_id` ชี้ตัวเอง
และเพจย่อยไม่มี route ส่วนกฎ "ชั้นเดียว / เพจหลักที่มีเพจย่อยไม่มี route" ตรวจที่ service migration แปลงกลุ่มเดิม
ทุกกลุ่มเป็นแถวเพจหลัก (ชื่อ = ชื่อกลุ่ม, `sort_order` = ค่าน้อยสุดในกลุ่ม) และย้าย `Dashboard` ออกจากกลุ่มเป็นเพจหลัก
ผลบน DB สะอาด: `Dashboard`, "ลงทะเบียน" (ผู้ป่วย `/patients` — ต้อง `patient.view`, ผู้ใช้งาน `/users` — ต้อง
`user.view`), "Lab" (นัดหมายแล็ป — ต้อง `lab_visit.view`), "สิทธิ์" (การตั้งค่าเพจ, การตั้งค่า role) และ "การตั้งค่า"
(หน้า master: ปีงบประมาณ, โรค, คำนำหน้า, สัญชาติ, กลุ่มคนไข้, แลป) — admin ย้าย/เปลี่ยนชื่อเพจหลักเองได้ผ่านหน้า
การตั้งค่าเพจ

**ไม่มี `is_system`** ต่างจาก `roles` โดยตั้งใจ — `is_system` บน `roles` มีไว้กัน role `admin`/`nurse`
ที่ระบบ bootstrap ต้องพึ่งไม่ให้ถูกลบ แต่ไม่มี page ไหนที่ auth/bootstrap ต้องพึ่งการมีอยู่ของมัน
ลบ page ไหนก็ไม่ทำให้ระบบพัง

**Seed migration** สร้าง 3 หน้าเริ่มต้น: `Dashboard` (`/dashboard`, ไม่มี `page_permissions` = หน้า
สาธารณะ ใช้เป็นหน้า default หลัง login) และหน้าตั้งค่าสำหรับแอดมิน 2 หน้า — `การตั้งค่าเพจ`
(`/settings/pages`) กับ `การตั้งค่า role` (`/settings/roles`) — ปัจจุบันแต่ละหน้าต้องมีสิทธิ์ "ดู" ของหน้านั้นเอง
(`page.view` / `role.view`, ดูข้อ 10) — นอกเหนือ
จาก 3 หน้านี้ admin เพิ่มเองทีหลังผ่าน API ตอนมีหน้าจริงของฟีเจอร์งานพยาบาล (ตารางเวร, checklist,
คำนวณยา — ยังไม่เริ่มออกแบบ)

### page_permissions

เชื่อม page กับ permission ที่ "พอจะเข้าได้" — เหมือน `role_permissions` แต่ฝั่ง page แทนฝั่ง role

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| page_id | integer | FK ไป `pages.id`, ลบตามเมื่อ page ถูกลบ |
| permission | text | ค่าตรงกับ `Permission` enum ฝั่ง Rust เสมอ (เหมือน `role_permissions.permission`) |

primary key คือ (`page_id`, `permission`)

**Semantic ที่ต้องระบุชัด (ไม่มีมาตรฐานตายตัว):**

- ผู้ใช้เห็น page ได้ถ้ามี **อย่างน้อยหนึ่ง** permission ที่ตรงกับที่ page นั้นต้องการ (OR ไม่ใช่ AND)
  — มีสิทธิ์ด้านใดด้านหนึ่งที่เกี่ยวก็พอให้เห็นทางเข้า
- page ที่**ไม่มีแถวใน `page_permissions` เลย** ถือเป็นหน้าสาธารณะสำหรับผู้ใช้ที่ login แล้ว
  (เช่น หน้า dashboard/profile) ทุกคนเห็นได้โดยไม่ต้องมี permission เฉพาะ

### sessions

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| token_hash | text | primary key, SHA-256 ของ token ที่อยู่ใน cookie |
| user_id | integer | foreign key ไป users, ลบตามเมื่อผู้ใช้ถูกลบ |
| created_at | timestamptz | เวลาที่ login |
| last_seen_at | timestamptz | อัปเดตแบบประหยัด (ดูหัวข้อ 7) |
| idle_expires_at | timestamptz | หมดอายุถ้าไม่ใช้งาน |
| absolute_expires_at | timestamptz | หมดอายุเด็ดขาดไม่ว่าจะใช้งานอยู่หรือไม่ |
| user_agent | text | ไว้ให้ผู้ใช้ดูว่ามีอุปกรณ์ไหน login อยู่ |

ต้องมี index บน `user_id` (ใช้ตอน logout ทุกอุปกรณ์) และงานลบ session ที่หมดอายุทิ้งเป็นระยะ

### registration_links

ลิงก์ลงทะเบียน/ตั้งรหัสผ่านใหม่ที่ admin ออกให้ (หัวข้อ 8) — แพทเทิร์นเดียวกับ `sessions`: เก็บแค่ hash ของ token
ไม่มี id/uuid ไม่ลบแถว (ใช้แล้วตั้ง `used_at`, ถูกแทนด้วยลิงก์ใหม่ตั้ง `revoked_at`)

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| token_hash | text | primary key, SHA-256 ของ token ในลิงก์ (token ดิบอยู่ใน response ครั้งเดียว ไม่เก็บ) |
| user_id | integer | FK ไป `users.id` (on delete cascade) บัญชีที่ลิงก์นี้ใช้ตั้งรหัส |
| created_by | integer | FK ไป `users.id` (on delete set null) admin ที่ออกลิงก์ |
| created_at / expires_at | timestamptz | หมดอายุ = ออกลิงก์ + 3 วัน |
| used_at | timestamptz | ลงทะเบียน/ตั้งรหัสสำเร็จเมื่อ — ใช้ซ้ำไม่ได้ |
| revoked_at | timestamptz | ถูกแทนด้วยลิงก์ใหม่เมื่อ (ผู้ใช้หนึ่งคนมีลิงก์ที่ใช้ได้อย่างมากหนึ่งอัน) |

index บน `user_id` — ลิงก์ "ใช้ได้" = `used_at` และ `revoked_at` เป็น null, ยังไม่หมดอายุ และบัญชียัง `is_active`
(ปิดบัญชีแล้วลิงก์ใช้ไม่ได้ เปิดกลับแล้วลิงก์ที่ยังไม่หมดอายุใช้ได้อีก)

### login_throttle

ตัวนับ login ผิดของ **ชื่อที่ไม่มีในระบบ** (ชื่อที่มีอยู่นับใน `users.failed_attempts` / `locked_until`) — กติกาเดียวกันทุกอย่าง
คนลองจึงแยกไม่ออกว่าชื่อไหนมีจริง (ข้อ 9)

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| username_hash | text | primary key — SHA-256 (hex) ของชื่อที่พิมพ์มา (ตัวเล็ก ตัดช่องว่าง) **ไม่เก็บชื่อดิบ** (อาจเป็นรหัสผ่านที่พิมพ์ผิดช่อง) |
| failed_attempts | integer | ผิดติดกันกี่ครั้ง (ล็อกหมดอายุแล้วนับใหม่) |
| locked_until | timestamptz | null = ไม่ล็อก |
| updated_at | timestamptz | ผิดครั้งล่าสุด — แถวที่ไม่ได้แตะเกิน 30 วันถูกลบตอนเขียน |

### audit_log

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| id | bigserial | |
| actor_user_id | integer | FK ไป `users.id` (on delete set null), null ได้ กรณี login ล้มเหลวที่ไม่รู้ว่าใคร |
| action | text | เข้า/ออก: `login_succeeded`, `login_failed`, `logout`, `logout_all` — การใช้งาน: `<ชนิด>_created/_updated/_deleted` ของทุกหน้า (`patient`, `lab_visit_group`, `lab_visit`, `user`, `role`, `page`, `budget_year`, `disease`, `prename`, `nationality`, `patient_group`, `lab`), `lab_visit_group_exported`, `user_activated/deactivated/roles_replaced/password_reset`, `user_registered`, `user_password_set`, `password_changed` |
| target_type / target_id | text / text | สิ่งที่ถูกกระทำ — `target_id` เก็บเป็น **uuid** ไม่ใช่ id int ภายใน เพื่ออ้างอิงข้ามเวลาได้ — แถวผู้ป่วยมีแค่ uuid; เหตุการณ์ login เก็บ username ของบัญชีที่มีอยู่ หรือ **SHA-256 ของชื่อที่พิมพ์มา** ถ้าไม่มีในระบบ (อาจเป็นรหัสผ่านที่พิมพ์ผิดช่อง — แถวก่อน 2026-10-12 เป็นชื่อดิบ) API ไม่ส่งคอลัมน์นี้ออก |
| metadata | jsonb | ข้อมูลเพิ่มเติม ห้ามใส่ความลับ/ข้อมูลผู้ป่วย — มีแค่: `{"userAgent"}` ของเหตุการณ์เข้า/ออกระบบ (ตัดไม่เกิน 512 ตัวอักษร), `{"label"}` ชื่อ role/เพจ ณ ตอนนั้น (สองตารางนี้ลบจริง), `{"document", "format"}` ของการส่งออก (`cd4`/`roster`, `excel`/`pdf`) |
| ip | inet | IP ของผู้ใช้ — มีเฉพาะเหตุการณ์เข้า/ออกระบบ (ดูด้านล่าง) |
| created_at | timestamptz | index `(action, created_at)` (ประวัติการเข้าออก) และ `(created_at)` (ประวัติการใช้งาน) |

ตารางนี้เขียนอย่างเดียว ไม่มีการแก้หรือลบจากในแอป

**ประวัติการเข้าออก** (หน้า "ประวัติการเข้าออก" `/audit/auth`, API `GET /audit/auth-events`, สิทธิ์ `audit.view`):
- `login_succeeded`, `login_failed`, `logout`, `logout_all` บันทึก IP + User-Agent จาก request header (ไม่เก็บ header อื่น
  — cookie/token ห้ามเด็ดขาด handler ที่รับ `HeaderMap` ต้องใส่ `headers` ใน `skip` ของ `tracing::instrument`)
- `logout` ลบ session ด้วย `delete … returning user_id` คำสั่งเดียวแล้วบันทึกของเจ้าของ — cookie ที่ session หมดอายุ/ไม่มีแล้ว
  ไม่บันทึก (session ที่หมดอายุเองไม่ใช่เหตุการณ์ เพราะไม่มี request ให้บันทึก)
- **IP = ค่าขวาสุดของ `X-Forwarded-For`** คือค่าที่ Caddy (proxy ชั้นเดียวหน้า backend) เติมจากการเชื่อมต่อจริง ค่าทางซ้ายผู้ใช้
  ปลอมได้จึงไม่ใช้ ไม่มี header = null — ข้อจำกัด: (1) ถ้าวันหน้ามี Caddy สองชั้นซ้อนกัน ตัวในต้องตั้ง `trusted_proxies`
  (2) บน Docker Desktop ของ Windows ได้ IP ของ docker gateway ไม่ใช่ IP เครื่องจริง (เซิร์ฟเวอร์ Linux ได้ IP จริง)
  (3) dev เปิด backend ที่ `127.0.0.1:8081` ด้วย ผู้ที่เรียกตรงจากเครื่องนี้ปลอม header ได้
- login ไม่สำเร็จด้วยชื่อที่ไม่มีในระบบ: ชื่อที่พิมพ์มายังเก็บใน `target_id` แต่ **API ประวัติไม่อ่านคอลัมน์นี้** (อาจเป็นรหัสผ่าน
  ที่พิมพ์ผิดช่อง) หน้าเว็บแสดงแค่ "ไม่พบชื่อผู้ใช้" + IP/อุปกรณ์

**ประวัติการใช้งาน** (หน้า "ประวัติการใช้งาน" `/audit/activities`, API `GET /audit/activities`, สิทธิ์ `audit.view` ตัวเดียวกัน):
- ทุก action ยกเว้นเข้า/ออกระบบ — แต่ละโมดูลเขียน audit ในทรานแซกชันเดียวกับการแก้ข้อมูล (หน้าตั้งค่า master, role, เพจ
  เริ่มบันทึกพร้อมหน้านี้ ก่อนหน้านั้นไม่มีประวัติ) ไม่เก็บการเปิดดูหน้า และไม่เก็บ IP/อุปกรณ์ (ดูคู่กับประวัติการเข้าออก)
- **การส่งออกเอกสาร** (Excel/PDF ของนัดหมายแล็ป สร้างฝั่งหน้าเว็บ) บันทึกผ่าน `POST /lab-visit-groups/{id}/exports` ซึ่ง
  หน้าเว็บเรียกก่อนสร้างไฟล์/แสดงหน้าพิมพ์ทุกครั้ง — จุดเดียวที่ backend ตรวจ `lab_visit.export` บันทึกไม่ได้ = ไม่ส่งออก
  (เปิดหน้าพิมพ์ซ้ำ = ส่งออกอีกครั้ง)
- **ชื่อรายการ** (`targetLabel`): ตารางที่ soft delete อ่านชื่อจาก join ด้วย uuid ตอนแสดงผล (ได้ชื่อปัจจุบัน ใช้กับแถวเก่าได้)
  — ผู้ใช้ = `ชื่อที่แสดง (username)`, master = ชื่อ/ปี, นัดหมายและคนไข้ในนัดหมาย = `ปีงบ <ปี> · <กลุ่มคนไข้>`;
  role/เพจ ใช้ `metadata.label` — **แถวผู้ป่วยไม่มีชื่อรายการเสมอ** (ไม่ join ตาราง patient) หน้าเว็บแสดง "ผู้ป่วย 1 ราย"
- response ไม่มี `target_id` และ metadata ดิบ มีแค่ `targetType`, `targetLabel`, `exportDocument`, `exportFormat`

## 5. API

ทุก path อยู่ใต้ `/api`

| Method | Path | สิทธิ์ | อธิบาย |
|---|---|---|---|
| POST | `/auth/login` | สาธารณะ | รับ username/password ถ้าผ่านจะตั้ง cookie และคืนข้อมูลผู้ใช้ — ไม่ผ่าน: `401 { error: "invalid credentials", remainingAttempts }` (เหลืออีกกี่ครั้งก่อนล็อก 4..1) ครั้งที่ 5 และระหว่างล็อก: `429 account temporarily locked` + `Retry-After` (ข้อ 9) |
| POST | `/auth/logout` | ต้อง login | ลบ session ปัจจุบันและล้าง cookie — บันทึก `logout` (IP/User-Agent) ถ้ามี session |
| POST | `/auth/logout-all` | ต้อง login | ลบทุก session ของผู้ใช้คนนี้ |
| GET | `/audit/activities?from=&to=` | `audit.view` | ประวัติการใช้งาน (ทุก action ยกเว้นเข้า/ออก) กฎช่วงวัน/จำนวนแถวเดียวกับ `auth-events` ต่อแถว: `occurredAt`, `action`, `user`, `targetType`, `targetLabel` (แถวผู้ป่วย = null เสมอ), `exportDocument`, `exportFormat` (เฉพาะการส่งออก) |
| GET | `/audit/auth-events?from=&to=` | `audit.view` | ประวัติการเข้าออกของผู้ใช้ทุกคน (`YYYY-MM-DD` วันตามเวลาไทย รวมทั้งสองวัน ไม่เกิน 366 วัน — เกิน/กลับด้าน = `400`) เรียงใหม่→เก่า ไม่เกิน 5,000 แถว (`truncated`) ต่อแถว: `occurredAt`, `event`, `user` (null = ไม่พบชื่อผู้ใช้), `ip`, `userAgent` |
| GET | `/auth/me` | ต้อง login | คืนข้อมูลผู้ใช้ปัจจุบันพร้อมรายการ permission |
| POST | `/auth/password` | ต้อง login | เปลี่ยนรหัสผ่านตัวเอง ต้องกรอกรหัสเดิมด้วย (ผิด → `401`) — รหัสใหม่ ≥ 12 ตัวและต้องไม่ซ้ำรหัสเดิม (`400`) หน้าเว็บเปิดจากเมนูชื่อผู้ใช้ท้ายเมนูซ้าย (ไม่บังคับเปลี่ยน) |
| GET | `/users` | `user.view` | รายชื่อผู้ใช้ทั้งหมดพร้อม role, `isActive`, `mustChangePassword`, `lockedUntil` (null = ไม่ล็อก), ข้อมูลตัว (`prenameId/prenameName/firstName/lastName/position`), `registeredAt` (null = ยังไม่ลงทะเบียน) และ `registrationExpiresAt` (ลิงก์ที่ค้างอยู่ อาจหมดอายุแล้ว — null = ไม่มี) (โมดูล `users/` — หน้า "ผู้ใช้งาน" `/users` กลุ่มเมนูลงทะเบียน) |
| POST | `/users` | `user.create` | สร้างผู้ใช้ที่ยังไม่ลงทะเบียน + role เริ่มต้น → `201 { user, registrationToken, registrationExpiresAt }` (token ของลิงก์ลงทะเบียน อยู่ใน response ครั้งเดียว หน้าเว็บประกอบเป็น `/register#<token>`) — username เก็บเป็นตัวเล็กที่ trim แล้ว ยาว 3–32 ตัว เฉพาะ `a-z 0-9 . _ -` (`400 invalid username`), ชื่อที่แสดง 1–100 ตัว, `409 username already taken` |
| PUT | `/users/{id}` | `user.update` | ฟอร์มแก้ไข: **แทนที่ทั้งชุด** ชื่อที่แสดง + `prenameId/firstName/lastName/position` (null/ว่าง = ล้างค่า, ≤100 ตัว, คำนำหน้าต้องมีอยู่และไม่ถูกลบ `400 unknown prename`) + `roleIds` (กฎเดียวกับ `PUT /users/{id}/roles`) ในทรานแซกชันเดียว |
| PATCH | `/users/{id}` | `user.update` | เปิด/ปิดบัญชี, เปลี่ยน display_name (ไม่รวม role — ดูแถว `/users/{id}/roles`) — ปิดบัญชีแล้วลบ session ทั้งหมดของคนนั้นในทรานแซกชันเดียวกัน (ลิงก์ที่ค้างใช้ไม่ได้ระหว่างปิด); ปิดบัญชีตัวเองไม่ได้ (`400 cannot deactivate your own account`) |
| POST | `/users/{id}/reset-password` | `user.update` | รหัสเดิมใช้ไม่ได้ทันที + ปลดล็อก + เพิกถอน session ทั้งหมด + แทนลิงก์เดิมด้วยลิงก์ใหม่ ในทรานแซกชันเดียว → `{ registrationToken, registrationExpiresAt }` (ใช้ออกลิงก์ลงทะเบียนใหม่ให้คนที่ยังไม่ลงทะเบียนด้วย) — รีเซ็ตของตัวเองไม่ได้ (`400 cannot reset your own password` ใช้ `/auth/password` แทน) |
| POST | `/registration/lookup` | สาธารณะ | `{ token }` → ข้อมูลบัญชีของลิงก์: `username, displayName, mode` (`register` = ลงทะเบียนครั้งแรก / `reset` = ตั้งรหัสใหม่), `expiresAt`, ข้อมูลตัวเดิม, `prenames` (ตัวเลือกคำนำหน้า) — ลิงก์ใช้ไม่ได้ทุกกรณีได้ `400 invalid or expired registration link` ข้อความเดียว (โมดูล `registration/` — หน้า `/register`) |
| POST | `/registration/complete` | สาธารณะ | `{ token, newPassword, prenameId?, firstName?, lastName?, position? }` → 204 รหัส ≥ 12 ตัว; โหมด register บังคับคำนำหน้า/ชื่อ/สกุล (ตำแหน่งไม่บังคับ), โหมด reset ไม่แตะข้อมูลตัว — ใช้ลิงก์ครั้งเดียว ปลดล็อก ลบ session ของบัญชี token อยู่ใน body เสมอ (ไม่อยู่ใน URL ที่ไปติด log) |
| GET | `/roles` | `role.view` หรือ `user.view` | รายชื่อ role ทั้งหมดพร้อม permission ที่มี |
| POST | `/roles` | `role.create` | สร้าง role ใหม่ พร้อมชุด permission เริ่มต้น |
| PATCH | `/roles/{id}` | `role.update` | แก้ description หรือแทนที่ชุด permission ทั้งหมด (full replace) — ถ้า `is_system` ห้ามแก้ `name` (`400 cannot rename a system role` เฉพาะชื่อที่ต่างจากเดิม ส่งชื่อเดิมมาได้) แก้สิทธิ์ของ role ที่ตัวเองถือจนสิทธิ์รวมของตัวเองไม่เหลือ `role.view` + `role.update` ไม่ได้ (`400 cannot remove your own role management permission` — กันแอดมินล็อกตัวเองออก) |
| DELETE | `/roles/{id}` | `role.delete` | ลบ role — ปฏิเสธด้วย `409` ถ้ายังมีผู้ใช้ผูกอยู่ หรือถ้า `is_system` |
| PUT | `/users/{id}/roles` | `user.update` | แทนที่ชุด role ทั้งหมดของผู้ใช้คนนี้ (full replace) — ต้องมีอย่างน้อย 1 role เสมอ ส่ง array ว่างจะได้ `400`; uuid ซ้ำถูกตัดทิ้ง; แก้ของตัวเองแล้วสิทธิ์รวมต้องยังมี `role.view` + `role.update` (`400 cannot remove your own role management permission`) |
| GET | `/pages` | `page.view` | รายชื่อ page ทั้งหมดพร้อม permission ที่ต้องมี (สำหรับหน้าแอดมินจัดการเมนู) เรียงตามโครงเมนู (เพจหลักแต่ละอันตามด้วยเพจย่อย) — `route` null = หัวข้อกลุ่ม, `refId` = uuid ของเพจหลัก |
| POST | `/pages` | `page.create` | สร้าง page ใหม่ พร้อมชุด permission ที่ต้องการ — ไม่ส่ง `route` = หัวข้อกลุ่ม, `refId` = อยู่ใต้เพจหลักนั้น `400` ถ้าเพจหลักไม่พบ/ไม่ใช่หัวข้อกลุ่มชั้นบน (`main page not found`, `main page must be a top-level page without a route`) หรือเพจย่อยไม่มี route (`sub page requires a route`) |
| PATCH | `/pages/{id}` | `page.update` | แก้ label/icon/route/sort_order หรือแทนที่ชุด permission ทั้งหมด (full replace) — `refId`: ไม่ส่ง = ไม่แตะ, `null` = ย้ายขึ้นเป็นเพจหลัก, uuid = ย้ายเข้ากลุ่ม (ล้าง route เป็น null ไม่ได้) `409 page has sub pages` ถ้าหัวข้อที่ยังมีเพจย่อยถูกย้ายเข้ากลุ่มอื่นหรือใส่ route |
| DELETE | `/pages/{id}` | `page.delete` | ลบ page — `409 page has sub pages` ถ้ายังมีเพจย่อย (ย้าย/ลบเพจย่อยก่อน) |
| GET | `/budget-years` | `budget_year.view` | รายการปีงบประมาณที่ยังไม่ถูกลบ เรียง `sort_order` แล้ว `year` มาก→น้อย (โมดูล `budget_year/` — หน้า "ตั้งค่าปีงบประมาณ") |
| POST | `/budget-years` | `budget_year.create` | สร้างปีงบประมาณ (`year` พ.ศ. 4 หลัก, `startDate`/`endDate` เป็น `YYYY-MM-DD`) — `409` ถ้าปีซ้ำ, `400` ถ้าวันสิ้นสุดก่อนวันเริ่ม |
| PATCH | `/budget-years/{id}` | `budget_year.update` | แก้บางฟิลด์ (เช็คช่วงวันที่กับค่าเดิมของอีกฝั่งด้วย) |
| DELETE | `/budget-years/{id}` | `budget_year.delete` | soft delete — `409` ถ้ายังมี `lab_visit_group` (ที่ไม่ถูกลบ) อ้างปีนี้อยู่ |
| GET | `/diseases` | `disease.view` | รายการโรคที่ยังไม่ถูกลบ เรียง `sort_order` แล้วชื่อ (โมดูล `disease/` — หน้า "ตั้งค่าโรค") |
| POST | `/diseases` | `disease.create` | สร้างโรค (`name`, `abbreviation` ไม่เกิน 100 ตัวอักษร ไม่ว่างหลัง trim) — `409` ถ้าชื่อซ้ำ (ไม่สนตัวพิมพ์), `400` ถ้าค่าผิด |
| PATCH | `/diseases/{id}` | `disease.update` | แก้บางฟิลด์ |
| DELETE | `/diseases/{id}` | `disease.delete` | soft delete — `409` ถ้ายังมี `medical_condition` (ที่ไม่ถูกลบ) อ้างโรคนี้อยู่ |
| GET / POST | `/prenames` | `prename.view` (หรือ `user.view`) / `prename.create` | master คำนำหน้า (โมดูล `prename/`): list / สร้าง (`name`, `abbreviation` ไม่บังคับ, `sortOrder`) |
| PATCH / DELETE | `/prenames/{id}` | `prename.update` / `prename.delete` | แก้ / soft delete — `409` ถ้าชื่อซ้ำ (ไม่สนตัวพิมพ์) หรือยังมี `patient` ใช้ |
| GET / POST | `/nationalities` | `nationality.view` / `nationality.create` | master สัญชาติ (`nationality/`): `nationalName`, `nation`, `sortOrder`, `isActive` |
| PATCH / DELETE | `/nationalities/{id}` | `nationality.update` / `nationality.delete` | `409` ถ้า `nationalName` ซ้ำหรือยังมี `patient` ใช้ |
| GET / POST | `/patient-groups` | `patient_group.view` / `patient_group.create` | master กลุ่มคนไข้ (`patient_group/`): `name`, `sortOrder`, `isActive` |
| PATCH / DELETE | `/patient-groups/{id}` | `patient_group.update` / `patient_group.delete` | `409` ถ้าชื่อซ้ำหรือยังมี `visit_group`/`lab_visit_group` ใช้ |
| GET / POST | `/labs` | `lab.view` / `lab.create` | master แลป (`lab/`): `name`, `abbreviation` ไม่บังคับ, `sortOrder`, `isActive` |
| PATCH / DELETE | `/labs/{id}` | `lab.update` / `lab.delete` | `409` ถ้าชื่อซ้ำ (ยังไม่มีตารางอ้าง `lab` จึงลบได้เสมอ) |
| GET | `/patients` | `patient.view` | ผู้ป่วยทั้งหมดที่ยังไม่ถูกลบ (HN/เลขประจำตัว/ชื่อ-สกุลถอดรหัสแล้ว) พร้อมโรคประจำตัว (โมดูล `patient/` — หน้า "ผู้ป่วย" กลุ่มเมนูลงทะเบียน) |
| GET | `/patients/options` | `patient.view` | ตัวเลือกของฟอร์ม: คำนำหน้า, สัญชาติและโรค (เฉพาะที่ใช้งานอยู่) — อ่านผ่านโมดูลนี้ เพราะ `/diseases` ฯลฯ ต้องมีสิทธิ์ดูหน้า master นั้น |
| POST | `/patients` | `patient.create` | ลงทะเบียน (รวม `diseaseIds` โรคประจำตัว) — `409 hn already registered` ถ้า HN ซ้ำ, `400` ถ้าค่าผิด/อ้าง master ที่ไม่มี/โรคปิดใช้งาน |
| PUT | `/patients/{id}` | `patient.update` | **แทนที่ทั้งชุด** (ฟิลด์ nullable ส่ง null = ล้างค่า; `diseaseIds` แทนที่โรคที่ผูกอยู่ทั้งหมด) |
| DELETE | `/patients/{id}` | `patient.delete` | soft delete ผู้ป่วยพร้อมโรคประจำตัว (ปล่อย HN ให้ใช้ซ้ำ) |
| GET | `/lab-visit-groups` | `lab_visit.view` | นัดหมายแล็ปทั้งหมด (หัวนัดหมาย: ปีงบประมาณ + กลุ่มคนไข้ + วันนัด CD4 ครั้งที่ 1/2 + วันนัด VL + จำนวนคนไข้) (โมดูล `lab_visit/` — หน้า "นัดหมายแล็ป" กลุ่มเมนู Lab) |
| GET | `/lab-visit-groups/options` | `lab_visit.view` | ตัวเลือกของฟอร์ม: ปีงบประมาณ, กลุ่มคนไข้ (ที่ยังไม่ถูกลบ พร้อม `isActive`) — เพราะ `/budget-years`, `/patient-groups` ต้องมีสิทธิ์ดูหน้านั้น |
| POST | `/lab-visit-groups` | `lab_visit.create` | สร้างนัดหมาย → `201 { id }` — `409 lab visit group already exists` ถ้าปี+กลุ่มซ้ำ, `400` ถ้า CD4 ครั้งที่ 2 ไม่หลังครั้งที่ 1 หรืออ้าง master ที่ไม่มี/ปิดใช้งาน |
| GET | `/lab-visit-groups/{id}` | `lab_visit.view` | หัวนัดหมาย + `visits` (รายชื่อคนไข้: `patientId` เป็น uuid **ไม่มีข้อมูลระบุตัว** — หน้าเว็บ join กับ `/patients` เอง) ไม่รวมคนไข้ที่ถูกลบจากทะเบียน |
| PUT / DELETE | `/lab-visit-groups/{id}` | `lab_visit.update` / `lab_visit.delete` | แทนที่หัวนัดหมาย (คงปีงบ/กลุ่มที่ถูกปิดใช้งานภายหลังไว้ได้) / soft delete พร้อมรายชื่อคนไข้ทั้งหมด |
| POST | `/lab-visit-groups/{id}/visits` | `lab_visit.update` | `{ patientIds }` เพิ่มคนไข้หลายคน (ต่อท้ายตามลำดับที่ส่ง, ตัดซ้ำ) — ทั้งชุดถูกปฏิเสธถ้ามีคนไม่พบ/ปิดใช้งาน (`400`) หรืออยู่ในนัดหมายแล้ว (`409`) |
| POST | `/lab-visit-groups/{id}/exports` | `lab_visit.export` | `{ document: cd4 \| roster, format: excel \| pdf }` → 204 บันทึกการส่งออกเอกสาร (`lab_visit_group_exported`) — หน้าเว็บเรียกก่อนสร้าง Excel/แสดงหน้าพิมพ์ทุกครั้ง ค่าอื่น = `400`, นัดหมายไม่มี/ถูกลบ = `404` |
| PUT / DELETE | `/lab-visits/{id}` | `lab_visit.update` | แทนที่ระยะเวลากินยา (ปี 0–100, เดือน 0–11), ผล VL, สูตรยา, ลำดับ / เอาคนไข้ออกจากนัดหมาย (soft delete เพิ่มกลับได้) |
| GET | `/menu` | ต้อง login | คืนรายชื่อ page ที่ผู้ใช้ปัจจุบันเข้าถึงได้ เรียงตามโครงเมนู (`id`, `label`, `icon`, `route`, `refId`) — ดูหัวข้อ 11 |
| GET | `/health` | สาธารณะ | สำหรับ healthcheck ของ container |

`POST /users` รับ `roleId` เดียว (ไม่ใช่ array) เพื่อให้ตรงกับ default "คราวละ 1 role" ตอนสร้าง
ผู้ใช้ — ถ้าอยากให้ผู้ใช้คนนั้นมีหลาย role ให้ใช้ `PUT /users/{id}/roles` เพิ่มทีหลัง

`GET /menu` ต่างจาก `GET /auth/me` ตรงที่คืนโครงสร้างเมนูพร้อมใช้ (label/icon/route) ที่กรองตามสิทธิ์
แล้ว ไม่ใช่แค่รายการ permission ดิบ — คำนวณฝั่ง backend ให้ ไม่ต้องให้ frontend คำนวณเอง

### รหัสสถานะที่ใช้

- `401` ยังไม่ได้ login หรือ session ใช้ไม่ได้ — login ไม่ผ่านมี `remainingAttempts` ใน body ด้วย
- `403` login แล้วแต่ไม่มีสิทธิ์
- `409` ชื่อผู้ใช้ซ้ำ
- `429` ถูกล็อกชั่วคราวจากการลองผิดหลายครั้ง พร้อม header `Retry-After`

## 6. ขั้นตอนการ login

1. handler ตรวจรูปแบบ input เบื้องต้น แล้วส่งต่อให้ service
2. service ปรับ username เป็นตัวพิมพ์เล็กและตัดช่องว่าง
3. ตรวจล็อก (`users.locked_until` หรือ `login_throttle` ของชื่อที่ไม่มี) ถ้ายังไม่พ้นเวลา ตอบ `429` ทันที
4. ดึงผู้ใช้จากฐานข้อมูล **ถ้าไม่พบ ให้ยังคำนวณ hash หลอกอยู่ดี** เพื่อให้เวลาตอบกลับใกล้เคียงกัน
5. ตรวจรหัสผ่านด้วย argon2 (ใน `spawn_blocking`)
6. ถ้าผิด หรือไม่พบผู้ใช้ หรือบัญชีถูกปิด → บันทึก audit, นับผิดเพิ่ม (ชื่อที่ไม่มีนับใน `login_throttle`) แล้วตอบ `401` ด้วยข้อความกลาง ๆ
   ข้อความเดียวเสมอ + `remainingAttempts` (ครบ 5 ครั้ง → ล็อก ตอบ `429` แทน) — สามกรณีได้ผลเหมือนกันทุกอย่าง
7. ถ้าผ่าน → รีเซ็ตตัวนับ, สร้าง session ใหม่, ตั้ง cookie, บันทึก audit, คืนข้อมูลผู้ใช้
8. จุดที่เว้นไว้สำหรับ MFA ในอนาคตคือระหว่างข้อ 6 กับ 7

ข้อความ error ของ login ต้องเหมือนกันทุกกรณี ไม่บอกว่าไม่มีชื่อผู้ใช้นี้ ไม่บอกว่ารหัสผ่านผิด
และไม่บอกว่าบัญชีถูกปิด รายละเอียดจริงอยู่ใน log กับ audit เท่านั้น

## 7. การตรวจ session ในแต่ละ request

Extractor อ่าน cookie แล้ว hash ค่าที่ได้ ไปหาในตาราง `sessions` ด้วย hash นั้น
เงื่อนไขที่ต้องผ่านครบ: มี session อยู่จริง, ยังไม่เลย `idle_expires_at` และ `absolute_expires_at`,
เจ้าของ session ยัง `is_active` (ตรวจใน `auth::service::current_user_with_permissions` — บัญชีที่ถูกปิด
ได้ `401` ทันทีแม้แถว session ยังค้างอยู่ และ `PATCH /users/{id}` ที่ปิดบัญชีก็ลบ session ของคนนั้นทิ้งด้วย)

ถ้าผ่าน ให้เลื่อน `idle_expires_at` ออกไป แต่ **ไม่ต้องเขียน DB ทุก request**
ให้เขียนเมื่อเวลาที่ผ่านไปมากกว่าเกณฑ์ (เช่น หนึ่งนาที) เท่านั้น เพื่อลดภาระการเขียน

ค่าอายุที่แนะนำเป็นค่าเริ่มต้น: idle 8 ชั่วโมง (ประมาณหนึ่งเวร) และ absolute 30 วัน
ตั้งผ่าน environment variable ได้

แถว session ที่หมดอายุแล้ว (idle หรือ absolute) ถูกลบทุกครั้งที่มีคน login สำเร็จ — ไม่ต้องมีงานตั้งเวลาแยก
(แถวที่หมดอายุใช้ต่อไม่ได้อยู่แล้ว แค่ไม่ให้ค้างในตาราง)

### การตั้ง cookie

- `HttpOnly` เพื่อกัน JavaScript อ่าน
- `Secure` ใช้ได้เฉพาะบน HTTPS
- `SameSite=Lax` ลดความเสี่ยง CSRF สำหรับการใช้งานแบบ same-site
- `Path=/`
- ไม่ต้องตั้ง `Max-Age` ยาว ถ้าอยากให้ปิดเบราว์เซอร์แล้วหลุด

ถ้าภายหลัง frontend ถูกแยกไปคนละโดเมนกับ backend จะต้องเปลี่ยนเป็น `SameSite=None`
และต้องเพิ่มการป้องกัน CSRF แบบ token ทันที ให้ถือเป็นการตัดสินใจที่ต้องทบทวนร่วมกันก่อน

## 8. รหัสผ่าน

- ใช้ argon2id พารามิเตอร์เริ่มต้นตามคำแนะนำของ OWASP (หน่วยความจำ 19 MiB, iterations 2, parallelism 1)
- ทุกการ hash และ verify ต้องอยู่ใน `spawn_blocking` มิฉะนั้นจะบล็อก async runtime — เรียกผ่าน `password::hash_async` /
  `verify_async` ซึ่งรอคิวให้คำนวณพร้อมกันได้ไม่เกิน 4 งาน (ตัวละ 19 MiB) ถูกยิง login รัว ๆ จะรอคิวจนชน timeout
  ของ request แทนที่จะเปิด blocking thread จนหน่วยความจำเครื่องหมด
- ความยาวขั้นต่ำ 12 ตัวอักษร ไม่บังคับรูปแบบตัวอักษรพิเศษ ความยาวมีผลมากกว่าความซับซ้อน
- ตรวจกับรายการรหัสผ่านยอดนิยมแบบสั้น ๆ ได้ถ้าอยากเพิ่มภายหลัง
- เปลี่ยนรหัสผ่านสำเร็จ ให้เพิกถอน session อื่นทั้งหมดยกเว้นอันปัจจุบัน
- รหัสใหม่ต้องไม่ซ้ำรหัสเดิม (กันใส่รหัสชั่วคราวที่แอดมินเห็นแล้วซ้ำ ซึ่งทำให้การเปลี่ยนไม่มีความหมาย)
- **ไม่มีรหัสผ่านชั่วคราวแล้ว — ใช้ลิงก์ลงทะเบียน/ตั้งรหัสใหม่แทน:**
  - สร้างผู้ใช้/รีเซ็ตรหัส ตั้ง `password_hash` เป็น hash ของค่าสุ่ม 256 บิตที่ทิ้งทันที (login ไม่ผ่านแบบเดียวกับรหัสผิด)
    แล้วออกลิงก์ `/register#<token>` ใช้ครั้งเดียว อายุ 3 วัน (ตาราง `registration_links` เก็บแค่ SHA-256 ของ token)
  - token อยู่หลัง `#` — ไม่ถูกส่งไป server ตอนเปิดหน้า (ไม่ติด log/Referer) หน้าเว็บอ่านแล้วลบออกจาก URL ทันที
    และส่งให้ API ใน body ของ POST เท่านั้น
  - ลิงก์ใช้ไม่ได้ทุกกรณีได้ข้อความกลางเดียว ตรวจลิงก์ (ราคาถูก) ก่อนคำนวณ argon2 เสมอ จึงไม่เปิดช่อง DoS
    และ token 256 บิตเดาไม่ได้ จึงไม่ต้องมี rate limit เพิ่ม
  - ลงทะเบียนครั้งแรก: ผู้ใช้กรอกคำนำหน้า ชื่อ สกุล (บังคับ) ตำแหน่ง (ไม่บังคับ) + ตั้งรหัส → `registered_at`
    ลิงก์จากการรีเซ็ต: ตั้งรหัสใหม่อย่างเดียว — ทั้งสองแบบปลดล็อก ลบ session เก่า audit `user_registered` /
    `user_password_set` (actor = เจ้าของบัญชี)
  - ลิงก์ชี้ไปที่ origin ที่แอดมินเปิดหน้าอยู่ — ระบบที่เปิดผ่าน localhost ได้ลิงก์ที่เปิดได้แค่เครื่องนั้น
    (หน้าเว็บเตือน) ส่งให้มือถือได้จริงเมื่อเปิดผ่านโดเมน HTTPS (cookie เป็น `Secure`)
- `must_change_password` เหลือแค่ bootstrap admin และบัญชีเดิมที่สร้างด้วยรหัสชั่วคราวก่อนมีลิงก์ — ไม่บังคับเปลี่ยน
  แค่ทำให้หน้าเว็บเตือนที่ปุ่มเปลี่ยนรหัสผ่าน

## 9. การล็อกชั่วคราว

นับ login ผิดติดกันต่อ **ชื่อผู้ใช้ที่พิมพ์มา** ครบ 5 ครั้งล็อก **60 วินาที** (เดิม 15 นาที — เปลี่ยนตามที่เจ้าของขอ 2026-10-12)
- ผิดครั้งที่ 1–4 ตอบ `401` พร้อม `remainingAttempts` (4..1) หน้า login แสดง "เหลือโอกาส X/5 ครั้ง" เริ่มที่ 5/5
- ครั้งที่ 5 และทุกครั้งระหว่างล็อก (แม้รหัสถูก) ตอบ `429` + `Retry-After` หน้า login ปิดฟอร์มและนับถอยหลังตามนั้น
- ล็อกหมดอายุแล้วนับใหม่ (ได้ 5 ครั้งเต็ม), login สำเร็จรีเซ็ตเป็น 0, รีเซ็ตรหัสผ่านจากหน้าผู้ใช้งานปลดล็อก
- **ชื่อที่ไม่มีในระบบนับและล็อกด้วยกติกาเดียวกัน** (ตาราง `login_throttle` เก็บแค่ SHA-256 ของชื่อ) — status, body และ
  `Retry-After` เหมือนชื่อที่มีจริงทุกครั้ง จึงใช้ตัวนับเดาว่าชื่อไหนมีอยู่ไม่ได้ (ก่อนหน้านี้ชื่อที่ไม่มีไม่เคยถูกล็อก
  คนลองผิด 5 ครั้งจึงรู้ได้) — เทส `backend/tests/login_throttle.rs`
- ตัวนับอยู่ที่ server หน้าเว็บแค่แสดง: refresh แล้วหน้ากลับเป็น 5/5 แต่ server ยังจำ (ส่งแล้วได้ `429` นับถอยหลังต่อ)

ข้อจำกัดที่รู้อยู่:
- ล็อก 60 วินาทีลองเดารหัสได้ราว 5 ครั้งต่อนาทีต่อชื่อ — รับได้เพราะรหัส ≥ 12 ตัว + argon2 + ระบบไม่เปิดสาธารณะ
- คนอื่นจงใจกรอกผิดเพื่อล็อกบัญชีเราได้ (ครั้งละ 60 วินาที) ยอมรับได้เพราะระบบมีผู้ใช้น้อย
- ถ้าวันหนึ่งมีผู้ใช้มากขึ้นหรือเปิดสาธารณะ ให้เปลี่ยนเป็นล็อกที่นานขึ้นเรื่อย ๆ

การจำกัดจำนวน request ต่อ IP เป็นหน้าที่ของ Caddy ไม่ใช่ของ backend

## 10. การจัดการสิทธิ์

- ผู้ใช้มีได้ **หลาย role** ผ่านตาราง `user_roles` (many-to-many, ดูหัวข้อ 4)
- **Default: สร้างผู้ใช้ใหม่ได้ 1 role เสมอ** (`POST /users` รับ `roleId` เดียว) — หลาย role เป็น
  ความสามารถที่มีไว้ใช้ทีหลังผ่าน `PUT /users/{id}/roles` ไม่ใช่ flow ปกติตอนสร้างบัญชี
- **Invariant ที่ service layer ต้องคุม: ผู้ใช้ต้องมีอย่างน้อย 1 role ตลอดเวลา** ห้ามเหลือ 0 role
  เด็ดขาด (กันไม่ให้มี user ที่ login ได้แต่ไม่มีสิทธิ์อะไรเลยโดยไม่ตั้งใจ) — `PUT /users/{id}/roles`
  ที่ส่ง array ว่างต้องถูกปฏิเสธด้วย `400`
- **`Permission` แยกตามเพจ: `<resource>.<action>`** — ปิดตายใน Rust (ตาราง `PERMISSIONS` ใน
  `backend/src/domain/mod.rs` แต่ละตัวผูกกับ authorization check จริงในโค้ด สร้างผ่าน UI ไม่ได้) มี 47 ตัว:
  `patient` (ผู้ป่วย), `lab_visit` (นัดหมายแล็ป), `role`, `page` และ master 6 หน้า (`budget_year`, `disease`,
  `prename`, `nationality`, `patient_group`, `lab`) ได้ `view` / `create` / `update` / `delete` — `user` (ผู้ใช้งาน)
  ไม่มี `delete` (ปิดใช้งานแทน), `lab_visit` มี `export` เพิ่ม, `shift` มี `view` / `update` และ `audit` มี `view`
  (สองตัวหลังยังไม่มีหน้า) หน้าตั้งค่าสิทธิ์แสดงเป็นตาราง เพจ × ดู/เพิ่ม/แก้ไข/ลบ/ส่งออก
  - **ทุก action ตรวจแยกกัน** (แก้ไขไม่ implies ดู) — POST = เพิ่ม, PUT/PATCH = แก้ไข, DELETE = ลบ, อ่าน = ดู
    ผู้ใช้งาน: แก้ข้อมูล/ชุด role/เปิด-ปิดใช้งาน/รีเซ็ตรหัส = `user.update` นัดหมายแล็ป: เพิ่ม/แก้/เอาคนไข้ออกจาก
    นัดหมาย และแก้หัวนัดหมาย = `lab_visit.update` (สร้างนัดหมาย = `create`, ลบนัดหมายทั้งอัน = `delete`)
  - **ข้อมูลข้ามเพจ:** หน้านัดหมายแล็ป (รายละเอียด/พิมพ์) join ชื่อ/HN จาก `GET /patients` จึงต้องมี `patient.view`
    ด้วย (ไม่มีก็ขึ้นข้อความแจ้ง) หน้าผู้ใช้งานอ่าน `GET /roles` และ `GET /prenames` ด้วย `user.view` ได้
  - **`lab_visit.export` คุมที่หน้าเว็บ** (ซ่อนปุ่มส่งออก + กั้น route หน้าพิมพ์) เพราะเอกสารสร้างในเบราว์เซอร์จาก
    ข้อมูลที่อ่านด้วยสิทธิ์ดู backend ไม่มี endpoint ส่งออกให้กั้น — ไม่ใช่การกันข้อมูลรั่ว คนที่ดูได้ก็เห็นข้อมูลชุดเดียวกัน
  - migration `20261013100000_page_action_permissions.sql` แปลงคีย์เดิม 6 ตัวของทุก role (รวม role ที่สร้างเอง):
    `patient.read` → `patient.view` + `lab_visit.view` + `lab_visit.export`, `patient.write` → เพิ่ม/แก้ไข/ลบ ของ
    ผู้ป่วยและนัดหมายแล็ป, `user.manage` → ทุก action ของผู้ใช้งาน/สิทธิ์/เพจ/master, `shift.read/write` →
    `shift.view/update`, `audit.read` → `audit.view` และ `page_permissions` เป็นสิทธิ์ "ดู" ของเพจนั้น — เมนูของทุก role
    เหมือนเดิม ท้าย migration ตรวจว่าไม่เหลือคีย์นอกชุดใหม่ (เหลือ = rollback ทั้งก้อน เพราะคีย์ที่ parse ไม่ได้ทำทุก
    request พัง)
- **Role ไม่ใช่ Rust enum ปิดตายอีกต่อไป** — เป็นข้อมูลในตาราง `roles` ที่ admin สร้าง/แก้/ลบได้เอง
  ผ่าน API ในหัวข้อ 5 ยกเว้น 2 ตัวที่ seed มาตอน migration แรก (`admin`, `nurse`,
  `is_system = true`) ที่ลบและเปลี่ยนชื่อไม่ได้
- Permission ที่ผู้ใช้มีจริง = **union** ของ permission จากทุก role ที่ผู้ใช้ถืออยู่ (join
  `user_roles` → `role_permissions` แล้ว dedupe) — `GET /auth/me` คืนค่าที่รวมแล้วนี้
- **กันแอดมินล็อกตัวเองออก (service ของ `users/`):** ปิดบัญชีตัวเอง และรีเซ็ตรหัสของตัวเองถูกปฏิเสธด้วย `400`
  และสิทธิ์รวมของตัวเอง (union ทุก role ที่ถือ) ต้องยังมี `role.view` + `role.update` เสมอ — มีสองตัวนี้ก็เข้าหน้า
  การตั้งค่าสิทธิ์แล้วแก้สิทธิ์คืนให้ตัวเองได้ ตรวจทั้งตอนแทน role ของตัวเอง (`PUT /users/{id}/roles`, `PUT /users/{id}`)
  และตอนแก้สิทธิ์ของ role ที่ตัวเองถือ (`PATCH /roles/{id}`) — ปฏิเสธด้วย
  `400 cannot remove your own role management permission`
- **ห้ามให้สิทธิ์เกินตัวเอง (service ของ `users/`, เพิ่ม 2026-10-12 หลัง security audit):** สิทธิ์ของผู้ทำ = union ทุก role ที่ถือ
  - ใส่ role ให้ใคร (`POST /users`, `PUT /users/{id}`, `PUT /users/{id}/roles`) ได้เฉพาะ role ที่สิทธิ์ทุกตัว ⊆ ของผู้ทำ
  - สร้าง role / ตั้งสิทธิ์ของ role (`POST /roles`, `PATCH /roles/{id}`) ได้เฉพาะสิทธิ์ที่ผู้ทำถืออยู่
    → `403 cannot grant permissions you do not have`
  - บัญชีที่มีสิทธิ์ที่ผู้ทำไม่มี (เช่น admin) แก้ข้อมูล/role, รีเซ็ตรหัส, เปิด-ปิดบัญชีไม่ได้
    → `403 cannot manage a user with permissions you do not have` — กันยึดบัญชี admin ด้วยการรีเซ็ตรหัส
    (คนรีเซ็ตได้ลิงก์ตั้งรหัสใหม่มาเอง)
  - role ที่มีสิทธิ์ที่ผู้ทำไม่มี แก้ (แม้แค่คำอธิบาย/ลดสิทธิ์) หรือลบไม่ได้ → `403 cannot manage a role with permissions you do not have`
  - เหตุผล: ก่อนหน้านี้คนที่มีแค่ `user.update` ใส่ role admin ให้ตัวเองได้ (ผ่านกฎกันล็อกตัวเองเพราะ admin มี role.view +
    role.update) และ `role.update` ให้สิทธิ์อะไรก็ได้กับ role ที่ตัวเองถือ — admin มีครบทุกสิทธิ์จึงทำได้ทุกอย่างตามเดิม
  - เทส `backend/tests/permission_ceiling.rs`
- **audit ของการจัดการผู้ใช้:** ทุกการเขียนของ `/users*` บันทึก `audit_log` ในทรานแซกชันเดียวกัน —
  action `user_created`, `user_updated` (แก้ชื่อ/ข้อมูลตัว), `user_activated`, `user_deactivated`, `user_password_reset`,
  `user_roles_replaced` (เฉพาะเมื่อชุด role เปลี่ยนจริง) target_type `user`, target_id เป็น uuid ของผู้ใช้ ไม่มี metadata
  และ `user_registered` / `user_password_set` เมื่อผู้ใช้ใช้ลิงก์สำเร็จ (actor = เจ้าของบัญชีเอง) — role/เพจ บันทึก
  `role_*` / `page_*` (created/updated/deleted) พร้อม `metadata.label` (ดูหัวข้อ audit_log)

จุดที่ตรวจสิทธิ์

- **Router** ตรวจแค่ว่า login แล้วหรือยัง
- **Service** เป็นที่ตรวจ permission จริง เพราะสิทธิ์มักขึ้นกับข้อมูล เช่น "แก้บันทึกของตัวเองได้"
- `403` ต่างจาก `401` ชัดเจน อย่าใช้ปนกัน

**สิ่งที่เสียไปกับการเปลี่ยนนี้ (บันทึกไว้ให้ชัด เพราะกลับคำตัดสินใจเดิมของหัวข้อนี้):** ก่อนหน้านี้
เอกสารบอกว่า "ตาราง roles/permissions แบบเต็มยังไม่ต้องทำ จะทำเมื่อผู้ใช้เยอะพอที่จะอยากเปลี่ยนสิทธิ์
โดยไม่ deploy ใหม่" — ตอนนี้ทำแล้ว ข้อดีคือเปลี่ยนสิทธิ์ของ role ได้โดยไม่ต้อง deploy ใหม่ตามที่ตั้งใจ
แต่แลกกับ compile-time exhaustiveness check ที่เคยรับประกันว่า permission ใหม่ต้องถูกกำหนดให้ครบ
ทุก role ตอน compile หายไป — role ที่สร้างใหม่แล้วไม่ได้ตั้ง permission ให้จะไม่มีสิทธิ์อะไรเลย
(fail-safe แต่ไม่มี compiler เตือนเหมือนก่อน)

การควบคุมว่าเมนู/หน้าไหนแสดงให้ใครเห็นสร้างอยู่บนฐานของ permission ชุดนี้ ไม่ใช่ระบบสิทธิ์ใหม่อีกชุด
— ดูหัวข้อ 11

## 11. การจัดการเมนู/การเข้าถึงหน้า (Page Access)

`pages` และ `page_permissions` (หัวข้อ 4) คือ mapping ว่า permission ชุดไหนเห็น page ไหนในเมนู
ไม่ใช่ระบบสิทธิ์ใหม่ — สร้างอยู่บนฐานของ `Permission` ที่มีอยู่แล้วทั้งหมด

- **โมเดล:** page หนึ่งอันมีได้หลาย permission ที่ "พอให้เข้าได้" (many-to-many ผ่าน
  `page_permissions`) ผู้ใช้เห็น page ถ้ามี permission ตรงกัน **อย่างน้อยหนึ่งตัว** (OR) page ที่ไม่
  ผูก permission เลยถือเป็นหน้าสาธารณะสำหรับผู้ใช้ที่ login แล้วทุกคน
- **`GET /menu` คำนวณฝั่ง backend:** join permission ของผู้ใช้ปัจจุบัน (ที่มีอยู่แล้วจาก
  `user_roles` → `role_permissions`, หัวข้อ 10) กับ `page_permissions` กรองเอาเฉพาะ page ที่
  ตัดกันไม่ว่าง หรือไม่มี permission กำหนดเลย เรียงตามโครงเมนูแล้วคืน label/icon/route/refId
  พร้อมใช้ ไม่ต้องให้ frontend คำนวณเอง
- **เมนู 2 ชั้น (`pages.ref_id`):** เพจย่อยเห็นเมื่อทั้งตัวเองและเพจหลักผ่านสิทธิ์ (permission บนหัวข้อกลุ่ม
  คุมทั้งกลุ่ม — ค่าเริ่มต้นหัวข้อไม่ผูก permission) หัวข้อกลุ่ม (เพจหลักไม่มี route) เห็นเมื่อมีเพจย่อยที่เห็นได้
  อย่างน้อยหนึ่งหน้า ไม่โชว์หัวข้อเปล่า เช่น nurse ไม่เห็นหัวข้อ "สิทธิ์"/"การตั้งค่า" — RoleSwitcher ที่ preview
  สิทธิ์ของ role อื่นกรองด้วยกฎเดียวกันฝั่งหน้าเว็บ (`frontend/src/features/auth/menu.ts`)
- **Seed 3 หน้าเริ่มต้น** (หัวข้อ 4): `Dashboard` เป็นหน้า default หลัง login (สาธารณะ ไม่ต้องมี
  permission) กับหน้าตั้งค่า `การตั้งค่าเพจ`/`การตั้งค่า role` (ปัจจุบัน `page.view` / `role.view`) —
  นอกเหนือจากนี้ admin เพิ่มเองทีหลังผ่าน `POST /pages`
- **อยู่ใน module `users/` เดิม** (จุดเดียวกับ `/roles` — เป็นกลุ่มเดียวกันคือ "การจัดการสิทธิ์ระดับ
  แอดมิน" ไม่ใช่ auth/session) ยังไม่แยกเป็น module ใหม่ต่างหาก ถ้า `users/` เริ่มใหญ่เกะกะค่อย
  พิจารณาแยกทีหลัง ไม่ทำล่วงหน้า

## 12. การจัดการ error

มี `AppError` เพียง enum เดียวสำหรับทั้งแอป และ implement การแปลงเป็น response ที่เดียว
กฎคือ รายละเอียดทางเทคนิคไปที่ log ส่วน response ที่ส่งออกไปบอกเท่าที่ผู้ใช้ต้องรู้
error ทุกชนิดของ login ต้องแปลงเป็น response หน้าตาเดียวกัน

ทุก log ควรมี request id เพื่อไล่ปัญหาได้ และต้องไม่มีรหัสผ่าน token หรือ cookie อยู่ในนั้นเด็ดขาด

## 13. ผู้ใช้คนแรก

ตอนเริ่มระบบ ถ้ายังไม่มีผู้ใช้เลย ให้สร้างบัญชี admin จากค่าใน environment variable
และตั้ง `must_change_password` เป็นจริง ห้าม hard-code รหัสผ่านไว้ในโค้ดหรือใน migration

ต้องผูก role `admin` ให้บัญชีนี้ผ่าน `user_roles` ในทรานแซกชันเดียวกับตอนสร้างผู้ใช้ (สอดคล้องกับ
invariant ในหัวข้อ 10 ที่ผู้ใช้ต้องมีอย่างน้อย 1 role เสมอ) role `admin` ต้องมีอยู่แล้วจาก seed
migration (`is_system = true`) ก่อนขั้นตอนนี้จะรันได้

## 14. ลำดับการพัฒนา

1. โครง Axum + เชื่อม Postgres + `/health` + migration แรก
2. ตาราง `users` + hash รหัสผ่าน + สร้าง admin คนแรก
3. login / logout / me พร้อม session และ cookie
4. extractor และการบังคับสิทธิ์ + โมดูล `users`
5. audit log + การล็อกชั่วคราว + งานลบ session ที่หมดอายุ
6. เทสระดับ API ให้ครบตามรายการใน CLAUDE.md

## 15. คำถามที่ยังเปิดอยู่

- frontend จะเป็นอะไร และจะอยู่โดเมนเดียวกับ backend หรือไม่ (มีผลกับการตั้ง cookie)
- ต้องการฟีเจอร์ "จำฉันไว้" หรือปลดล็อกด้วย PIN บนมือถือหรือไม่
- ต้องการหน้าแสดงอุปกรณ์ที่ login อยู่ให้ผู้ใช้จัดการเองหรือไม่
