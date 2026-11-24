# กฎการเขียน Python (my-his)

เอกสารนี้คือมาตรฐานการเขียนโค้ดของโปรเจกต์ ใช้กับทุกไฟล์ใน `chatbot/`
ถ้าข้อไหนขัดกับความจำเป็นจริงหน้างาน ให้เขียนคอมเมนต์อธิบายเหตุผลไว้ตรงนั้น แล้วค่อยแหกกฎ
เลขข้อตรงกับ `docs/rust-guidelines.md` (ข้อ 6 = Async ทั้งสองภาษา) เพื่ออ้างข้ามภาษาได้
**ห้ามเลื่อนเลขข้อ** หัวข้อใหม่ให้ต่อท้าย

## 1. หลักการรวม

1. **ให้ type checker จับข้อผิดพลาดแทนคน** Python ไม่มี compiler แบบ Rust — pyrefly คือตัวแทน และต้องผ่านก่อนรันทุกครั้ง (ข้อ 18)
   ถ้าเลือกได้ระหว่างกฎที่ต้องจำกับชนิดข้อมูลที่ทำผิดแล้ว pyrefly ฟ้อง ให้เลือกอย่างหลังเสมอ
2. **ชัดเจนดีกว่าสั้น** โค้ดที่อ่านแล้วเข้าใจใน 30 วินาทีมีค่ามากกว่าโค้ดที่ฉลาด (ไม่ใช้ metaclass, decorator ซ้อน, monkey patch ในโค้ดจริง)
3. **อย่าสร้างนามธรรมก่อนมีผู้ใช้สองราย** `Protocol`, `ABC`, generic และ decorator ของตัวเองให้เพิ่มเมื่อมีของจริงสองแบบ

## 2. ห้ามพังเงียบ

- ห้าม `except:` เปล่า และห้าม `except Exception:` / `except BaseException:` ในโค้ดที่ทำงานตอนรับ request (ruff `BLE`)
  จับเฉพาะชนิดที่รู้ว่าจะเกิดและรู้ว่าจะรับมืออย่างไร
- ห้าม `except ...: pass` ถ้าตั้งใจข้ามจริง ให้ใช้ `contextlib.suppress(ชนิด)` พร้อมคอมเมนต์เหตุผล
- ห้ามใช้ `assert` ตรวจข้อมูลหรือสิทธิ์ตอนรันจริง — `python -O` ลบ `assert` ทิ้งทั้งหมด (ruff `S101`; ใช้ได้ในเทส)
- exception ที่หลุดถึง FastAPI = 500 และผู้ใช้ไม่ได้อะไรกลับไปเลย ให้แปลงเป็น error ที่ตั้งใจเสมอ (ข้อ 3)
- ไม่ต้อง `sys.exit()` กลางโค้ด ยกเว้นสคริปต์ CLI ใน `scripts/`

## 3. Error

- แต่ละโมดูลนิยาม exception ของตัวเองใน `service.py` / `repository.py` ให้บอกความหมายเชิงธุรกิจ เช่น `ConversationNotFound`
  ไม่ใช่โยน `psycopg.Error` หรือ `httpx.HTTPError` ขึ้นไปตรง ๆ
- แปลง exception ของโมดูลเป็นรหัสสถานะ HTTP ที่เดียว คือ exception handler ที่ลงทะเบียนใน `main.py` (เทียบ `AppError` ของ Rust)
- `HTTPException` ใช้ได้เฉพาะชั้น router และ `deps.py` — service/repository ห้ามรู้เรื่อง HTTP (ข้อ 7)
- ใช้ `raise ... from err` เมื่อแปลงชนิด exception เพื่อเก็บสาเหตุเดิมไว้ใน log (ruff `B904`)
- ข้อความที่ส่งกลับผู้ใช้กับข้อความใน log เป็นคนละอย่าง รายละเอียดทางเทคนิคอยู่ใน log เท่านั้น
- **error ต้องไม่บอกใบ้ข้อมูลที่ผู้โจมตีอยากรู้** ถ้าจะเพิ่ม endpoint ที่ตอบว่า "มี/ไม่มี" ให้คิดเรื่องนี้ก่อนเสมอ

## 4. ออกแบบด้วยชนิดข้อมูล

- **annotate ทุกฟังก์ชัน** ทั้งพารามิเตอร์และค่าที่คืน (ruff `ANN`) รวมถึงเทสและ fixture
- ห้าม `Any` ถ้าเลี่ยงไม่ได้ (เช่น JSON จากภายนอกก่อน parse) ให้คอมเมนต์เหตุผลและ parse เป็นโมเดลทันที
- `cast()` ใช้ได้เฉพาะขอบที่ library คืนชนิดหลวม (เช่น `request.app.state` ใน `deps.py`) — ไม่ใช้เพื่อปิดปาก pyrefly
- `# pyrefly: ignore` / `# type: ignore` ต้องระบุรหัส error และมีเหตุผลต่อท้ายบรรทัดเสมอ
- ใช้ `NewType` สำหรับ id เช่น `ConversationId = NewType("ConversationId", UUID)` เพื่อไม่ให้สลับ id ของคนละตารางกันได้
- ใช้ `Enum` (`StrEnum`) หรือ `Literal` แทนการส่ง string ไปมา ห้ามเทียบด้วย `role == "admin"`
- parse, don't validate: แปลงข้อมูลที่ไม่น่าเชื่อถือเป็นโมเดล pydantic ตั้งแต่ขอบระบบ (router) หลังจากนั้นโค้ดชั้นในไม่ต้องตรวจซ้ำ
- ถ้าสถานะหนึ่งเรื่องมี bool หลายตัวที่ต้องสอดคล้องกัน ให้เปลี่ยนเป็น enum
- `X | None` ต้องมีความหมายชัดเจน ถ้า `None` แปลว่า "ไม่พบ" ให้ repository คืน `None` และให้ service ตัดสินว่าเป็น error หรือไม่
- ใช้ไวยากรณ์ใหม่: `list[int]`, `X | None`, `type Alias = ...` (ruff `UP`) ไม่ใช้ `typing.List` / `Optional`

## 5. Mutability

- ห้าม mutable default argument (`def f(x: list[int] = [])`) (ruff `B006`)
- โมเดลโดเมนและ config เป็น immutable: pydantic `frozen=True` หรือ `@dataclass(frozen=True, slots=True)`
- ไม่แก้ object ที่รับเข้ามาเป็นอาร์กิวเมนต์ ให้คืนค่าใหม่แทน (`model_copy(update=...)`)
- ห้ามใช้ตัวแปรระดับโมดูลที่แก้ค่าได้เป็นที่เก็บ state ข้าม request — state ของ process อยู่ใน `app.state` ที่ lifespan สร้าง
  (gunicorn มีหลาย worker ค่าในหน่วยความจำไม่แชร์กันอยู่แล้ว)

## 6. Async

- **ห้ามงานที่บล็อกอยู่ใน `async def`** (ruff `ASYNC`): `time.sleep`, `requests`, เปิดไฟล์แบบ sync, งาน CPU หนัก
  ให้ใช้ของ async หรือ `await asyncio.to_thread(...)` — event loop ที่ค้างทำให้ทุก request ใน worker นั้นค้าง
  และ gunicorn ฆ่า worker ที่ค้างเกิน `timeout` (gunicorn.conf.py)
- **ทุกการเรียกออกนอก process ต้องมี timeout** — httpx client ใน lifespan ตั้ง `CHATBOT_HTTP_TIMEOUT_SECONDS` ไว้แล้ว
  pool ของ DB ตั้ง `POOL_TIMEOUT_SECONDS` (`db/pool.py`) ถ้าการเรียกไหนต้องสั้นกว่านั้นให้ส่ง `timeout=` เพิ่มที่จุดเรียก
- ใช้ `httpx.AsyncClient` ตัวเดียวจาก lifespan (`deps.get_http_client`) **ห้ามสร้าง client ใหม่ต่อ request** (เสีย connection pool/TLS ทุกครั้ง)
- งานหลายอย่างที่ไม่ขึ้นต่อกันให้ใช้ `asyncio.TaskGroup` แทนการ `await` เรียงกัน
- ห้าม `asyncio.create_task(...)` แล้วทิ้ง (task อาจถูก garbage collect กลางทางและ error หายเงียบ) ใช้ `TaskGroup`
  หรือเก็บ reference ไว้และปิดให้เรียบร้อยตอน shutdown
- handler ถูกยกเลิกกลางทางได้ (`CancelledError`) เมื่อ client ตัด connection — ห้ามจับ `CancelledError` แล้วกลืน
  งานที่ครึ่ง ๆ กลาง ๆ ไม่ได้ ให้ห่อด้วย `asyncio.shield` หรือทำใน transaction (ข้อ 11)
- ห้ามถือ `threading.Lock` คร่อม `await` ถ้าต้องใช้ lock ข้าม await ใช้ `asyncio.Lock` แต่ก่อนอื่นให้คิดว่าเลี่ยง lock ได้ไหม

## 7. โมดูลและการเปิดเผย

- **แบ่งตามฟีเจอร์** หนึ่งฟีเจอร์ = หนึ่งแพ็กเกจใต้ `src/my_his_chatbot/` เช่น `health/`, `chat/`
  ไฟล์มาตรฐาน: `router.py` → `service.py` → `repository.py` และ `schemas.py` (DTO)
- **ทิศทางการพึ่งพาเป็นทางเดียว** router → service → repository ห้ามย้อนกลับ
  `config.py`, `deps.py` (เฉพาะ router), `db/` เรียกใช้ได้ตามชั้นที่เหมาะสม
- **`service.py` และ `repository.py` ห้าม import `fastapi` / `starlette`** คืนค่าเป็นชนิดโดเมนหรือ exception ของโมดูล
- โมดูลฟีเจอร์ไม่เรียกกันเองข้ามโมดูล ถ้าต้องใช้ร่วมให้ย้ายลงโมดูลกลาง
- แต่ละฟีเจอร์เปิด `router` ตัวเดียว และ `main.py` (`create_app`) เป็นที่เดียวที่รู้จัก router ทั้งหมด
- `__init__.py` ของแต่ละฟีเจอร์เป็นหน้าร้าน ระบุสิ่งที่เปิดออกใน `__all__` ชัดเจน
- ห้าม `from x import *`
- ไฟล์ยาวเกินประมาณ 300 บรรทัด เป็นสัญญาณว่าควรแยก ไม่ใช่กฎตายตัว แต่ให้หยุดคิดทุกครั้งที่เกิน

## 8. การตั้งชื่อ

ตาม PEP 8

- `snake_case` สำหรับฟังก์ชัน ตัวแปร โมดูล, `CamelCase` สำหรับคลาส, `SCREAMING_SNAKE_CASE` สำหรับค่าคงที่
- ชื่อขึ้นต้น `_` = ภายในโมดูล ห้ามเรียกจากโมดูลอื่น
- ไม่ใส่คำนำหน้า `get_` ให้ property/เมธอดที่แค่อ่าน (ยกเว้น FastAPI dependency ใน `deps.py` ที่เป็นธรรมเนียมของ FastAPI)
- ชื่อฟังก์ชันบอกสิ่งที่ทำ ไม่ใช่วิธีที่ทำ — `archive_conversation()` ดีกว่า `update_status_column()`
- หลีกเลี่ยงตัวย่อที่ไม่ใช่คำที่ใช้กันทั่วไป

## 9. Pydantic และ DTO

- **แยก DTO ออกจากแถวของตาราง** แม้หน้าตาเหมือนกัน เพื่อไม่ให้คอลัมน์ใหม่รั่วออก API โดยไม่ตั้งใจ
- DTO ขาเข้าตั้ง `model_config = ConfigDict(extra="forbid")` พิมพ์ชื่อฟิลด์ผิดแล้วฟ้องทันที (เทียบ `deny_unknown_fields`)
- ความยาว/รูปแบบของฟิลด์ขาเข้าตรวจด้วย `Field(max_length=..., pattern=...)` ที่ DTO — ข้อความแชตต้องมี `max_length` เสมอ
- ความลับ (รหัสผ่าน, token, API key, URL ที่มีรหัส) ใช้ `SecretStr` เสมอ
- **ข้อความที่ผู้ใช้พิมพ์และคำตอบของ LLM อาจมีข้อมูลผู้ป่วย** ให้ใส่ `Field(repr=False)` กันหลุดผ่าน `repr()`/log
  และใส่เทสที่ `repr(model)` แล้วตรวจว่าไม่มีค่านั้นหลุดออกมา (ตัวอย่าง: `tests/test_config.py`)
- เปิด `model_dump()` เฉพาะฟิลด์ที่ตั้งใจ ห้ามส่ง object ของ DB ออก response ตรง ๆ

## 10. Comprehension

- ใช้ list/dict comprehension แทนลูปสะสมค่าด้วยมือ เมื่อมันอ่านง่ายกว่า
- ห้าม comprehension ซ้อนเกินสองชั้น หรือมี side effect ข้างใน — กลับไปใช้ `for` ธรรมดา ความอ่านง่ายสำคัญกว่า

## 11. ฐานข้อมูล (psycopg 3)

- **ห้ามประกอบ SQL ด้วย f-string, `%` หรือ `.format()`** พารามิเตอร์ผูกผ่าน `%s` / `%(name)s` เสมอ (ruff `S608`)
  ชื่อตาราง/คอลัมน์ที่ต้องเปลี่ยนตามค่า ใช้ `psycopg.sql.SQL` + `sql.Identifier` เท่านั้น
- แปลงแถวเป็นโมเดลด้วย `row_factory=class_row(Model)` ไม่ใช้ tuple index
- งานที่ต้องแก้หลายตารางให้ทำใน transaction เดียว (`async with conn.transaction():`)
- ระวัง N+1 ถ้าต้อง query ในลูป ให้เปลี่ยนเป็น query เดียวที่ join หรือใช้ `= any(%s)`
- ขนาด pool และ timeout ตั้งชัดเจนใน `db/pool.py` อย่าใช้ค่าปริยายโดยไม่รู้ว่าคืออะไร
- **ห้ามแก้ schema จาก Python** — migration ทั้งหมดอยู่ที่ `backend/migrations/` (sqlx) ที่เดียว chatbot ใช้ role DB ของตัวเอง
  ที่ได้สิทธิ์เฉพาะตาราง/คอลัมน์ที่ต้องใช้ (ดู `chatbot/README.md` งานถัดไป)
- ใช้คอลัมน์ `uuid` ระบุแถวเสมอ ไม่ส่ง id int ภายในออกไปนอก service (ข้อ 20)

## 12. JSON

- ชื่อฟิลด์ JSON ที่ backend เห็นเป็น camelCase ตามที่ตกลงกับ frontend: `alias_generator=to_camel`, `populate_by_name=True`
  และตอบด้วย `response_model_by_alias` (ค่าปริยายของ FastAPI)
- id ที่ส่งออกเป็น `uuid` เสมอ ห้ามส่ง id int ภายใน
- เวลาเป็น ISO 8601 พร้อม timezone (UTC) เสมอ (ข้อ 14)

## 13. Log

- ใช้ `logging` (`logger = logging.getLogger(__name__)`) ไม่ใช้ `print` (ruff `T20`; ยกเว้นสคริปต์ CLI พร้อม `noqa` + เหตุผล)
- `error` สำหรับสิ่งที่ต้องมีคนมาดู, `warning` สำหรับสิ่งผิดปกติที่ระบบรับมือได้แล้ว, `info` สำหรับเหตุการณ์สำคัญ, `debug` สำหรับตอนพัฒนา
- ใช้ argument ของ logger ไม่ใช่ f-string: `logger.info("pool opened size=%d", size)`
- **ห้าม log** prompt ที่ส่งไป LLM, คำตอบของ LLM, ข้อความแชตของผู้ใช้, ข้อมูลผู้ป่วย (plaintext หรือ ciphertext), รหัสผ่าน, token,
  API key, `CHATBOT_DATABASE_URL` ไม่ว่ากรณีใด รวมถึงตอนเกิด error — `logger.exception` พิมพ์ค่าตัวแปรใน traceback ได้
  ถ้า exception อาจถือข้อความแชต ให้ log แค่ชนิดของ exception
- ถ้าต้องวัดผลการเรียก LLM ให้ log แค่ metadata (เวลา, จำนวน token, รหัสสถานะ)

## 14. เวลา

- ใช้ `datetime` ที่มี timezone เสมอ: `datetime.now(UTC)` (ruff `DTZ`) ห้าม `datetime.now()` เปล่า และห้าม `datetime.utcnow()`
- เก็บและส่งเป็น UTC การแปลงเป็นเวลาไทยทำที่ชั้นแสดงผลเท่านั้น (เวรดึกคร่อมเที่ยงคืน)
- ฟังก์ชันที่ตรรกะขึ้นกับเวลา ให้รับเวลาเป็นพารามิเตอร์ — เรียก `datetime.now(UTC)` ที่ router แล้วส่งลงไป
  `service.py` และ `repository.py` ไม่เรียกนาฬิกาเอง

## 15. Config และค่าคงที่

- อ่าน environment variable ที่เดียวคือ `Settings` ใน `config.py` (pydantic-settings, prefix `CHATBOT_`)
- **ห้าม `os.environ` / `os.getenv` กระจายอยู่กลางโค้ด** (`gunicorn.conf.py` ก็อ่านผ่าน `Settings`)
- ถ้าค่าที่จำเป็นหายหรือผิดรูปแบบ ให้ล้มตั้งแต่ตอนเริ่ม (validation ของ pydantic) ไม่ใช่ตอนมี request เข้ามา
- ตัวแปรใหม่ต้องเพิ่มใน `chatbot/.env.example` และถ้าใช้ใน container ให้เพิ่มใน `environment:` ของ service `chatbot`
  ใน `docker-compose.yml` ทีละตัว — **ห้ามใช้ `env_file: .env`** ของรากโปรเจกต์ (มี key เข้ารหัสข้อมูลผู้ป่วย)
- ตัวเลขที่มีความหมาย (timeout, ขนาด pool, จำนวนข้อความสูงสุด) ต้องเป็นค่าคงที่ที่มีชื่อหรืออยู่ใน config พร้อมคอมเมนต์เหตุผล

## 16. Dependency

- **ถามก่อนเพิ่มแพ็กเกจใหม่ทุกครั้ง** (รวม extra ใหม่ของแพ็กเกจเดิม)
- เพิ่มด้วย `uv add <pkg>` / `uv add --dev <pkg>` เท่านั้น ห้ามใช้ `pip install` และห้ามแก้ `uv.lock` ด้วยมือ
- commit `uv.lock` ขึ้น git — Dockerfile ใช้ `uv sync --locked` (lock ไม่ตรงกับ pyproject = build ล้ม)
- เลือกแพ็กเกจที่มีคนใช้มาก ยังอัปเดตอยู่ และมี wheel `manylinux` สำหรับ Python เวอร์ชันที่ใช้ (image ไม่มี compiler)
- ถ้าเขียนเองได้ใน 30 บรรทัดและไม่ใช่เรื่องความปลอดภัย ให้เขียนเอง
- เรื่องที่เกี่ยวกับการเข้ารหัสหรือ hash **ห้ามเขียนเอง**

## 17. เทส

- เทสอยู่ใน `chatbot/tests/` ใช้ pytest — fixture กลางอยู่ใน `tests/conftest.py`
- ยิงผ่านแอปจริงด้วย `TestClient` ใน `with`-block (รัน lifespan) ได้ app จาก `create_app(settings)` ไม่ใช่ `main.app`
- เทสที่อ่าน config ใช้ fixture `settings` (ล้าง `CHATBOT_*` ด้วย `monkeypatch`) — ห้ามพึ่ง environment ของเครื่องคนรัน
- HTTP ขาออก (LLM API) จำลองด้วย `httpx.MockTransport` ห้ามยิงของจริงในเทส
- ชื่อเทสบอกพฤติกรรมที่คาดหวัง เช่น `test_empty_database_url_means_not_configured`
- ห้าม `sleep` ในเทสเพื่อรอเวลา ให้ฉีดเวลาเข้าไปเป็นพารามิเตอร์แทน (ข้อ 14)
- ทุก bug ที่เจอ ให้เขียนเทสที่ทำให้มันล้มก่อน แล้วค่อยแก้

## 18. เครื่องมือ

- **`pyrefly check` ต้องผ่านก่อนรันโค้ดทุกครั้ง** — บังคับไว้แล้วทุกทางที่รันได้:
  - dev server: `uv run python scripts/dev.py` (ตรวจก่อน แล้วค่อย start uvicorn `--reload` ที่ `127.0.0.1:8082`)
  - เทส: `uv run pytest` (`pytest_sessionstart` ใน `tests/conftest.py` ตรวจก่อน ไม่ผ่าน = ไม่รันเทส)
  - image: `Dockerfile` สั่ง `ruff check` + `pyrefly check` ใน builder stage ไม่ผ่าน = build ล้ม
  - **ห้ามรันโค้ด chatbot ทางอื่น** (`uv run uvicorn ...`, `python -m ...` ตรง ๆ) และห้ามแก้ด่านเหล่านี้ให้ข้ามได้
- `uv run ruff format` ก่อน commit เสมอ ใช้ค่ามาตรฐาน (line length 120)
- `uv run ruff check` ต้องผ่าน
- `# noqa` ต้องระบุรหัสกฎและเหตุผลต่อท้าย ห้าม `# noqa` เปล่า ห้ามปิดกฎระดับไฟล์ (ยกเว้น `per-file-ignores` ใน `pyproject.toml`)

## 19. สิ่งที่ยังไม่ต้องทำใน chatbot

- ไม่ใช้ ORM (SQLAlchemy ฯลฯ) — SQL เขียนเองผ่าน psycopg (ข้อ 11)
- ไม่มี cache, message queue, Celery — เหตุผลเดียวกับ CLAUDE.md
- ไม่ใช้ framework LLM ขนาดใหญ่ (LangChain, LlamaIndex ฯลฯ) จนกว่าจะตกลงกัน — เรียก API ผ่าน httpx ก่อน
- ไม่ทำ `Protocol`/abstraction ของ repository หรือผู้ให้บริการ LLM จนกว่าจะมีแบบที่สองจริง
- ไม่ปรับแต่งประสิทธิภาพก่อนวัด (ข้อยกเว้น: เพดานที่กันการโจมตี/กันเครื่องเต็ม เช่น ขนาดข้อความ, timeout, จำนวน worker)

## 20. ข้อมูลผู้ป่วยและความลับ

สรุปจาก CLAUDE.md หัวข้อ "ขอบเขตข้อมูล" ถ้าขัดกันให้ยึด CLAUDE.md

- chatbot **ไม่ได้รับ** key เข้ารหัส (`DATA_ENCRYPTION_KEY`, `BLIND_INDEX_KEY`) และไม่ถอดรหัสคอลัมน์ผู้ป่วยเอง
- ข้อมูลผู้ป่วยที่ chatbot เห็นมีเท่าที่ backend เลือกส่งมาเท่านั้น (backend เป็นด่านคัดกรอง — ทางเข้าแบบ A)
- **ห้ามส่งข้อมูลที่ระบุตัวผู้ป่วยได้ (HN, เลขประจำตัวคนไข้, ชื่อ, สกุล) ไปผู้ให้บริการ LLM ภายนอก**
- ห้ามเก็บบทสนทนาที่อาจมีข้อมูลผู้ป่วยลง DB จนกว่าจะออกแบบตารางร่วมกับ data dict (`docs/my-his-data-dict.xlsx`)
- role DB ของ chatbot ห้ามมีสิทธิ์อ่านคอลัมน์ผู้ป่วยที่เข้ารหัส
- ห้ามใส่ข้อมูลผู้ป่วย ข้อความแชต หรือความลับใน log, error message และ response ที่เป็น error
- ห้ามเก็บเลขบัตรประชาชน
