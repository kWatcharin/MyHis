# chatbot (Python/FastAPI) — service ภายใน เรียกผ่าน backend เท่านั้น (docs/infrastructure.md ข้อ 1–2)
# ปักเวอร์ชันตรงทั้ง python และ uv (ไม่ใช้ tag ลอย) แบบเดียวกับ rust/caddy — ขยับพร้อม .python-version และ uv บนเครื่อง dev
# ใช้ slim-trixie (Debian 13 = stable ปัจจุบัน) — dependency ที่ compile ทุกตัวมี wheel manylinux cp314 แล้ว ไม่ต้องมี compiler

FROM python:3.14.6-slim-trixie AS builder
COPY --from=ghcr.io/astral-sh/uv:0.11.24 /uv /bin/uv

# compile .pyc ตอน build (start เร็วขึ้น), copy แทน hardlink (cache mount อยู่คนละ filesystem), ห้าม uv โหลด Python เอง (ใช้ของ image)
ENV UV_COMPILE_BYTECODE=1 UV_LINK_MODE=copy UV_PYTHON_DOWNLOADS=0
WORKDIR /app

# --- แคช dependency แยกชั้นจากโค้ด: คีย์ด้วย pyproject.toml/uv.lock เท่านั้น ---
# ลง dev group ด้วย (ruff/pyrefly) เพื่อใช้เป็นด่านตรวจด้านล่าง; --locked = uv.lock ต้องตรงกับ pyproject ไม่งั้น build ล้ม
RUN --mount=type=cache,target=/root/.cache/uv \
    --mount=type=bind,source=uv.lock,target=uv.lock \
    --mount=type=bind,source=pyproject.toml,target=pyproject.toml \
    uv sync --locked --no-install-project

COPY . .

# --- ด่านตรวจ (docs/python-guidelines.md ข้อ 18): type ผิดหรือ lint ไม่ผ่าน = build ล้ม ไม่มี image ให้รัน ---
# ติดตั้งตัวโปรเจกต์ก่อน (pyrefly ต้อง resolve import my_his_chatbot ได้เหมือนบนเครื่อง dev)
RUN --mount=type=cache,target=/root/.cache/uv \
    uv sync --locked
RUN uv run --locked --no-sync ruff check \
    && uv run --locked --no-sync pyrefly check

# ตัด dev group ออกจาก venv (sync แบบ exact ลบแพ็กเกจที่ไม่อยู่ในชุด) และติดตั้งโปรเจกต์แบบไม่ editable
# ให้ runtime ไม่ต้องมี src/ — copy แค่ .venv ไปพอ
RUN --mount=type=cache,target=/root/.cache/uv \
    uv sync --locked --no-dev --no-editable

# --- Runtime ---
FROM python:3.14.6-slim-trixie

WORKDIR /app
COPY --from=builder /app/.venv /app/.venv
COPY gunicorn.conf.py /app/gunicorn.conf.py

ENV PATH="/app/.venv/bin:$PATH" \
    PYTHONUNBUFFERED=1 \
    PYTHONDONTWRITEBYTECODE=1

USER nobody
EXPOSE 8000
CMD ["gunicorn", "-c", "/app/gunicorn.conf.py", "my_his_chatbot.main:app"]
