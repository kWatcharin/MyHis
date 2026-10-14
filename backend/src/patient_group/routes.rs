//! ผูก path กับ handler (ห้ามมีตรรกะธุรกิจ) — Router ตรวจแค่ว่า login แล้วหรือยัง (CurrentUser
//! extractor ทำให้อัตโนมัติ) ส่วนตรวจสิทธิ์ `patient_group.*` จริงอยู่ที่ service (docs/auth-design.md ข้อ 10)

use axum::routing::{get, patch};
use axum::Router;

use crate::patient_group::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/patient-groups",
            get(handlers::list).post(handlers::create),
        )
        .route(
            "/patient-groups/{id}",
            patch(handlers::update).delete(handlers::delete),
        )
}
