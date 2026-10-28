//! ผูก path กับ handler (ห้ามมีตรรกะธุรกิจ) — Router ตรวจแค่ว่า login แล้วหรือยัง (CurrentUser
//! extractor ทำให้อัตโนมัติ) ส่วนตรวจสิทธิ์ `item.*` จริงอยู่ที่ service (docs/auth-design.md ข้อ 10)
//! `/items/options` เป็น path คงที่ จึงมาก่อน `/items/{id}` ได้ — แก้ไขเป็น PUT แทนที่ทั้งชุด (ฟอร์มส่งหน่วยแปลงมาครบ)

use axum::routing::{get, put};
use axum::Router;

use crate::item::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/items", get(handlers::list).post(handlers::create))
        .route("/items/options", get(handlers::options))
        .route(
            "/items/{id}",
            put(handlers::replace).delete(handlers::delete),
        )
}
