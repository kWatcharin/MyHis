# ต้องมี backend/.sqlx ที่อัปเดตแล้ว (cargo sqlx prepare) ก่อน build จริงเสมอ — ตั้งแต่มี query
# จริงใน src/auth/repository.rs แล้ว build ผ่าน SQLX_OFFLINE ด้านล่าง ไม่ต่อ DB ตอน build เลย
# ดู docs/infrastructure.md ข้อ 8
# Rust version ปักตรงตาม stable ล่าสุด ณ วันที่ตั้งค่า (กันยายน 2026) — อัปเดตตรงนี้เวลาจะขยับเวอร์ชัน
# ใช้ image เต็ม (ไม่ใช่ -slim) เพราะมี gcc/g++/make/pkg-config ติดมาให้แล้ว (เช็คจริงแล้ว) — บาง
# dependency ต้องใช้ตอน build เช่น ring ที่ rustls เรียกผ่าน sqlx ไม่ต้อง apt-get install เพิ่มเอง

FROM rust:1.98.1-bookworm AS builder
WORKDIR /app

# --- แคช dependency แยกชั้นจากโค้ดเรา ---
# copy แค่ manifest ก่อน แล้ว compile ด้วยโค้ดปลอม เพื่อให้ dependency ทั้งหมด (ส่วนที่กินเวลาตอน
# build จริง ๆ เช่น axum/sqlx/tokio) ถูก cache เป็นเลเยอร์แยก คีย์ด้วย Cargo.toml/Cargo.lock
# เท่านั้น แก้แค่โค้ดเราทีหลัง rebuild จะเร็วขึ้นมากเพราะเลเยอร์นี้ยังถูก reuse ได้
# (.dockerignore กัน target/ ไว้แล้ว — ต้องกันไว้ ไม่งั้น COPY . . รอบสองจะทับ cache นี้ทิ้ง)
#
# package นี้มีทั้ง lib (src/lib.rs) และ bin (src/main.rs) — ต้องปลอมทั้งคู่ ไม่ใช่แค่ main.rs
# เหมือนตัวอย่างทั่วไปที่มีแค่ bin เดียว เพราะ lib.rs ของเรามี query!/query_as! จริงที่ต้องมี
# DATABASE_URL หรือ .sqlx ตอน compile ถ้าไม่ปลอม lib.rs ด้วย ขั้นตอนนี้จะ compile lib จริงและพังทันที
# --locked: Cargo.lock ต้องตรงกับ Cargo.toml เสมอ — ถ้าไม่ตรง build ล้ม แทนที่จะ resolve เวอร์ชัน dependency ใหม่เงียบ ๆ ตอน build
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs
RUN cargo build --release --locked
# ลบ artifact ของทั้ง bin และ lib ปลอม (ทั้งใน deps/ และตัว link ด้านนอกของ bin) ไม่แตะ .rlib ของ
# dependency ตัวอื่นที่ cache ไว้ — กันเคส cargo คิดว่า target ของเรายัง fresh อยู่แล้วข้าม compile
# จริงในรอบถัดไป หมายเหตุ: cargo แปลง `-` ในชื่อ package เป็น `_` ในชื่อไฟล์ใต้ deps/
RUN rm -f target/release/deps/my_his_backend-* \
    target/release/deps/libmy_his_backend-* \
    target/release/my-his-backend

# เอาโค้ดจริงทับโค้ดปลอม (รวม .sqlx/) แล้ว build อีกรอบ — dependency ที่ compile ไว้ข้างบน reuse
# ได้ทันที เหลือแค่ compile lib+bin ของเราเอง SQLX_OFFLINE=true ให้ query!/query_as! อ่านจาก .sqlx
# แทนการต่อ DB จริง (ไม่มี DB ให้ต่อตอน build อยู่แล้ว)
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --locked

# --- Runtime ---
FROM debian:bookworm-slim

# ca-certificates เผื่อ backend เรียก HTTPS ออกนอกในอนาคต — ไม่ copy /etc/ssl/certs จาก builder
# ซ้ำแบบตัวอย่างที่อ้างอิงมา เพราะ apt-get install ca-certificates ในสเตจนี้ให้ certs ครบอยู่แล้ว
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/my-his-backend /usr/local/bin/my-his-backend
USER nobody
EXPOSE 8080
CMD ["my-his-backend"]
