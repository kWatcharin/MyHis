//! ผูก path กับ handler (ห้ามมีตรรกะธุรกิจ) — Router ตรวจแค่ว่า login แล้วหรือยัง (CurrentUser
//! extractor ทำให้อัตโนมัติ) ส่วนตรวจสิทธิ์ `patient.*` จริงอยู่ที่ service
//! (docs/auth-design.md ข้อ 10) `/patients/options` เป็น path คงที่ จึงมาก่อน `/patients/{id}` ได้

use axum::routing::{get, put};
use axum::Router;

use crate::patient::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/patients", get(handlers::list).post(handlers::create))
        .route("/patients/options", get(handlers::options))
        .route(
            "/patients/{id}",
            put(handlers::replace).delete(handlers::delete),
        )
}
