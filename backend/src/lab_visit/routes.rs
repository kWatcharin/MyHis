//! ผูก path กับ handler (ห้ามมีตรรกะธุรกิจ) — Router ตรวจแค่ว่า login แล้วหรือยัง (CurrentUser
//! extractor ทำให้อัตโนมัติ) ส่วนตรวจสิทธิ์ `lab_visit.*` จริงอยู่ที่ service
//! (docs/auth-design.md ข้อ 10) `/lab-visit-groups/options` เป็น path คงที่ จึงอยู่คู่กับ
//! `/lab-visit-groups/{id}` ได้ (path คงที่ชนะ path ที่มีตัวแปร)

use axum::routing::{get, post, put};
use axum::Router;

use crate::lab_visit::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/lab-visit-groups",
            get(handlers::list).post(handlers::create),
        )
        .route("/lab-visit-groups/options", get(handlers::options))
        .route(
            "/lab-visit-groups/{id}",
            get(handlers::get)
                .put(handlers::replace)
                .delete(handlers::delete),
        )
        .route("/lab-visit-groups/{id}/visits", post(handlers::add_visits))
        .route(
            "/lab-visit-groups/{id}/exports",
            post(handlers::record_export),
        )
        .route(
            "/lab-visits/{id}",
            put(handlers::replace_visit).delete(handlers::delete_visit),
        )
}
