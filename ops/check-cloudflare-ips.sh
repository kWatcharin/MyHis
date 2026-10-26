#!/bin/sh
# เทียบรายการ Cloudflare ใน ops/Caddyfile (trusted_proxies static …) กับรายการปัจจุบันของ Cloudflare
# ใช้ก่อน deploy และปีละครั้ง (docs/infrastructure.md หัวข้อ Cloudflare) — อ่านอย่างเดียว ไม่แก้อะไร
#
# ใช้: sh ops/check-cloudflare-ips.sh [path/to/Caddyfile]
# ผล: OK = ตรงกัน | FAIL = ต่างกัน (แสดงรายการที่ต่าง, exit 1) | SKIP = ดึงรายการไม่ได้ (ไม่ใช่ความผิดของ Caddyfile, exit 0)
#
# ถ้า FAIL: แก้บรรทัด trusted_proxies ใน ops/Caddyfile ให้ตรง (ตัวที่หายไปจาก Cloudflare ไม่ต้องรอ) แล้ว
# `docker compose up -d caddy` — เหลือ CIDR เก่าที่ Cloudflare เลิกใช้ = คนอื่นที่ได้ IP นั้นปลอม CF-Connecting-IP ได้
set -eu

here=$(cd "$(dirname "$0")" && pwd)
caddyfile=${1:-$here/Caddyfile}
urls="https://www.cloudflare.com/ips-v4 https://www.cloudflare.com/ips-v6"

if [ ! -f "$caddyfile" ]; then
  echo "FAIL ไม่พบไฟล์ $caddyfile"
  exit 1
fi

line=$(grep -E '^[[:space:]]*trusted_proxies[[:space:]]+static[[:space:]]' "$caddyfile" | head -n 1 || true)
if [ -z "$line" ]; then
  echo "FAIL ไม่พบบรรทัด 'trusted_proxies static …' ใน $caddyfile"
  exit 1
fi

listed=$(printf '%s\n' "$line" | sed -E 's/^[[:space:]]*trusted_proxies[[:space:]]+static[[:space:]]+//' | tr -s ' \t' '\n\n' | tr -d '\r' | grep -v '^$' | sort)

if ! command -v curl >/dev/null 2>&1; then
  echo "SKIP ไม่มี curl บนเครื่องนี้ ตรวจรายการ Cloudflare ไม่ได้"
  exit 0
fi

current=""
for url in $urls; do
  if ! part=$(curl -fsS --max-time 20 "$url" 2>/dev/null); then
    echo "SKIP ดึง $url ไม่ได้ (ไม่มีอินเทอร์เน็ต/ถูกบล็อก) — ลองใหม่ภายหลัง"
    exit 0
  fi
  current="$current
$(printf '%s\n' "$part" | tr -d '\r')"
done
current=$(printf '%s\n' "$current" | grep -v '^$' | sort)

if [ -z "$current" ]; then
  echo "SKIP Cloudflare ตอบรายการว่าง — ไม่ใช้ตัดสิน"
  exit 0
fi

# comm ต้องการไฟล์ที่เรียงแล้วสองไฟล์ — ใช้ไฟล์ชั่วคราว ลบทิ้งเสมอ
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
printf '%s\n' "$listed" >"$tmp/listed"
printf '%s\n' "$current" >"$tmp/current"
added=$(comm -13 "$tmp/listed" "$tmp/current")
removed=$(comm -23 "$tmp/listed" "$tmp/current")

count=$(printf '%s\n' "$current" | wc -l | tr -d ' ')
if [ -z "$added" ] && [ -z "$removed" ]; then
  echo "OK   trusted_proxies ใน Caddyfile ตรงกับ Cloudflare ($count รายการ)"
  exit 0
fi

echo "FAIL trusted_proxies ใน Caddyfile ไม่ตรงกับ Cloudflare"
if [ -n "$added" ]; then
  echo "  Cloudflare มีแต่ Caddyfile ยังไม่มี (ต้องเพิ่ม):"
  printf '%s\n' "$added" | sed 's/^/    + /'
fi
if [ -n "$removed" ]; then
  echo "  Caddyfile มีแต่ Cloudflare ไม่ประกาศแล้ว (ต้องเอาออก):"
  printf '%s\n' "$removed" | sed 's/^/    - /'
fi
exit 1
