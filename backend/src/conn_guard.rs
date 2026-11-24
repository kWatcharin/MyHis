//! ด่านระดับ connection ของ backend: จำกัดจำนวน connection พร้อมกัน และตัด connection ที่เงียบ/ค้างเขียนนานเกินกำหนด
//! (docs/infrastructure.md ข้อ 6)
//!
//! `axum::serve` ไม่มี option timeout เลย — ผู้ที่เปิด connection แล้วเงียบถือ socket ไว้ได้ไม่จำกัด ห่อ `Listener` ไว้ตรงนี้
//! ใช้แค่ tokio ที่มีอยู่ (ไม่ต้องเพิ่ม `hyper-util`):
//! - `GuardedListener` ขอ permit **ก่อน** accept — ครบ cap แล้วไม่ accept เพิ่ม ที่เหลือรออยู่ใน backlog ของ kernel (ไม่กิน FD)
//! - `GuardedIo` ตั้ง deadline เดียวต่อ connection รีเซ็ตทุกครั้งที่อ่าน/เขียนคืบหน้า; ถ้ามี op ค้าง (Pending) จนเลย deadline
//!   คืน `TimedOut` — hyper ปิด connection และ permit คืนเมื่อ io ถูก drop
//!
//! ขีดจำกัด: แยก "ว่างระหว่าง request" กับ "ค้างกลาง header" ไม่ได้ (ต้องใช้ header_read_timeout ของ hyper) คนส่ง header ทีละ
//! ไบต์ถี่ ๆ จึงถือ connection ได้จนกว่า Caddy ชั้นหน้า (`read_header 10s`) จะตัด — backend ต้องไม่เปิดให้เข้าตรงโดยไม่ผ่าน Caddy
//!
//! request ที่ส่ง header ครบแล้วค้างที่ body (ถือ permit ไว้) ถูกตัดโดย `RequestBodyTimeoutLayer` ใน `app.rs` (3 วินาทีต่อการอ่าน)
//! และ `TimeoutLayer` 10 วินาที — permit เป็น pool ร่วมของทุก client (peer คือ Caddy ชั้นในเสมอ) ไม่มีโควตาต่อ client จึงต้องเปิด
//! Cloudflare + firewall ให้ origin รับเฉพาะ Cloudflare (docs/infrastructure.md หัวข้อ Cloudflare)
//!
//! **idle ต้องมากกว่า idle ของ upstream connection ใน Caddy** (ค่าปริยาย 2 นาที) เพื่อให้ Caddy ปิด keep-alive ก่อนเสมอ ไม่งั้น
//! backend อาจปิดตอนที่ Caddy เพิ่งส่งคำขอ (POST) เข้ามา แล้ว Caddy ตอบ 502

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::serve::Listener;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::{Instant, Sleep};

/// connection พร้อมกันสูงสุด — peer มีแค่ Caddy ชั้นใน ซึ่งจำกัดไว้ที่ `max_conns_per_host 64` ต่อ `reverse_proxy` 2 ตัว
/// (frontend/Caddyfile) รวมไม่เกินค่านี้พอดี ต้องแก้คู่กัน เกินนี้รอใน backlog
/// เพดานนี้คุม body ที่ buffer พร้อมกันด้วย: 128 × 256 KiB (`app.rs` MAX_BODY_BYTES) = 32 MiB (docs/memory-optimization.md ข้อ 4.3)
pub const MAX_CONNECTIONS: usize = 128;

/// เวลาที่ connection ไม่มีความคืบหน้าเลยก่อนถูกตัด — ต้องมากกว่า idle ของ upstream ใน Caddy (ดูหัวไฟล์)
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

pub struct GuardedListener<L> {
    inner: L,
    slots: Arc<Semaphore>,
    idle: Duration,
}

impl<L: Listener> GuardedListener<L> {
    pub fn new(inner: L, max_connections: usize, idle: Duration) -> Self {
        Self {
            inner,
            slots: Arc::new(Semaphore::new(max_connections)),
            idle,
        }
    }
}

impl<L: Listener> Listener for GuardedListener<L> {
    type Io = GuardedIo<L::Io>;
    type Addr = L::Addr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        // permit ก่อน accept: ครบ cap แล้วค้างตรงนี้ ไม่ดึง connection ใหม่ออกจาก backlog
        // semaphore นี้ไม่ถูกปิดตลอดอายุ listener (ถือโดย listener เอง) acquire จึงไม่มีทางล้มเหลว
        let permit = Arc::clone(&self.slots)
            .acquire_owned()
            .await
            .expect("connection semaphore is never closed");
        let (io, addr) = self.inner.accept().await;
        (GuardedIo::new(io, self.idle, permit), addr)
    }

    fn local_addr(&self) -> io::Result<Self::Addr> {
        self.inner.local_addr()
    }
}

pub struct GuardedIo<Io> {
    inner: Io,
    idle: Duration,
    deadline: Pin<Box<Sleep>>,
    _permit: OwnedSemaphorePermit,
}

impl<Io> GuardedIo<Io> {
    fn new(inner: Io, idle: Duration, permit: OwnedSemaphorePermit) -> Self {
        Self {
            inner,
            idle,
            deadline: Box::pin(tokio::time::sleep(idle)),
            _permit: permit,
        }
    }

    /// op คืบหน้า (Ready) → เริ่มนับ idle ใหม่
    fn touch(&mut self) {
        self.deadline.as_mut().reset(Instant::now() + self.idle);
    }

    /// op ยังค้าง (Pending) → เลย deadline แล้วให้ error ไม่งั้นรอต่อ (timer ปลุก task ให้มาเช็กอีกครั้ง)
    fn expire_if_idle<T>(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<T>> {
        match self.deadline.as_mut().poll(cx) {
            Poll::Ready(()) => Poll::Ready(Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "connection made no progress before the idle timeout",
            ))),
            Poll::Pending => Poll::Pending,
        }
    }

    /// ผลของ op หนึ่งครั้ง: Ready = คืบหน้า (รีเซ็ต deadline), Pending = ตรวจ deadline
    fn track<T>(
        &mut self,
        cx: &mut Context<'_>,
        polled: Poll<io::Result<T>>,
    ) -> Poll<io::Result<T>> {
        match polled {
            Poll::Ready(result) => {
                self.touch();
                Poll::Ready(result)
            }
            Poll::Pending => self.expire_if_idle(cx),
        }
    }
}

impl<Io: AsyncRead + Unpin> AsyncRead for GuardedIo<Io> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let polled = Pin::new(&mut this.inner).poll_read(cx, buf);
        this.track(cx, polled)
    }
}

impl<Io: AsyncWrite + Unpin> AsyncWrite for GuardedIo<Io> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let polled = Pin::new(&mut this.inner).poll_write(cx, data);
        this.track(cx, polled)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let polled = Pin::new(&mut this.inner).poll_write_vectored(cx, bufs);
        this.track(cx, polled)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let polled = Pin::new(&mut this.inner).poll_flush(cx);
        this.track(cx, polled)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        let polled = Pin::new(&mut this.inner).poll_shutdown(cx);
        this.track(cx, polled)
    }
}

#[cfg(test)]
mod tests {
    use std::future::poll_fn;

    use axum::routing::get;
    use axum::Router;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::time::{sleep, timeout};

    use super::*;

    const SLACK: Duration = Duration::from_secs(5);

    async fn guarded(
        cap: usize,
        idle: Duration,
    ) -> (GuardedListener<TcpListener>, std::net::SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        (GuardedListener::new(listener, cap, idle), addr)
    }

    /// อ่านหนึ่งครั้งจาก io (ไม่พึ่ง feature io-util ของ tokio)
    async fn read_some<Io: AsyncRead + Unpin>(io: &mut Io) -> io::Result<usize> {
        let mut storage = [0u8; 16];
        poll_fn(|cx| {
            let mut buf = ReadBuf::new(&mut storage);
            Pin::new(&mut *io)
                .poll_read(cx, &mut buf)
                .map_ok(|()| buf.filled().len())
        })
        .await
    }

    /// อ่านฝั่ง client จนกว่าอีกฝั่งจะปิด (Ok(0)) แล้วคืนทุกไบต์ที่ได้รับ
    async fn read_to_end(client: &TcpStream) -> Vec<u8> {
        let mut received = Vec::new();
        loop {
            client.readable().await.unwrap();
            let mut chunk = [0u8; 256];
            match client.try_read(&mut chunk) {
                Ok(0) => return received,
                Ok(n) => received.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(_) => return received,
            }
        }
    }

    #[tokio::test]
    async fn a_silent_connection_is_cut_after_the_idle_timeout() {
        let (mut listener, addr) = guarded(4, Duration::from_millis(150)).await;
        let _client = TcpStream::connect(addr).await.unwrap();
        let (mut io, _) = listener.accept().await;

        let started = std::time::Instant::now();
        let err = timeout(SLACK, read_some(&mut io))
            .await
            .expect("read must not hang")
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        assert!(
            started.elapsed() >= Duration::from_millis(140),
            "{:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn progress_resets_the_idle_timer() {
        let (mut listener, addr) = guarded(4, Duration::from_millis(500)).await;
        let client = TcpStream::connect(addr).await.unwrap();
        let (mut io, _) = listener.accept().await;

        // ส่งไบต์ทุก 150 ms รวม 900 ms (เกิน idle 500 ms ถ้าไม่ถูกรีเซ็ต) แล้วหยุด — idle กว้างไว้เผื่อเครื่องโหลดหนัก
        let writer = tokio::spawn(async move {
            for _ in 0..6 {
                sleep(Duration::from_millis(150)).await;
                client.writable().await.unwrap();
                client.try_write(b"x").unwrap();
            }
            client // ถือไว้ไม่ให้ปิด
        });
        let started = std::time::Instant::now();
        for _ in 0..6 {
            let n = timeout(SLACK, read_some(&mut io)).await.unwrap().unwrap();
            assert_eq!(n, 1);
        }
        assert!(
            started.elapsed() >= Duration::from_millis(800),
            "{:?}",
            started.elapsed()
        );
        let err = timeout(SLACK, read_some(&mut io))
            .await
            .unwrap()
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::TimedOut);
        drop(writer.await.unwrap());
    }

    #[tokio::test]
    async fn the_connection_cap_holds_back_accepts_until_one_closes() {
        let (mut listener, addr) = guarded(2, Duration::from_secs(60)).await;
        let _clients = [
            TcpStream::connect(addr).await.unwrap(),
            TcpStream::connect(addr).await.unwrap(),
            TcpStream::connect(addr).await.unwrap(),
        ];
        let (first, _) = timeout(SLACK, listener.accept()).await.unwrap();
        let (_second, _) = timeout(SLACK, listener.accept()).await.unwrap();

        // ตัวที่สามถูกกั้นไว้ที่ cap
        assert!(timeout(Duration::from_millis(200), listener.accept())
            .await
            .is_err());

        // connection แรกปิด → permit คืน → รับตัวที่สามได้
        drop(first);
        assert!(timeout(SLACK, listener.accept()).await.is_ok());
    }

    #[tokio::test]
    async fn serves_requests_and_closes_connections_that_stay_silent() {
        let (listener, addr) = guarded(8, Duration::from_millis(250)).await;
        let router = Router::new().route("/ping", get(|| async { "pong" }));
        let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });

        // คำขอปกติผ่าน guard ได้
        let client = TcpStream::connect(addr).await.unwrap();
        client.writable().await.unwrap();
        client
            .try_write(b"GET /ping HTTP/1.1\r\nHost: test\r\nConnection: close\r\n\r\n")
            .unwrap();
        let response =
            String::from_utf8(timeout(SLACK, read_to_end(&client)).await.unwrap()).unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(response.ends_with("pong"), "{response}");

        // connection ที่เปิดแล้วไม่ส่งอะไร ถูกฝั่ง server ปิด (อ่านได้ EOF)
        let silent = TcpStream::connect(addr).await.unwrap();
        let received = timeout(SLACK, read_to_end(&silent))
            .await
            .expect("server must close the silent connection");
        assert!(received.is_empty());

        server.abort();
    }
}
