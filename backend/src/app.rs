//! ประกอบ router ของทุกโมดูล + middleware กลาง — ที่เดียวที่รู้จัก router ของทุกโมดูล
//! (docs/rust-guidelines.md — โมดูลฟีเจอร์ไม่เรียกกันเอง, app.rs เป็นข้อยกเว้นที่รู้จักทุกโมดูล)
//!
//! /health อยู่นอก /api ตามธรรมเนียม healthcheck (docs/auth-design.md ข้อ 5 — ทุก path อื่นอยู่ใต้
//! /api)

use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Request};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use tower_http::timeout::{RequestBodyTimeoutLayer, TimeoutLayer};
use tower_http::trace::TraceLayer;

use crate::error::AppError;
use crate::state::AppState;

/// ขนาด body สูงสุดของทุก route (ไบต์) — ใหญ่สุดที่ใช้จริงคือ SQL 20,000 ตัวอักษร (≤ 80 KB) และอาร์เรย์ id ไม่เกิน 500 ตัว
/// ค่าปริยายของ axum คือ 2 MiB ซึ่งใส่อาร์เรย์ uuid ได้หลายหมื่นตัว
const MAX_BODY_BYTES: usize = 256 * 1024;

/// เวลาสูงสุดที่รอ body ก้อนถัดไป (ต่อการอ่าน ไม่ใช่รวม) — request ที่ส่ง header ครบแล้วเงียบ/ส่งทีละไบต์ถือ connection permit ของ
/// `conn_guard` ได้แค่นี้ ไม่ใช่เต็ม 10 วินาทีของ `TimeoutLayer` (ผู้โจมตีถือ permit ทั้ง 512 ได้ด้วย ~51 request/วินาที ที่ 10 วินาที)
/// body ปกติ ≤ 256 KiB มาทีเดียวแม้บนมือถือ จึงเผื่อ 3 วินาทีต่อก้อนเหลือเฟือ
const BODY_READ_TIMEOUT: Duration = Duration::from_secs(3);

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .nest("/api", api_router())
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(RequestBodyTimeoutLayer::new(BODY_READ_TIMEOUT))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            Duration::from_secs(10),
        ))
        .layer(middleware::from_fn(reject_cross_site))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// กัน CSRF ของคำขอที่เปลี่ยนสถานะ (ไม่ใช่ GET/HEAD/OPTIONS) ที่มาจากหน้าเว็บของ origin อื่น — โดยเฉพาะ POST ที่ไม่มี body
/// (reset-password, logout, logout-all) ซึ่ง Json extractor ไม่ได้ปฏิเสธ form ธรรมดา คุกกี้ `SameSite=Lax` กันได้แค่ข้าม site
/// ไม่กัน origin อื่นใน site เดียวกัน (subdomain/พอร์ตอื่นของโดเมนเดียวกัน) ตัดสินจาก header ที่เบราว์เซอร์ตั้งเอง ปลอมจากหน้าเว็บไม่ได้:
///
/// 1. มี `Sec-Fetch-Site` → ผ่านเฉพาะ `same-origin` (หน้าเว็บของเราเอง) กับ `none` (ผู้ใช้พิมพ์/กดเอง); `same-site` / `cross-site` → 403
/// 2. ไม่มี (เบราว์เซอร์เก่า / http ที่ไม่ใช่ localhost) แต่มี `Origin` → authority ต้องตรงกับ `Host` (`Origin: null` → 403)
/// 3. ไม่มีทั้งสองอย่าง (curl, สคริปต์ seed, เทส — ไม่ใช่เบราว์เซอร์ จึงไม่มีคุกกี้ลอยมากับคำขอข้ามต้นทาง) → ผ่าน
///
/// SPA เรียก `/api` แบบ same-origin เสมอ (frontend/Caddyfile, vite proxy) จึงไม่ต้องแก้ฝั่งหน้าเว็บ
async fn reject_cross_site(request: Request, next: Next) -> Result<Response, AppError> {
    if is_cross_site(request.method(), request.headers()) {
        return Err(AppError::Forbidden);
    }
    Ok(next.run(request).await)
}

fn is_cross_site(method: &Method, headers: &HeaderMap) -> bool {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return false;
    }
    if let Some(site) = headers.get("sec-fetch-site") {
        return !matches!(site.to_str(), Ok("same-origin" | "none"));
    }
    match headers.get(header::ORIGIN) {
        None => false,
        Some(origin) => !origin_matches_host(origin, headers.get(header::HOST)),
    }
}

/// `Origin` (เช่น `https://his.example.com`, `http://localhost:5173`) ตรงกับ `Host` ของคำขอ (ไม่สนตัวพิมพ์ใหญ่เล็ก) —
/// ทั้งสองค่าละพอร์ตมาตรฐานเหมือนกัน; ไม่เชื่อ `X-Forwarded-Host` เพราะผู้เรียกตั้งเองได้
fn origin_matches_host(origin: &HeaderValue, host: Option<&HeaderValue>) -> bool {
    let (Ok(origin), Some(Ok(host))) = (origin.to_str(), host.map(HeaderValue::to_str)) else {
        return false;
    };
    origin
        .split_once("://")
        .is_some_and(|(_, authority)| authority.eq_ignore_ascii_case(host))
}

fn api_router() -> Router<AppState> {
    Router::new()
        .merge(crate::auth::router())
        .merge(crate::users::router())
        .merge(crate::budget_year::router())
        .merge(crate::patient::router())
        .merge(crate::disease::router())
        .merge(crate::lab::router())
        .merge(crate::lab_visit::router())
        .merge(crate::patient_group::router())
        .merge(crate::nationality::router())
        .merge(crate::prename::router())
        .merge(crate::item_category::router())
        .merge(crate::item_unit::router())
        .merge(crate::item::router())
        .merge(crate::stock::router())
        .merge(crate::registration::router())
        .merge(crate::audit::router())
        .merge(crate::sql_console::router())
}

async fn health() -> &'static str {
    "ok"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, value.parse().unwrap());
        }
        map
    }

    #[test]
    fn safe_methods_are_never_blocked() {
        let hostile = headers(&[
            ("sec-fetch-site", "cross-site"),
            ("origin", "https://evil.test"),
        ]);
        for method in [Method::GET, Method::HEAD, Method::OPTIONS] {
            assert!(!is_cross_site(&method, &hostile), "{method}");
        }
    }

    #[test]
    fn fetch_metadata_decides_when_the_browser_sends_it() {
        for (site, blocked) in [
            ("same-origin", false),
            ("none", false),
            ("same-site", true),
            ("cross-site", true),
            ("", true),
            ("anything-else", true),
        ] {
            let map = headers(&[("sec-fetch-site", site), ("host", "his.example.com")]);
            assert_eq!(is_cross_site(&Method::POST, &map), blocked, "{site:?}");
        }
        // Sec-Fetch-Site ชนะ Origin: same-origin ที่ Origin แปลก (เช่น proxy เขียน Host ใหม่) ยังผ่าน
        let map = headers(&[
            ("sec-fetch-site", "same-origin"),
            ("origin", "http://localhost:5173"),
            ("host", "127.0.0.1:8081"),
        ]);
        assert!(!is_cross_site(&Method::POST, &map));
    }

    #[test]
    fn origin_must_match_host_when_there_is_no_fetch_metadata() {
        let host = "his.example.com";
        for (origin, blocked) in [
            ("https://his.example.com", false),
            ("https://HIS.example.com", false),
            ("https://other.example.com", true),
            ("https://his.example.com:8443", true),
            ("http://his.example.com.evil.test", true),
            ("null", true),
            ("", true),
        ] {
            let map = headers(&[("origin", origin), ("host", host)]);
            assert_eq!(is_cross_site(&Method::DELETE, &map), blocked, "{origin:?}");
        }
        // มี Origin แต่ไม่มี Host → เทียบไม่ได้ = ปฏิเสธ
        assert!(is_cross_site(
            &Method::POST,
            &headers(&[("origin", "https://his.example.com")])
        ));
    }

    #[test]
    fn requests_without_browser_headers_pass() {
        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            assert!(!is_cross_site(&method, &HeaderMap::new()), "{method}");
        }
    }
}
