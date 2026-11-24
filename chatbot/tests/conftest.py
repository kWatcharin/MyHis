import shutil
import subprocess
import sys
from collections.abc import Iterator
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from my_his_chatbot.config import Settings
from my_his_chatbot.main import create_app

CHATBOT_DIR = Path(__file__).resolve().parent.parent


def pytest_sessionstart(session: pytest.Session) -> None:
    """ตรวจ type ด้วย pyrefly ก่อนรันเทสทุกครั้ง (docs/python-guidelines.md ข้อ 18) — ไม่ผ่าน = ไม่รันเทส."""
    pyrefly = shutil.which("pyrefly", path=str(Path(sys.executable).parent))
    if pyrefly is None:
        pytest.exit("pyrefly not found in venv — run `uv sync` first", returncode=1)
    # คำสั่งและอาร์กิวเมนต์คงที่ทั้งหมด ไม่มีค่าจากผู้ใช้
    checked = subprocess.run([pyrefly, "check"], cwd=CHATBOT_DIR, check=False)  # noqa: S603
    if checked.returncode != 0:
        pytest.exit("pyrefly check failed — fix type errors before running tests", returncode=checked.returncode)


@pytest.fixture
def settings(monkeypatch: pytest.MonkeyPatch) -> Settings:
    """Settings ที่ไม่ขึ้นกับ chatbot/.env และ CHATBOT_* ใน shell ของคนรันเทส."""
    for name in ("CHATBOT_LOG_LEVEL", "CHATBOT_WORKERS", "CHATBOT_HTTP_TIMEOUT_SECONDS", "CHATBOT_DATABASE_URL"):
        monkeypatch.delenv(name, raising=False)
    return Settings(_env_file=None)


@pytest.fixture
def client(settings: Settings) -> Iterator[TestClient]:
    # with-block = รัน lifespan (สร้าง httpx client / pool) เหมือนตอนรันจริง
    with TestClient(create_app(settings)) as test_client:
        yield test_client
