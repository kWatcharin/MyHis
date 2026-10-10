//! ผูก path กับ handler (ห้ามมีตรรกะธุรกิจ) — สาธารณะทั้งคู่ (ไม่ต้อง login) ใช้ POST เพื่อให้ token อยู่ใน body
//! ไม่ไปอยู่ใน URL ที่ TraceLayer/Caddy log ได้ — endpoint ทั้งหมด: docs/auth-design.md ข้อ 5

use axum::routing::post;
use axum::Router;

use crate::registration::handlers;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/registration/lookup", post(handlers::lookup))
        .route("/registration/complete", post(handlers::complete))
}
