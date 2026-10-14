//! ผูก path กับ handler (ห้ามมีตรรกะธุรกิจ) — Router ตรวจแค่ว่า login แล้วหรือยัง (CurrentUser
//! extractor ทำให้อัตโนมัติ) ส่วนตรวจสิทธิ์ `user.*` / `role.*` / `page.*` จริงอยู่ที่ service (docs/auth-design.md ข้อ 10)
//! Endpoint ทั้งหมด: docs/auth-design.md ข้อ 5

use axum::routing::{get, patch, post, put};
use axum::Router;

use crate::state::AppState;
use crate::users::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/users",
            get(handlers::list_users).post(handlers::create_user),
        )
        .route(
            "/users/{id}",
            patch(handlers::update_user).put(handlers::replace_user),
        )
        .route("/users/{id}/reset-password", post(handlers::reset_password))
        .route("/users/{id}/roles", put(handlers::replace_user_roles))
        .route(
            "/roles",
            get(handlers::list_roles).post(handlers::create_role),
        )
        .route(
            "/roles/{id}",
            patch(handlers::update_role).delete(handlers::delete_role),
        )
        .route(
            "/pages",
            get(handlers::list_pages).post(handlers::create_page),
        )
        .route(
            "/pages/{id}",
            patch(handlers::update_page).delete(handlers::delete_page),
        )
        .route("/menu", get(handlers::menu))
}
