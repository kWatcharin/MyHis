//! ผูก path กับ handler (ห้ามมีตรรกะธุรกิจ) — Router ตรวจแค่ว่า login แล้วหรือยัง (CurrentUser
//! extractor ทำให้อัตโนมัติ) ส่วนตรวจสิทธิ์ `stock_receipt.*` จริงอยู่ที่ service (docs/auth-design.md ข้อ 10)
//! `/stock/receipts/options` เป็น path คงที่ จึงมาก่อน `/stock/receipts/{id}` ได้ — แก้หัวใบเป็น PUT (ส่งครบสามช่อง)
//! ยกเลิกเป็น POST `…/cancel` เพราะไม่ใช่การลบแถว (ลง movement กลับรายการ + มีเหตุผล)

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;
use crate::stock::handlers;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/stock/receipts",
            get(handlers::list).post(handlers::create),
        )
        .route("/stock/receipts/options", get(handlers::options))
        .route(
            "/stock/receipts/{id}",
            get(handlers::detail).put(handlers::update),
        )
        .route("/stock/receipts/{id}/cancel", post(handlers::cancel))
        .route("/stock/lots", get(handlers::lots))
}
