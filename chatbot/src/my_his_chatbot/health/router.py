"""Liveness probe ของ healthcheck ใน docker-compose.yml.

ตอบ 200 เมื่อ process ยังรับ request ได้เท่านั้น — ไม่ ping DB/LLM: ของภายนอกล่มต้องไม่ทำให้ container ถูกมองว่า
ไม่ healthy แล้ว restart วน
"""

from typing import Literal

from fastapi import APIRouter
from pydantic import BaseModel

router = APIRouter()


class HealthResponse(BaseModel):
    status: Literal["ok"]


@router.get("/health")
async def health() -> HealthResponse:
    return HealthResponse(status="ok")
