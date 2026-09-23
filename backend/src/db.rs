//! สร้าง connection pool, รันไมเกรชันตอนเริ่ม — ถ้า migration ล้มเหลว ต้องล้มทั้ง process
//! (docs/infrastructure.md ข้อ 4) ขนาด pool และ timeout ต้องตั้งชัดเจน (docs/rust-guidelines.md ข้อ 11)

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}

pub async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
