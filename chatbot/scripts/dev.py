"""รัน dev server ของ chatbot — ตรวจ type ด้วย pyrefly ก่อนทุกครั้ง (docs/python-guidelines.md ข้อ 18).

วิธีใช้ (ใน chatbot/):  uv run python scripts/dev.py
pyrefly ไม่ผ่าน = ไม่ start server และออกด้วยรหัสของ pyrefly
"""

import shutil
import subprocess
import sys
from pathlib import Path

CHATBOT_DIR = Path(__file__).resolve().parent.parent
DEV_PORT = 8082  # ตรงกับพอร์ต dev ของ chatbot ใน docker-compose.override.example.yml


def venv_tool(name: str) -> str:
    """หา executable ใน venv เดียวกับ python ที่รันสคริปต์นี้ (ไม่พึ่ง PATH)."""
    found = shutil.which(name, path=str(Path(sys.executable).parent))
    if found is None:
        sys.exit(f"{name} not found in venv — run `uv sync` first")
    return found


def main() -> int:
    # คำสั่งและอาร์กิวเมนต์คงที่ทั้งหมด ไม่มีค่าจากผู้ใช้
    checked = subprocess.run([venv_tool("pyrefly"), "check"], cwd=CHATBOT_DIR, check=False)  # noqa: S603
    if checked.returncode != 0:
        print("pyrefly check failed — dev server not started", file=sys.stderr)  # noqa: T201 สคริปต์ CLI พิมพ์ถึงคนรันโดยตรง
        return checked.returncode
    served = subprocess.run(  # noqa: S603
        [
            venv_tool("uvicorn"),
            "my_his_chatbot.main:app",
            "--reload",
            "--host",
            "127.0.0.1",
            "--port",
            str(DEV_PORT),
        ],
        cwd=CHATBOT_DIR,
        check=False,
    )
    return served.returncode


if __name__ == "__main__":
    sys.exit(main())
