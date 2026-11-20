"""FastAPI dependency ที่ router ใช้ดึงของที่ lifespan สร้างไว้ (ใช้ได้เฉพาะชั้น router — ข้อ 7)."""

from typing import cast

import httpx
from fastapi import HTTPException, Request, status
from psycopg_pool import AsyncConnectionPool


def get_http_client(request: Request) -> httpx.AsyncClient:
    """httpx client ตัวเดียวของ process — ห้ามสร้างใหม่ต่อ request (ข้อ 6)."""
    return cast("httpx.AsyncClient", request.app.state.http)


def get_db_pool(request: Request) -> AsyncConnectionPool:
    pool = cast("AsyncConnectionPool | None", request.app.state.db)
    if pool is None:
        # ไม่ได้ตั้ง CHATBOT_DATABASE_URL — endpoint ที่ต้องใช้ DB ตอบ 503 แทนการล้มตอนเริ่ม
        raise HTTPException(status_code=status.HTTP_503_SERVICE_UNAVAILABLE, detail="database not configured")
    return pool
