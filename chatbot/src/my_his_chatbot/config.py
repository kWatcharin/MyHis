"""อ่าน environment variable ที่เดียวของ service (docs/python-guidelines.md ข้อ 15).

ตัวแปรทุกตัวขึ้นต้นด้วย ``CHATBOT_`` — compose ส่งเข้า container ผ่าน ``environment:`` ทีละตัว
ห้ามใช้ ``env_file: .env`` ของรากโปรเจกต์ เพราะไฟล์นั้นมี key เข้ารหัสข้อมูลผู้ป่วยและรหัสผ่าน DB
ตอนรันบนเครื่อง dev อ่าน ``chatbot/.env`` (git-ignore) เพิ่มได้ผ่าน python-dotenv ของ pydantic-settings
"""

from typing import Literal

from pydantic import Field, SecretStr
from pydantic_settings import BaseSettings, SettingsConfigDict

LogLevel = Literal["critical", "error", "warning", "info", "debug"]


class Settings(BaseSettings):
    model_config = SettingsConfigDict(
        env_prefix="CHATBOT_",
        env_file=".env",
        env_file_encoding="utf-8",
        extra="ignore",
        # ตัวแปรที่ตั้งเป็นค่าว่าง (CHATBOT_DATABASE_URL= ใน .env.example / compose) = ไม่ได้ตั้ง ใช้ค่าปริยาย
        env_ignore_empty=True,
        frozen=True,
    )

    log_level: LogLevel = "info"

    # จำนวน worker ของ gunicorn (gunicorn.conf.py อ่านค่านี้) — เครื่องจริง 2 vCPU / 4 GB
    workers: int = Field(default=2, ge=1, le=8)

    # เพดานเวลาของทุกการเรียก HTTP ขาออก (LLM API ฯลฯ) — gunicorn `timeout` ไม่ได้จำกัดเวลาต่อ request
    http_timeout_seconds: float = Field(default=30.0, gt=0, le=300)

    # ไม่ตั้ง = ไม่เปิด connection pool (รอบวางโครงยังไม่ต่อ network `data`) — ดู chatbot/README.md งานถัดไป
    # SecretStr: กันรหัสผ่านใน URL หลุดผ่าน repr/log
    database_url: SecretStr | None = None
