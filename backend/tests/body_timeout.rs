//! request ที่ส่ง header ครบแล้วค้างที่ body ต้องถูกตัดภายในไม่กี่วินาที (`RequestBodyTimeoutLayer` ใน app.rs — 3 วินาทีต่อการอ่าน) ไม่ใช่รอเต็ม
//! 10 วินาทีของ `TimeoutLayer`: connection permit ของ `conn_guard` เป็น pool ร่วมของทุก client การถือ permit ไว้ได้นานเท่าไรเป็นตัวกำหนด
//! อัตราคำขอค้างที่ทำให้ pool เต็ม (security audit รอบ 2) — ทดสอบด้วยเซิร์ฟเวอร์จริง (GuardedListener + router ของแอป) บนซ็อกเก็ต loopback

mod common;

use std::time::{Duration, Instant};

use my_his_backend::conn_guard::{GuardedListener, IDLE_TIMEOUT};
use sqlx::PgPool;
use tokio::net::{TcpListener, TcpStream};

/// เซิร์ฟเวอร์ตัวจริงที่เปิดแค่ `max_connections` connection พร้อมกัน
async fn serve(pool: PgPool, max_connections: usize) -> std::net::SocketAddr {
    let app = common::test_app(pool).await;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let guarded = GuardedListener::new(listener, max_connections, IDLE_TIMEOUT);
    tokio::spawn(async move { axum::serve(guarded, app).await.unwrap() });
    addr
}

async fn write_all(stream: &TcpStream, bytes: &[u8]) {
    let mut sent = 0;
    while sent < bytes.len() {
        stream.writable().await.unwrap();
        match stream.try_write(&bytes[sent..]) {
            Ok(n) => sent += n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(e) => panic!("write failed: {e}"),
        }
    }
}

/// หัว request ที่บอก Content-Length: 100 แต่ไม่ส่ง body เลย (หรือส่งแค่บางส่วน)
async fn stalled_post(addr: std::net::SocketAddr) -> TcpStream {
    let stream = TcpStream::connect(addr).await.unwrap();
    write_all(
        &stream,
        b"POST /api/registration/lookup HTTP/1.1\r\nHost: test\r\nContent-Type: application/json\r\nContent-Length: 100\r\nConnection: close\r\n\r\n",
    )
    .await;
    stream
}

/// request ปกติที่ครบ — คำตอบเป็น 400 (ลิงก์ไม่ถูกต้อง) ทันที ไม่ต้องมีข้อมูลใน DB
async fn honest_post(addr: std::net::SocketAddr) -> TcpStream {
    let body = br#"{"token":"not-a-real-link"}"#;
    let head = format!(
        "POST /api/registration/lookup HTTP/1.1\r\nHost: test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let stream = TcpStream::connect(addr).await.unwrap();
    write_all(&stream, &[head.as_bytes(), body].concat()).await;
    stream
}

/// อ่านจนเซิร์ฟเวอร์ปิด (Connection: close) หรือครบเวลา — คืน (บรรทัดสถานะ, ปิดแล้วหรือยัง, เวลาที่ใช้)
async fn read_status(stream: &TcpStream, limit: Duration) -> (String, bool, Duration) {
    let started = Instant::now();
    let deadline = started + limit;
    let mut out: Vec<u8> = Vec::new();
    let closed = loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match tokio::time::timeout(remaining, stream.readable()).await {
            Err(_) => break false,
            Ok(Err(_)) => break true,
            Ok(Ok(())) => {}
        }
        let mut chunk = [0u8; 8192];
        match stream.try_read(&mut chunk) {
            Ok(0) => break true,
            Ok(n) => out.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => break true,
        }
    };
    let text = String::from_utf8_lossy(&out).to_string();
    (
        text.lines().next().unwrap_or("").to_string(),
        closed,
        started.elapsed(),
    )
}

#[sqlx::test]
async fn a_request_stalled_on_its_body_is_cut_in_seconds_and_frees_its_permit(pool: PgPool) {
    // เปิด connection พร้อมกันได้แค่ 2: ถ้า request ค้างถือ permit นาน คำขอปกติที่ตามมาต้องรอตามไปด้วย
    let addr = serve(pool, 2).await;

    let first = stalled_post(addr).await;
    let second = stalled_post(addr).await;
    tokio::time::sleep(Duration::from_millis(300)).await;

    // คำขอปกติเข้ามาตอนที่ permit ทั้งสองถูกถืออยู่ — ต้องได้คำตอบทันทีที่ permit แรกถูกคืน (ไม่ใช่หลัง 10 วินาที)
    let started = Instant::now();
    let honest = honest_post(addr).await;
    let (status, _closed, _) = read_status(&honest, Duration::from_secs(9)).await;
    let waited = started.elapsed();
    assert!(
        status.starts_with("HTTP/1.1 400"),
        "honest request should be answered: {status:?}"
    );
    assert!(
        waited < Duration::from_secs(6),
        "honest request waited {waited:?} behind stalled bodies (expected ≈ the 3 s body timeout)"
    );

    // และตัว request ที่ค้างถูกตัดด้วยสถานะ 4xx ภายในไม่กี่วินาที (ไม่ใช่เงียบจนครบ 10 วินาทีแล้วค่อย 408 ของ TimeoutLayer)
    let (status, closed, elapsed) = read_status(&first, Duration::from_secs(9)).await;
    assert!(closed, "stalled connection should be closed");
    assert!(status.starts_with("HTTP/1.1 4"), "{status:?}");
    assert!(
        elapsed < Duration::from_secs(7),
        "stalled body held the connection for {elapsed:?}"
    );
    let (status, closed, _) = read_status(&second, Duration::from_secs(9)).await;
    assert!(closed && status.starts_with("HTTP/1.1 4"), "{status:?}");
}
