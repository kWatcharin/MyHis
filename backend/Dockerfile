# โครงร่างคร่าว ๆ — ยังไม่ทดสอบ build
# ต้องมี backend/.sqlx ที่อัปเดตแล้ว (cargo sqlx prepare) ก่อน build จริง
# ดู docs/infrastructure.md ข้อ 8

FROM rust:1 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/my-his-backend /usr/local/bin/my-his-backend
USER nobody
CMD ["my-his-backend"]
