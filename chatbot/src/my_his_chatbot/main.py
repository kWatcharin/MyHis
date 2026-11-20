"""ประกอบแอป FastAPI — ที่เดียวที่รู้จัก router ทุกตัว (เทียบ backend/src/app.rs).

gunicorn โหลด ``my_his_chatbot.main:app`` (gunicorn.conf.py); เทสเรียก ``create_app(Settings(...))`` เอง
"""

import logging
from collections.abc import AsyncGenerator
from contextlib import asynccontextmanager

import httpx
from fastapi import FastAPI

from my_his_chatbot import health
from my_his_chatbot.config import Settings
from my_his_chatbot.db import create_pool

logger = logging.getLogger(__name__)


def create_app(settings: Settings | None = None) -> FastAPI:
    resolved = settings if settings is not None else Settings()

    @asynccontextmanager
    async def lifespan(app: FastAPI) -> AsyncGenerator[None]:
        logging.basicConfig(level=resolved.log_level.upper())
        app.state.settings = resolved
        app.state.db = None
        async with httpx.AsyncClient(timeout=resolved.http_timeout_seconds) as http:
            app.state.http = http
            if resolved.database_url is None:
                logger.info("database not configured; connection pool disabled")
                yield
                return
            pool = create_pool(resolved.database_url.get_secret_value())
            await pool.open()
            app.state.db = pool
            try:
                yield
            finally:
                await pool.close()

    app = FastAPI(
        title="my-his chatbot",
        lifespan=lifespan,
        # service ภายใน (เรียกผ่าน backend เท่านั้น) — ไม่เปิดหน้าเอกสาร API
        docs_url=None,
        redoc_url=None,
        openapi_url=None,
    )
    app.include_router(health.router)
    return app


app = create_app()
