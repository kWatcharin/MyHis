//! ผูก path กับ handler (ห้ามมีตรรกะธุรกิจ) — Router ตรวจแค่ว่า login แล้วหรือยัง (CurrentUser
//! extractor ทำให้อัตโนมัติ) ส่วนตรวจสิทธิ์ `item_unit.*` จริงอยู่ที่ service (docs/auth-design.md ข้อ 10)

use axum::routing::{get, patch};
use axum::Router;

use crate::item_unit::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/item-units", get(handlers::list).post(handlers::create))
        .route(
            "/item-units/{id}",
            patch(handlers::update).delete(handlers::delete),
        )
}
