//! Thin entrypoint — logic จริงอยู่ใน lib.rs และโมดูลย่อย (แยกไว้เพื่อให้ backend/tests/ เรียก
//! router ตัวจริงได้ผ่าน crate lib แทนที่จะเรียก main.rs ซึ่งเป็นคนละ crate)
//! อนุญาต unwrap()/expect() เฉพาะที่นี่ตอนเริ่มระบบ (docs/rust-guidelines.md ข้อ 2)

use std::sync::Arc;

use my_his_backend::auth;
use my_his_backend::config::Config;
use my_his_backend::state::AppState;
use my_his_backend::turnstile::Turnstile;
use my_his_backend::{app, db, password};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::from_env().expect("invalid configuration");

    let pool = db::create_pool(&config.database_url)
        .await
        .expect("failed to connect to database");

    db::run_migrations(&pool)
        .await
        .expect("failed to run migrations");

    // hash คงที่ตอน startup ใช้ตอน login ไม่พบ username เพื่อความ timing-safe
    // (docs/auth-design.md ข้อ 6) — argon2 บล็อก thread จริง ต้องอยู่ใน spawn_blocking เสมอ
    let dummy_password_hash: Arc<str> = tokio::task::spawn_blocking(password::dummy_hash)
        .await
        .expect("dummy hash task panicked")
        .expect("failed to compute dummy password hash")
        .into();

    auth::bootstrap_first_admin(&pool, &config)
        .await
        .expect("failed to bootstrap first admin user");

    let turnstile = match &config.turnstile {
        Some(turnstile_config) => {
            let turnstile =
                Turnstile::new(turnstile_config).expect("failed to build Turnstile HTTP client");
            if turnstile.uses_test_key() {
                tracing::warn!("Turnstile uses Cloudflare test keys: login is not protected");
            }
            Some(Arc::new(turnstile))
        }
        None => {
            tracing::warn!("Turnstile disabled: TURNSTILE_SITE_KEY/TURNSTILE_SECRET_KEY not set");
            None
        }
    };

    let bind_addr = config.bind_addr;
    let state = AppState {
        pool,
        config: Arc::new(config),
        dummy_password_hash,
        turnstile,
    };

    let listener = tokio::net::TcpListener::bind(bind_addr)
        .await
        .expect("failed to bind address");

    tracing::info!(%bind_addr, "starting server");

    axum::serve(listener, app::router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server error");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
