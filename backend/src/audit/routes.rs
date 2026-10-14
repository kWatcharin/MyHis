//! ผูก path กับ handler (ห้ามมีตรรกะธุรกิจ) — Router ตรวจแค่ว่า login แล้วหรือยัง (CurrentUser
//! extractor ทำให้อัตโนมัติ) ส่วนตรวจสิทธิ์ `audit.view` จริงอยู่ที่ service (docs/auth-design.md ข้อ 10)

use axum::routing::get;
use axum::Router;

use crate::audit::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/audit/auth-events", get(handlers::auth_events))
        .route("/audit/activities", get(handlers::activities))
}
