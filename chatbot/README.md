# my-his chatbot

service Python/FastAPI สำหรับทำ chatbot — **ตอนนี้วางโครงเท่านั้น** (มีแค่ `GET /health`)
กฎการเขียนโค้ด: [`docs/python-guidelines.md`](../docs/python-guidelines.md) — อ่านก่อนแตะไฟล์ในโฟลเดอร์นี้

## ตำแหน่งในระบบ

```
frontend → /api/chat/* → backend (Rust: session, สิทธิ์, audit, คัดกรองข้อมูลผู้ป่วย) → chatbot:8000 → LLM API ภายนอก
```

- chatbot เป็น service ภายในล้วน อยู่ network `chat` ร่วมกับ `backend` เท่านั้น — `frontend`/`caddy` มองไม่เห็น
  จึง **ไม่มีโค้ด auth ใน Python** ทุก request ที่มาถึงผ่านด่านของ backend มาแล้ว
- ได้ environment เฉพาะ `CHATBOT_*` (ไม่เห็น key เข้ารหัสข้อมูลผู้ป่วยหรือรหัส DB ของ backend)
- ข้อมูลผู้ป่วยที่ chatbot เห็นมีเท่าที่ backend ส่งมา และห้ามส่งข้อมูลที่ระบุตัวผู้ป่วยไป LLM ภายนอก (กฎข้อ 20)

## โครงสร้าง

```
chatbot/
├─ pyproject.toml          dependency + ตั้งค่า ruff / pyrefly / pytest
├─ uv.lock                 ตรึงเวอร์ชัน (commit ขึ้น git — Docker ใช้ --locked)
├─ Dockerfile              builder ตรวจ ruff + pyrefly ก่อน → runtime มีแค่ .venv (รันเป็น nobody)
├─ gunicorn.conf.py        gunicorn + uvicorn worker ใน container (อ่าน CHATBOT_WORKERS ผ่าน Settings)
├─ scripts/dev.py          dev server: pyrefly check → uvicorn --reload ที่ 127.0.0.1:8082
├─ src/my_his_chatbot/
│  ├─ main.py              create_app() + lifespan (httpx client ตัวเดียว, DB pool เมื่อตั้งค่า) — ที่เดียวที่รู้จัก router ทุกตัว
│  ├─ config.py            Settings (pydantic-settings, prefix CHATBOT_) — ที่เดียวที่อ่าน env
│  ├─ deps.py              dependency ของ router: get_http_client, get_db_pool
│  ├─ db/pool.py           AsyncConnectionPool ของ psycopg 3
│  └─ health/router.py     GET /health (liveness — ไม่ ping DB/LLM)
└─ tests/                  pytest + TestClient; conftest.py ตรวจ pyrefly ก่อนรันเทสทุกครั้ง
```

ฟีเจอร์ใหม่ = แพ็กเกจใหม่ใต้ `src/my_his_chatbot/<ฟีเจอร์>/` มี `router.py → service.py → repository.py` + `schemas.py`
แล้ว `include_router` ใน `main.py` (กฎข้อ 7)

## รันบนเครื่อง dev

ต้องมี [uv](https://docs.astral.sh/uv/) (uv โหลด Python 3.14 ให้เองตาม `.python-version`)

```bash
cd chatbot
uv sync                          # สร้าง .venv ตาม uv.lock (รวม dev group)
cp .env.example .env             # ไม่บังคับ — ไม่มีไฟล์นี้ก็ใช้ค่าปริยาย

uv run python scripts/dev.py     # pyrefly check → dev server http://127.0.0.1:8082/health
uv run pytest                    # pyrefly check → เทส
uv run ruff format && uv run ruff check
```

**pyrefly ต้องผ่านก่อนรันทุกครั้ง** — ทั้ง 3 ทางที่รันโค้ดได้ (dev server, pytest, docker build) ตรวจให้เองแล้ว
ห้ามรันด้วย `uv run uvicorn ...` ตรง ๆ (ข้ามด่าน) — กฎข้อ 18

ใน Docker (จากรากโปรเจกต์): `docker compose up -d --build chatbot` — พอร์ต `127.0.0.1:8082` ถ้าคัดลอกส่วน chatbot
จาก `docker-compose.override.example.yml` แล้ว (ชนกับ dev server นอก Docker ที่พอร์ตเดียวกัน — เลือกอย่างใดอย่างหนึ่ง)

## เพิ่ม dependency

ถามเจ้าของโปรเจกต์ก่อนทุกครั้ง แล้ว `uv add <pkg>` (หรือ `uv add --dev <pkg>`) — ต้องมี wheel `manylinux` x86_64
สำหรับ cp314 เพราะ image ไม่มี compiler (กฎข้อ 16)

## ค่าที่ต้องรู้

- **keep-alive:** gunicorn `keepalive 120` ต้องนานกว่า pool idle ของ client ฝั่ง backend (reqwest ปริยาย 90 วินาที)
  ถ้าตั้ง `pool_idle_timeout` ของ backend ให้คงลำดับนี้ไว้ ไม่งั้น backend จะใช้ socket ที่ chatbot ปิดไปแล้ว → 502 เป็นระยะ
- **timeout:** gunicorn `timeout 120` ไว้ฆ่า worker ที่ event loop ค้าง ไม่ใช่เพดานต่อ request — เพดานเรียกออกนอกคือ
  `CHATBOT_HTTP_TIMEOUT_SECONDS` (httpx)
- **หน่วยความจำ:** container `mem_limit 512m`, 2 worker; `max_requests 1000` เริ่ม worker ใหม่กันหน่วยความจำโตสะสม

## งานถัดไป (ยังไม่ทำ)

1. **proxy ใน backend Rust** — โมดูล `chat/` (สิทธิ์ `chat.use` + migration เมนู/สิทธิ์, audit, ส่งต่อไป `http://chatbot:8000`)
   ถ้าตอบแบบ streaming ต้องเปิด feature `stream` ของ reqwest และคิดเพดานแชตพร้อมกัน (กิน connection ของ backend)
2. **ยืนยันตัวผู้เรียก** (shared secret header ระหว่าง backend ↔ chatbot) ถ้าวันหนึ่งมี service อื่นอยู่วง `chat`
3. **เปิด DB** — role DB แยกของ chatbot สร้างแบบเดียวกับ `my_his_sql_reader` (`backend/src/sql_console/setup.rs`) grant เฉพาะ
   ตาราง/คอลัมน์ที่ต้องใช้ ห้ามคอลัมน์ผู้ป่วยที่เข้ารหัส → ต่อ service `chatbot` เข้าวง `data` และส่ง `CHATBOT_DATABASE_URL`
   ผ่าน `environment:` ใน `docker-compose.yml` → เพิ่มชื่อตัวแปรใน `SECRET_NAMES` ของ `ops/check-deploy.sh` ตามความเหมาะสม
   ตารางใหม่ (เช่น บทสนทนา) ทำผ่าน migration ของ backend + data dict เท่านั้น
4. **เลือกผู้ให้บริการ LLM** + กฎคัดกรองข้อมูลผู้ป่วยก่อนส่งออก + เพดานความยาวข้อความ
5. `fastapi.testclient` เตือน `StarletteDeprecationWarning` (starlette อยากให้ใช้แพ็กเกจ `httpx2`) — ยังไม่เปลี่ยน รอตัดสินใจเรื่อง dependency
