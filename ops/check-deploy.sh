#!/bin/sh
# ตรวจว่า stack ที่รันอยู่ตรงกับที่ docs/infrastructure.md และ docs/security-audit.md (ข้อ 5) วางไว้ — อ่านอย่างเดียว ไม่แก้อะไร
# ใช้หลัง `docker compose up -d` บนเครื่องจริง และหลังขยับ compose/Caddyfile
#
# ใช้: sh ops/check-deploy.sh [ชื่อ compose project]   (ไม่ระบุ = COMPOSE_PROJECT_NAME หรือชื่อโฟลเดอร์ของ repo)
# ผลแต่ละข้อ: OK | WARN (ควรรู้ ไม่ถึงกับผิด) | FAIL (ผิดจากที่วางไว้) | SKIP (ตรวจไม่ได้/ไม่เกี่ยว)  exit 1 เมื่อมี FAIL
#
# ไม่พิมพ์ค่าใด ๆ จาก .env หรือ environment ของ container — แสดงเฉพาะ ‘ชื่อ’ ตัวแปร (และชื่อโดเมนที่ไม่ใช่ความลับ)
# ไม่ใช่การรับรองความปลอดภัย: ข้อที่ต้องทำบนเครื่อง/Cloudflare จริงพิมพ์เป็นรายการมือท้ายสุด
set -eu
MSYS_NO_PATHCONV=1
export MSYS_NO_PATHCONV

if ! command -v docker >/dev/null 2>&1; then
  echo "FAIL ไม่พบคำสั่ง docker"
  exit 1
fi

here=$(cd "$(dirname "$0")/.." && pwd)
default_project=$(basename "$here" | tr 'A-Z' 'a-z' | tr -cd 'a-z0-9_-')
project=${1:-${COMPOSE_PROJECT_NAME:-$default_project}}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fails=0
warns=0
ok() { printf 'OK   %s\n' "$*"; }
skip() { printf 'SKIP %s\n' "$*"; }
warn() { warns=$((warns + 1)); printf 'WARN %s\n' "$*"; }
fail() { fails=$((fails + 1)); printf 'FAIL %s\n' "$*"; }
section() { printf '\n== %s\n' "$*"; }

# id ของ container ที่รันอยู่ของ service (ว่าง = ไม่ได้รัน)
cid() {
  docker ps --filter "label=com.docker.compose.project=$project" \
    --filter "label=com.docker.compose.service=$1" --format '{{.ID}}' | head -n 1
}
# ชื่อตัวแปร environment ของ container (ตัดค่าทิ้งทันที ไม่เก็บ/ไม่พิมพ์ค่า)
env_names() {
  docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$1" | cut -d= -f1 | grep -v '^$' | sort
}
# ชื่อ network ที่ container ต่ออยู่
nets_of() {
  docker inspect -f '{{range $k, $v := .NetworkSettings.Networks}}{{println $k}}{{end}}' "$1" | grep -v '^$' | sort
}

SECRET_NAMES='DATA_ENCRYPTION_KEY BLIND_INDEX_KEY DATABASE_URL SQL_CONSOLE_DB_PASSWORD TURNSTILE_SECRET_KEY INITIAL_ADMIN_PASSWORD'

printf 'compose project: %s\n' "$project"
caddy=$(cid caddy)
frontend=$(cid frontend)
backend=$(cid backend)
db=$(cid db)
chatbot=$(cid chatbot)

section "1. container และ healthcheck"
for svc in db backend chatbot; do
  id=$(cid "$svc")
  if [ -z "$id" ]; then
    fail "$svc ไม่ได้รัน"
    continue
  fi
  health=$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$id")
  if [ "$health" = healthy ]; then ok "$svc healthy"; else fail "$svc healthcheck = $health (ต้อง healthy)"; fi
done
if [ -z "$frontend" ]; then fail "frontend ไม่ได้รัน"; else ok "frontend รันอยู่"; fi
if [ -z "$caddy" ]; then
  skip "caddy ไม่ได้รัน (dev เปิดผ่าน frontend ตรง ๆ) — บนเครื่องจริงต้องรัน"
else
  ok "caddy รันอยู่"
fi

section "2. พอร์ตที่ publish (เฉพาะ caddy 80/443 ออกนอกเครื่อง)"
for svc in caddy frontend backend db chatbot; do
  id=$(cid "$svc")
  [ -n "$id" ] || continue
  ports=$(docker port "$id" 2>/dev/null || true)
  if [ -z "$ports" ]; then
    if [ "$svc" = caddy ]; then fail "caddy ไม่ publish พอร์ตใดเลย"; else ok "$svc ไม่ publish พอร์ต"; fi
    continue
  fi
  printf '%s\n' "$ports" >"$tmp/ports"
  flagged=0
  # อ่านจากไฟล์ (ไม่ใช่ pipe) เพื่อให้ fail/warn นับในเชลล์หลัก
  while IFS= read -r line; do
    addr=${line##* -> }
    host=${addr%:*}
    hport=${addr##*:}
    case "$svc:$hport" in
      caddy:80 | caddy:443) ;;
      *)
        flagged=1
        case "$host" in
          127.0.0.1 | '[::1]') warn "$svc publish $addr (loopback — ถูกต้องสำหรับ docker-compose.override.yml ของ dev เท่านั้น)" ;;
          *) fail "$svc publish $addr ออกนอกเครื่อง" ;;
        esac
        ;;
    esac
  done <"$tmp/ports"
  if [ "$flagged" -eq 0 ]; then ok "$svc publish เฉพาะ 80/443"; fi
done

section "3. network (caddy ต้องไม่เห็น backend, db, chatbot — frontend ต้องไม่เห็น chatbot)"
if [ -z "$caddy" ]; then
  skip "ไม่มี caddy จึงไม่มีเส้นทางจากอินเทอร์เน็ต"
else
  caddy_nets=$(nets_of "$caddy")
  shared=""
  for other in "$backend" "$db" "$chatbot"; do
    [ -n "$other" ] || continue
    for n in $(nets_of "$other"); do
      if printf '%s\n' "$caddy_nets" | grep -qx "$n"; then shared="$shared $n"; fi
    done
  done
  if [ -z "$shared" ]; then ok "caddy ไม่อยู่ network เดียวกับ backend/db/chatbot"; else fail "caddy อยู่ network เดียวกับ backend/db/chatbot:$shared"; fi
fi
# chatbot เรียกได้จาก backend เท่านั้น (backend ตรวจ session/สิทธิ์ก่อนส่งต่อ) — frontend เห็น chatbot = ข้ามด่าน auth ได้
if [ -n "$frontend" ] && [ -n "$chatbot" ]; then
  frontend_nets=$(nets_of "$frontend")
  shared=""
  for n in $(nets_of "$chatbot"); do
    if printf '%s\n' "$frontend_nets" | grep -qx "$n"; then shared="$shared $n"; fi
  done
  if [ -z "$shared" ]; then ok "frontend ไม่อยู่ network เดียวกับ chatbot"; else fail "frontend อยู่ network เดียวกับ chatbot:$shared"; fi
fi
if [ -n "$db" ]; then
  for n in $(nets_of "$db"); do
    internal=$(docker network inspect -f '{{.Internal}}' "$n" 2>/dev/null || echo unknown)
    if [ "$internal" = true ]; then
      ok "db อยู่ network $n (internal: ไม่มีทางออกเน็ต)"
    else
      warn "db อยู่ network $n ที่ไม่ internal (ถูกต้องเฉพาะ dev ที่ override เพิ่มให้; บนเครื่องจริง db ควรอยู่ data วงเดียว)"
    fi
  done
fi

section "4. ตัวแปร environment (ดูเฉพาะชื่อ)"
for svc in db caddy frontend chatbot; do
  id=$(cid "$svc")
  [ -n "$id" ] || continue
  names=$(env_names "$id")
  leaked=""
  for s in $SECRET_NAMES; do
    if printf '%s\n' "$names" | grep -qx "$s"; then leaked="$leaked $s"; fi
  done
  if [ -n "$leaked" ]; then fail "$svc เห็นตัวแปรลับ:$leaked"; else ok "$svc ไม่เห็นตัวแปรลับของแอป"; fi
done

section "5. เพดานทรัพยากร"
for svc in backend caddy frontend chatbot; do
  id=$(cid "$svc")
  [ -n "$id" ] || continue
  mem=$(docker inspect -f '{{.HostConfig.Memory}}' "$id")
  pids=$(docker inspect -f '{{.HostConfig.PidsLimit}}' "$id")
  case "$mem" in 0 | '' | '<nil>') fail "$svc ไม่มี mem_limit" ;; *) ok "$svc mem_limit = $((mem / 1048576)) MiB" ;; esac
  case "$pids" in 0 | -1 | '' | '<nil>') fail "$svc ไม่มี pids_limit" ;; *) ok "$svc pids_limit = $pids" ;; esac
done

section "6. log ของ backend (500 บรรทัดท้าย)"
if [ -z "$backend" ]; then
  skip "backend ไม่ได้รัน"
else
  logs=$(docker logs --tail 500 "$backend" 2>&1 || true)
  if printf '%s\n' "$logs" | grep -q 'non-public client address'; then
    if [ -n "$caddy" ]; then
      fail "backend เห็น IP ผู้ใช้เป็นที่อยู่ภายใน — สายพร็อกซี (Cloudflare → Caddy → frontend → backend) ผิด ทุกคนจะใช้เพดานล็อกอิน IP ร่วมกัน"
    else
      warn "backend เห็น IP ผู้ใช้เป็นที่อยู่ภายใน (ปกติบน dev ที่ไม่มี caddy/Cloudflare: เห็น gateway ของ Docker) — บนเครื่องจริงนี่คือ FAIL"
    fi
  else
    ok "ไม่มีคำเตือน non-public client address"
  fi
  if printf '%s\n' "$logs" | grep -q 'Turnstile uses Cloudflare test keys'; then
    warn "Turnstile ใช้ key ทดสอบของ Cloudflare — ล็อกอินไม่ได้รับการป้องกัน (ต้องใส่ key จริงก่อนใช้งานจริง)"
  elif printf '%s\n' "$logs" | grep -q 'Turnstile disabled'; then
    warn "Turnstile ปิดอยู่ (ไม่ได้ตั้ง key) — ต้องเปิดก่อนใช้งานจริง"
  else
    ok "ไม่มีคำเตือนเรื่อง Turnstile"
  fi
  if printf '%s\n' "$logs" | grep -q 'budget table is full'; then
    warn "ตารางนับล็อกอินเต็มเคยเกิดขึ้น — ดูว่ามีการสุ่มยิงจากหลาย IP หรือไม่"
  else
    ok "ตารางนับล็อกอินไม่เต็ม"
  fi
fi

section "7. ทางเข้าจากอินเทอร์เน็ต (Cloudflare / header)"
domain=""
if [ -n "$caddy" ]; then
  domain=$(docker inspect -f '{{range .Config.Env}}{{println .}}{{end}}' "$caddy" | sed -n 's/^DOMAIN=//p' | head -n 1)
fi
case "$domain" in
  '' | localhost) skip "ไม่มีโดเมนจริง (DOMAIN ว่าง/localhost) — ข้ามการตรวจ DNS และ header" ;;
  *)
    printf 'โดเมน: %s\n' "$domain"
    if command -v dig >/dev/null 2>&1; then
      aaaa=$(dig +short AAAA "$domain" 2>/dev/null | head -n 1 || true)
      if [ -z "$aaaa" ]; then
        ok "ไม่มี AAAA record (ผู้ใช้ IPv6 เข้าผ่าน Cloudflare ได้ตามปกติ)"
      else
        warn "มี AAAA record: ต้องแน่ใจว่าชี้ผ่าน Cloudflare (ส้ม) ไม่ใช่ IP ของ origin ตรง"
      fi
    else
      skip "ไม่มี dig — ตรวจ AAAA ไม่ได้"
    fi
    if command -v curl >/dev/null 2>&1; then
      headers=$(curl -sS --max-time 15 -D - -o /dev/null "https://$domain/api/auth/login-config" 2>/dev/null | tr -d '\r' | tr 'A-Z' 'a-z' || true)
      if [ -z "$headers" ]; then
        skip "เปิด https://$domain ไม่ได้จากเครื่องนี้"
      else
        if printf '%s\n' "$headers" | grep -q '^cf-ray:'; then
          ok "คำขอผ่าน Cloudflare (มี cf-ray)"
        else
          warn "ไม่มี header cf-ray — ไม่ได้ผ่าน Cloudflare (ถ้าตั้งใจใช้ Cloudflare: สถานะส้มปิดอยู่หรือ DNS ผิด)"
        fi
        if printf '%s\n' "$headers" | grep -q '^cache-control:.*no-store'; then
          ok "/api/auth/login-config: Cache-Control no-store"
        else
          fail "/api/auth/login-config ไม่มี Cache-Control: no-store (Cloudflare อาจ cache คำตอบ)"
        fi
        if printf '%s\n' "$headers" | grep -q '^content-security-policy:'; then
          ok "มี Content-Security-Policy"
        else
          fail "ไม่มี Content-Security-Policy"
        fi
      fi
    else
      skip "ไม่มี curl — ตรวจ header ไม่ได้"
    fi
    ;;
esac

section "สรุป"
printf 'FAIL %s, WARN %s\n' "$fails" "$warns"
cat <<'MANUAL'

ขั้นตอนมือที่สคริปต์ทำแทนไม่ได้ (docs/security-audit.md ข้อ 5):
  [ ] firewall ของเครื่อง: เปิดพอร์ต 80/443 เฉพาะช่วง IP ของ Cloudflare (ops/check-cloudflare-ips.sh เทียบรายการให้)
  [ ] Cloudflare: SSL/TLS = Full (strict), DNS เป็น A record สถานะส้ม
  [ ] ใส่ Turnstile key จริงใน .env (ไม่ใช่ key ทดสอบ)
  [ ] ล็อกอินจริงจากเครือข่ายที่เป็น IPv4 และจาก IPv6 แล้วดูหน้า ‘ประวัติการเข้าออก’ ว่า IP ตรงกับ IP จริงของเครื่องที่ใช้
MANUAL

[ "$fails" -eq 0 ]
