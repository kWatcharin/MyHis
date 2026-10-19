//! ผูก path กับ handler (ห้ามมีตรรกะธุรกิจ) — Router ตรวจแค่ว่า login แล้วหรือยัง (CurrentUser extractor)
//! ส่วนตรวจสิทธิ์ `sql.view` จริงอยู่ที่ service (docs/auth-design.md ข้อ 10)

use axum::routing::{get, post};
use axum::Router;

use crate::sql_console::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sql/schema", get(handlers::schema))
        .route("/sql/query", post(handlers::query))
}
