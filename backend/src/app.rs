//! ประกอบ router ของทุกโมดูล + middleware กลาง — ที่เดียวที่รู้จัก router ของทุกโมดูล
//! (docs/rust-guidelines.md — โมดูลฟีเจอร์ไม่เรียกกันเอง, app.rs เป็นข้อยกเว้นที่รู้จักทุกโมดูล)
//!
//! /health อยู่นอก /api ตามธรรมเนียม healthcheck (docs/auth-design.md ข้อ 5 — ทุก path อื่นอยู่ใต้
//! /api) audit/ ยังไม่ implement รอบนี้ เลยยัง merge เข้ามาไม่ได้

use std::time::Duration;

use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .nest("/api", api_router())
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn api_router() -> Router<AppState> {
    Router::new()
        .merge(crate::auth::router())
        .merge(crate::users::router())
}

async fn health() -> &'static str {
    "ok"
}
