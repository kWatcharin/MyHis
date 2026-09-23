# Build ด้วย bun แล้วเสิร์ฟ /dist แบบ static ผ่าน caddy (SPA — client-side routing ด้วย
# react-router ต้อง fallback ทุก path ที่ไม่ใช่ไฟล์จริงไปที่ index.html ดู Caddyfile)
# ไม่ publish พอร์ตออกนอกเครื่อง — caddy หลัก (ops/Caddyfile) เป็นคนเดียวที่ publish 80/443
# แล้ว reverse proxy เข้ามาที่นี่ผ่าน network ภายใน เหมือนที่ backend ทำอยู่แล้ว
#
# bun version ปักตรงตามที่ install ไว้บนเครื่อง dev ตอนตั้งค่า (กันยายน 2026) — อัปเดตตรงนี้เวลา
# จะขยับเวอร์ชัน ใช้ image เต็ม (ไม่ใช่ -alpine/-slim) เฉพาะ builder stage เพราะ tailwindcss v4
# ใช้ lightningcss ซึ่งมี native binding ต่อแพลตฟอร์ม — บาง alpine (musl) ไม่มี prebuilt binary
# ให้ตรง เสี่ยง build พังแบบเงียบ ๆ runtime stage ใช้ caddy:2-alpine ได้ปกติเพราะเป็น Go binary
# เดี่ยว ๆ ไม่มีปัญหานี้

FROM oven/bun:1.4.2 AS builder
WORKDIR /app

# --- แคช dependency แยกชั้นจากโค้ดเรา (เหมือน backend/Dockerfile) ---
# copy แค่ manifest ก่อน ให้ bun install ถูก cache เป็นเลเยอร์แยก คีย์ด้วย package.json/bun.lock
# เท่านั้น แก้แค่โค้ดเราทีหลัง rebuild จะเร็วขึ้นมากเพราะเลเยอร์นี้ยังถูก reuse ได้
# (.dockerignore กัน node_modules/ ไว้แล้ว — ต้องกันไว้ ไม่งั้น COPY . . รอบสองจะทับ cache นี้ทิ้ง)
COPY package.json bun.lock ./
RUN bun install --frozen-lockfile

COPY . .
RUN bun run build

# --- Runtime: เสิร์ฟไฟล์ static ด้วย caddy (ใช้ image เดียวกับ reverse proxy หลักของโปรเจกต์
# ไม่เพิ่มเทคโนโลยีใหม่ เช่น nginx เข้ามาในสแตกแค่เพื่องานนี้) ---
FROM caddy:2-alpine
COPY --from=builder /app/dist /usr/share/caddy
COPY Caddyfile /etc/caddy/Caddyfile
EXPOSE 80
