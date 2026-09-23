//! ผูก path กับ handler, กำหนดว่า route ไหนต้อง login (ห้ามมีตรรกะธุรกิจ)
//! Endpoint ทั้งหมด: docs/auth-design.md ข้อ 5 (/health ไม่ได้อยู่ที่นี่ — app.rs เป็นคนลงทะเบียนตรง
//! ที่ root ตามธรรมเนียม healthcheck)

use axum::routing::{get, post};
use axum::Router;

use crate::auth::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(handlers::login))
        .route("/auth/logout", post(handlers::logout))
        .route("/auth/logout-all", post(handlers::logout_all))
        .route("/auth/me", get(handlers::me))
        .route("/auth/password", post(handlers::change_password))
}
