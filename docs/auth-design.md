# ดีไซน์ระบบ Authentication และสิทธิ์ (my-his)

## 1. เป้าหมายและขอบเขต

ระบบมีผู้ใช้ไม่กี่คน (ช่วงแรกคือผู้ดูแลระบบกับพยาบาลหนึ่งคน) ใช้งานผ่านเบราว์เซอร์บนมือถือเป็นหลัก
สิ่งที่ต้องทำให้ได้ใน Milestone นี้

- login / logout ที่ปลอดภัยพอสำหรับใช้งานจริงบนอินเทอร์เน็ต
- รู้ว่า request ไหนมาจากใคร และผู้ใช้คนนั้นมีสิทธิ์อะไร
- admin สร้างบัญชี ปิดบัญชี และรีเซ็ตรหัสผ่านให้ผู้ใช้ได้
- มีร่องรอยตรวจสอบย้อนหลังว่าใครทำอะไร

สิ่งที่ **ยังไม่ทำ** ในรอบนี้: สมัครสมาชิกเอง, ยืนยันตัวตนสองชั้น (MFA), ลืมรหัสผ่านผ่านอีเมล,
เข้าระบบด้วยบัญชี Google โครงสร้างที่วางไว้ต้องเติมสิ่งเหล่านี้ได้ทีหลังโดยไม่ต้องรื้อ

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

ไฟล์เสริมในโมดูล: `dto.rs` (รูปแบบ JSON), `password.rs` (argon2), `session.rs` (สร้าง token และจัดการ cookie)

## 4. ตารางฐานข้อมูล

### users

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| id | uuid | primary key |
| username | text | unique, เก็บเป็นตัวพิมพ์เล็กที่ตัดช่องว่างแล้ว |
| display_name | text | ชื่อที่แสดงในหน้าจอ |
| password_hash | text | argon2id พร้อม salt ในตัว |
| is_active | boolean | ปิดบัญชีโดยไม่ลบข้อมูล |
| must_change_password | boolean | บังคับเปลี่ยนรหัสหลัง admin รีเซ็ตให้ |
| failed_attempts | integer | นับความพยายามที่ผิดติดกัน |
| locked_until | timestamptz | null ถ้าไม่ถูกล็อก |
| created_at / updated_at | timestamptz | |

role ของผู้ใช้ย้ายไปอยู่ตาราง `user_roles` แล้ว (ดูหัวข้อ 10) เพื่อให้คนหนึ่งมีได้หลาย role

### roles

แคตตาล็อก role ที่ admin จัดการได้เอง (สร้าง/แก้/ลบผ่าน API ในหัวข้อ 5)

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| id | uuid | primary key |
| name | text | unique, ชื่อ role เช่น `admin`, `nurse`, หรือชื่อที่ admin ตั้งเพิ่มทีหลัง |
| description | text | nullable, อธิบาย role นี้ให้แอดมินคนอื่นเข้าใจ |
| is_system | boolean | true สำหรับ role ที่ seed มาตอน migration แรก (`admin`, `nurse`) — role ที่ `is_system = true` ห้ามลบและห้ามเปลี่ยน `name` |
| created_at / updated_at | timestamptz | |

### role_permissions

เชื่อม role กับ permission ที่ role นั้นมี — แทนที่ฟังก์ชัน compile-time ที่แปลง role เป็นชุด
permission ในดีไซน์เดิม เพราะตอนนี้ role สร้างเพิ่มได้โดยไม่ต้อง deploy ใหม่

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| role_id | uuid | FK ไป `roles.id`, ลบตามเมื่อ role ถูกลบ |
| permission | text | ค่าตรงกับ `Permission` enum ฝั่ง Rust เสมอ — parse ตอนอ่านจาก DB ถ้า parse ไม่ผ่านถือเป็นข้อมูลเสีย ให้ error ไม่ใช่ข้าม |

primary key คือ (`role_id`, `permission`)

`Permission` เองยังเป็น enum ปิดตายใน Rust เหมือนเดิม (แต่ละ permission ผูกกับ authorization
check จริงในโค้ด สร้างผ่าน UI ไม่ได้) สิ่งที่เปลี่ยนคือ *role ไหนมี permission ไหน* ย้ายจากโค้ด
ไปเป็นข้อมูลในตารางนี้แทน

### user_roles

เชื่อมผู้ใช้กับ role แบบ many-to-many (หนึ่งคนมีได้หลาย role — ดูกฎ default และ invariant ในหัวข้อ 10)

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| user_id | uuid | FK ไป `users.id`, ลบตามเมื่อผู้ใช้ถูกลบ |
| role_id | uuid | FK ไป `roles.id`, **ห้ามลบ role นี้** ถ้ายังมีแถวในตารางนี้อ้างถึงอยู่ (ต้องย้ายคนออกก่อนถึงจะลบ role ได้) |
| assigned_at | timestamptz | เวลาที่ได้รับ role นี้ |

primary key คือ (`user_id`, `role_id`) ต้องมี index บน `role_id` เพิ่ม (ใช้ตอนเช็คว่า role หนึ่ง
ยังมีคนใช้อยู่ไหมก่อนลบ)

### pages

แคตตาล็อกหน้า/เมนูที่ admin จัดการเองได้ (สร้าง/แก้/ลบผ่าน API ในหัวข้อ 5) ใช้คู่กับ
`page_permissions` ด้านล่างเพื่อคุมว่าใครเห็นเมนูไหน — ดูรายละเอียดเต็มในหัวข้อ 11

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| id | uuid | primary key |
| label | text | ชื่อเมนูที่แสดงผล |
| icon | text | nullable, ชื่อ icon ให้ frontend เป็นคนแปลผลเอง |
| route | text | unique, path ของหน้านี้ เช่น `/users`, `/shifts` |
| sort_order | integer | default 0, ลำดับการแสดงผลในเมนู |
| created_at / updated_at | timestamptz | |

**ไม่มี `is_system`** ต่างจาก `roles` โดยตั้งใจ — `is_system` บน `roles` มีไว้กัน role `admin`/`nurse`
ที่ระบบ bootstrap ต้องพึ่งไม่ให้ถูกลบ แต่ไม่มี page ไหนที่ auth/bootstrap ต้องพึ่งการมีอยู่ของมัน
ลบ page ไหนก็ไม่ทำให้ระบบพัง

**Seed migration** สร้าง 3 หน้าเริ่มต้น: `Dashboard` (`/dashboard`, ไม่มี `page_permissions` = หน้า
สาธารณะ ใช้เป็นหน้า default หลัง login) และหน้าตั้งค่าสำหรับแอดมิน 2 หน้า — `การตั้งค่าเพจ`
(`/settings/pages`) กับ `การตั้งค่า role` (`/settings/roles`) ทั้งคู่ต้องการ `user.manage` — นอกเหนือ
จาก 3 หน้านี้ admin เพิ่มเองทีหลังผ่าน API ตอนมีหน้าจริงของฟีเจอร์งานพยาบาล (ตารางเวร, checklist,
คำนวณยา — ยังไม่เริ่มออกแบบ)

### page_permissions

เชื่อม page กับ permission ที่ "พอจะเข้าได้" — เหมือน `role_permissions` แต่ฝั่ง page แทนฝั่ง role

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| page_id | uuid | FK ไป `pages.id`, ลบตามเมื่อ page ถูกลบ |
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
| user_id | uuid | foreign key ไป users, ลบตามเมื่อผู้ใช้ถูกลบ |
| created_at | timestamptz | เวลาที่ login |
| last_seen_at | timestamptz | อัปเดตแบบประหยัด (ดูหัวข้อ 7) |
| idle_expires_at | timestamptz | หมดอายุถ้าไม่ใช้งาน |
| absolute_expires_at | timestamptz | หมดอายุเด็ดขาดไม่ว่าจะใช้งานอยู่หรือไม่ |
| user_agent | text | ไว้ให้ผู้ใช้ดูว่ามีอุปกรณ์ไหน login อยู่ |

ต้องมี index บน `user_id` (ใช้ตอน logout ทุกอุปกรณ์) และงานลบ session ที่หมดอายุทิ้งเป็นระยะ

### audit_log

| คอลัมน์ | ชนิด | หมายเหตุ |
|---|---|---|
| id | bigserial | |
| actor_user_id | uuid | null ได้ กรณี login ล้มเหลวที่ไม่รู้ว่าใคร |
| action | text | เช่น `login_succeeded`, `login_failed`, `user_created` |
| target_type / target_id | text / text | สิ่งที่ถูกกระทำ |
| metadata | jsonb | ข้อมูลเพิ่มเติม ห้ามใส่ความลับ |
| ip | inet | |
| created_at | timestamptz | |

ตารางนี้เขียนอย่างเดียว ไม่มีการแก้หรือลบจากในแอป

## 5. API

ทุก path อยู่ใต้ `/api`

| Method | Path | สิทธิ์ | อธิบาย |
|---|---|---|---|
| POST | `/auth/login` | สาธารณะ | รับ username/password ถ้าผ่านจะตั้ง cookie และคืนข้อมูลผู้ใช้ |
| POST | `/auth/logout` | ต้อง login | ลบ session ปัจจุบันและล้าง cookie |
| POST | `/auth/logout-all` | ต้อง login | ลบทุก session ของผู้ใช้คนนี้ |
| GET | `/auth/me` | ต้อง login | คืนข้อมูลผู้ใช้ปัจจุบันพร้อมรายการ permission |
| POST | `/auth/password` | ต้อง login | เปลี่ยนรหัสผ่านตัวเอง ต้องกรอกรหัสเดิมด้วย |
| GET | `/users` | `user.manage` | รายชื่อผู้ใช้ |
| POST | `/users` | `user.manage` | สร้างผู้ใช้ พร้อมรหัสผ่านชั่วคราวและ role เริ่มต้น |
| PATCH | `/users/{id}` | `user.manage` | เปิด/ปิดบัญชี, เปลี่ยน display_name (ไม่รวม role — ดูแถว `/users/{id}/roles`) |
| POST | `/users/{id}/reset-password` | `user.manage` | ตั้งรหัสชั่วคราวและเพิกถอน session ทั้งหมดของคนนั้น |
| GET | `/roles` | `user.manage` | รายชื่อ role ทั้งหมดพร้อม permission ที่มี |
| POST | `/roles` | `user.manage` | สร้าง role ใหม่ พร้อมชุด permission เริ่มต้น |
| PATCH | `/roles/{id}` | `user.manage` | แก้ description หรือแทนที่ชุด permission ทั้งหมด (full replace) — ถ้า `is_system` ห้ามแก้ `name` |
| DELETE | `/roles/{id}` | `user.manage` | ลบ role — ปฏิเสธด้วย `409` ถ้ายังมีผู้ใช้ผูกอยู่ หรือถ้า `is_system` |
| PUT | `/users/{id}/roles` | `user.manage` | แทนที่ชุด role ทั้งหมดของผู้ใช้คนนี้ (full replace) — ต้องมีอย่างน้อย 1 role เสมอ ส่ง array ว่างจะได้ `400` |
| GET | `/pages` | `user.manage` | รายชื่อ page ทั้งหมดพร้อม permission ที่ต้องมี (สำหรับหน้าแอดมินจัดการเมนู) |
| POST | `/pages` | `user.manage` | สร้าง page ใหม่ พร้อมชุด permission ที่ต้องการ |
| PATCH | `/pages/{id}` | `user.manage` | แก้ label/icon/route/sort_order หรือแทนที่ชุด permission ทั้งหมด (full replace) |
| DELETE | `/pages/{id}` | `user.manage` | ลบ page |
| GET | `/menu` | ต้อง login | คืนรายชื่อ page ที่ผู้ใช้ปัจจุบันเข้าถึงได้ เรียงตาม `sort_order` — ดูหัวข้อ 11 |
| GET | `/health` | สาธารณะ | สำหรับ healthcheck ของ container |

`POST /users` รับ `roleId` เดียว (ไม่ใช่ array) เพื่อให้ตรงกับ default "คราวละ 1 role" ตอนสร้าง
ผู้ใช้ — ถ้าอยากให้ผู้ใช้คนนั้นมีหลาย role ให้ใช้ `PUT /users/{id}/roles` เพิ่มทีหลัง

`GET /menu` ต่างจาก `GET /auth/me` ตรงที่คืนโครงสร้างเมนูพร้อมใช้ (label/icon/route) ที่กรองตามสิทธิ์
แล้ว ไม่ใช่แค่รายการ permission ดิบ — คำนวณฝั่ง backend ให้ ไม่ต้องให้ frontend คำนวณเอง

### รหัสสถานะที่ใช้

- `401` ยังไม่ได้ login หรือ session ใช้ไม่ได้
- `403` login แล้วแต่ไม่มีสิทธิ์
- `409` ชื่อผู้ใช้ซ้ำ
- `429` ถูกล็อกชั่วคราวจากการลองผิดหลายครั้ง พร้อม header `Retry-After`

## 6. ขั้นตอนการ login

1. handler ตรวจรูปแบบ input เบื้องต้น แล้วส่งต่อให้ service
2. service ปรับ username เป็นตัวพิมพ์เล็กและตัดช่องว่าง
3. ตรวจ `locked_until` ถ้ายังไม่พ้นเวลา ตอบ `429` ทันที
4. ดึงผู้ใช้จากฐานข้อมูล **ถ้าไม่พบ ให้ยังคำนวณ hash หลอกอยู่ดี** เพื่อให้เวลาตอบกลับใกล้เคียงกัน
5. ตรวจรหัสผ่านด้วย argon2 (ใน `spawn_blocking`)
6. ถ้าผิด หรือไม่พบผู้ใช้ หรือบัญชีถูกปิด → เพิ่ม `failed_attempts`, บันทึก audit, ตอบ `401` ด้วยข้อความกลาง ๆ ข้อความเดียวเสมอ
7. ถ้าผ่าน → รีเซ็ตตัวนับ, สร้าง session ใหม่, ตั้ง cookie, บันทึก audit, คืนข้อมูลผู้ใช้
8. จุดที่เว้นไว้สำหรับ MFA ในอนาคตคือระหว่างข้อ 6 กับ 7

ข้อความ error ของ login ต้องเหมือนกันทุกกรณี ไม่บอกว่าไม่มีชื่อผู้ใช้นี้ ไม่บอกว่ารหัสผ่านผิด
และไม่บอกว่าบัญชีถูกปิด รายละเอียดจริงอยู่ใน log กับ audit เท่านั้น

## 7. การตรวจ session ในแต่ละ request

Extractor อ่าน cookie แล้ว hash ค่าที่ได้ ไปหาในตาราง `sessions` ด้วย hash นั้น
เงื่อนไขที่ต้องผ่านครบ: มี session อยู่จริง, ยังไม่เลย `idle_expires_at` และ `absolute_expires_at`,
เจ้าของ session ยัง `is_active`

ถ้าผ่าน ให้เลื่อน `idle_expires_at` ออกไป แต่ **ไม่ต้องเขียน DB ทุก request**
ให้เขียนเมื่อเวลาที่ผ่านไปมากกว่าเกณฑ์ (เช่น หนึ่งนาที) เท่านั้น เพื่อลดภาระการเขียน

ค่าอายุที่แนะนำเป็นค่าเริ่มต้น: idle 8 ชั่วโมง (ประมาณหนึ่งเวร) และ absolute 30 วัน
ตั้งผ่าน environment variable ได้

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
- ทุกการ hash และ verify ต้องอยู่ใน `spawn_blocking` มิฉะนั้นจะบล็อก async runtime
- ความยาวขั้นต่ำ 12 ตัวอักษร ไม่บังคับรูปแบบตัวอักษรพิเศษ ความยาวมีผลมากกว่าความซับซ้อน
- ตรวจกับรายการรหัสผ่านยอดนิยมแบบสั้น ๆ ได้ถ้าอยากเพิ่มภายหลัง
- เปลี่ยนรหัสผ่านสำเร็จ ให้เพิกถอน session อื่นทั้งหมดยกเว้นอันปัจจุบัน

## 9. การล็อกชั่วคราว

นับความผิดพลาดติดต่อกันต่อบัญชี ครบ 5 ครั้งล็อก 15 นาที และรีเซ็ตเมื่อ login สำเร็จ

ข้อจำกัดที่รู้อยู่: วิธีนี้เปิดช่องให้คนอื่นจงใจกรอกผิดเพื่อล็อกบัญชีเรา
ยอมรับได้เพราะระบบมีผู้ใช้น้อยและไม่เปิดสาธารณะ ถ้าวันหนึ่งมีผู้ใช้มากขึ้น
ให้เปลี่ยนเป็นการหน่วงเวลาที่เพิ่มขึ้นเรื่อย ๆ แทนการล็อกเด็ดขาด

การจำกัดจำนวน request ต่อ IP เป็นหน้าที่ของ Caddy ไม่ใช่ของ backend

## 10. การจัดการสิทธิ์

- ผู้ใช้มีได้ **หลาย role** ผ่านตาราง `user_roles` (many-to-many, ดูหัวข้อ 4)
- **Default: สร้างผู้ใช้ใหม่ได้ 1 role เสมอ** (`POST /users` รับ `roleId` เดียว) — หลาย role เป็น
  ความสามารถที่มีไว้ใช้ทีหลังผ่าน `PUT /users/{id}/roles` ไม่ใช่ flow ปกติตอนสร้างบัญชี
- **Invariant ที่ service layer ต้องคุม: ผู้ใช้ต้องมีอย่างน้อย 1 role ตลอดเวลา** ห้ามเหลือ 0 role
  เด็ดขาด (กันไม่ให้มี user ที่ login ได้แต่ไม่มีสิทธิ์อะไรเลยโดยไม่ตั้งใจ) — `PUT /users/{id}/roles`
  ที่ส่ง array ว่างต้องถูกปฏิเสธด้วย `400`
- `Permission` เป็น enum ปิดตายใน Rust เหมือนเดิม เช่น `UserManage`, `ShiftRead`, `ShiftWrite`,
  `AuditRead` (แต่ละตัวผูกกับ authorization check จริงในโค้ด สร้างผ่าน UI ไม่ได้)
- **Role ไม่ใช่ Rust enum ปิดตายอีกต่อไป** — เป็นข้อมูลในตาราง `roles` ที่ admin สร้าง/แก้/ลบได้เอง
  ผ่าน API ในหัวข้อ 5 ยกเว้น 2 ตัวที่ seed มาตอน migration แรก (`admin`, `nurse`,
  `is_system = true`) ที่ลบและเปลี่ยนชื่อไม่ได้
- Permission ที่ผู้ใช้มีจริง = **union** ของ permission จากทุก role ที่ผู้ใช้ถืออยู่ (join
  `user_roles` → `role_permissions` แล้ว dedupe) — `GET /auth/me` คืนค่าที่รวมแล้วนี้

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
  ตัดกันไม่ว่าง หรือไม่มี permission กำหนดเลย เรียงตาม `sort_order` แล้วคืน label/icon/route
  พร้อมใช้ ไม่ต้องให้ frontend คำนวณเอง
- **Seed 3 หน้าเริ่มต้น** (หัวข้อ 4): `Dashboard` เป็นหน้า default หลัง login (สาธารณะ ไม่ต้องมี
  permission) กับหน้าตั้งค่า `การตั้งค่าเพจ`/`การตั้งค่า role` (ต้อง `user.manage` ทั้งคู่) —
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
