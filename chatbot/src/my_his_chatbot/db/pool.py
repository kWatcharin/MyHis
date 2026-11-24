"""Connection pool ของ PostgreSQL (psycopg 3 async) — docs/python-guidelines.md ข้อ 11."""

from psycopg_pool import AsyncConnectionPool

# ขนาด pool ตั้งชัด ไม่ใช้ค่าปริยาย: ต่อ worker (gunicorn 2 worker = สูงสุด 8 connection)
POOL_MIN_SIZE = 1
POOL_MAX_SIZE = 4
# รอ connection ว่างนานสุดก่อนล้ม — สั้นกว่า http_timeout_seconds เพื่อให้ request ล้มด้วยสาเหตุที่ถูกต้อง
POOL_TIMEOUT_SECONDS = 5.0


def create_pool(conninfo: str) -> AsyncConnectionPool:
    """สร้าง pool แบบยังไม่เปิด — ผู้เรียก (lifespan) ต้อง ``await pool.open()`` และ ``await pool.close()`` เอง."""
    return AsyncConnectionPool(
        conninfo,
        min_size=POOL_MIN_SIZE,
        max_size=POOL_MAX_SIZE,
        timeout=POOL_TIMEOUT_SECONDS,
        open=False,
    )
