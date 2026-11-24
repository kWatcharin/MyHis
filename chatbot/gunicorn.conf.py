"""ค่าของ gunicorn ใน container (Dockerfile: gunicorn -c /app/gunicorn.conf.py my_his_chatbot.main:app).

อ่าน CHATBOT_WORKERS ผ่าน Settings ตัวเดียวกับแอป (docs/python-guidelines.md ข้อ 15 — ไม่อ่าน os.environ เอง)
"""

from my_his_chatbot.config import Settings

_settings = Settings()

# ฟังทุก interface ใน container — ไม่ publish ออกนอกเครื่อง เข้าถึงได้จาก backend ผ่าน network `chat` เท่านั้น
bind = "0.0.0.0:8000"
worker_class = "uvicorn_worker.UvicornWorker"
workers = _settings.workers

# heartbeat: ฆ่า worker ที่ event loop ค้าง (บล็อก) นานเกินนี้ — **ไม่ใช่**เพดานเวลาต่อ request
# (request ที่ await LLM อยู่ไม่ทำให้ชนค่านี้; เพดานเรียกออกนอกคือ CHATBOT_HTTP_TIMEOUT_SECONDS ของ httpx)
timeout = 120
graceful_timeout = 30

# ส่งต่อเป็น timeout_keep_alive ของ uvicorn — ฝั่ง server ต้องปิด connection ที่ว่าง *ช้ากว่า* pool idle ของ client เสมอ
# (reqwest ฝั่ง backend ปริยาย 90s) ไม่งั้น backend หยิบ socket ที่ chatbot ปิดไปแล้วมาใช้ → 502/reset เป็นระยะ
# กฎเดียวกับ backend vs Caddy ใน docs/infrastructure.md ข้อ 1
keepalive = 120

# เริ่ม worker ใหม่หลังรับครบจำนวนนี้ (สุ่มเหลื่อม) กันหน่วยความจำโตสะสม — mem_limit ของ container 512m
max_requests = 1000
max_requests_jitter = 100

# heartbeat file ของ worker อยู่ใน tmpfs (ดิสก์ช้าทำให้ worker ถูกมองว่าค้าง)
worker_tmp_dir = "/dev/shm"  # noqa: S108 tmpfs ของ container ตามคำแนะนำของ gunicorn ไม่ใช่ไฟล์ชั่วคราวที่คนอื่นเขียนได้

accesslog = "-"
errorlog = "-"
loglevel = _settings.log_level
