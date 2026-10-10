#!/usr/bin/env python3
"""เติมข้อมูลสัญชาติทั้งหมด (199 รายการ: สมาชิก UN 193 + ผู้สังเกตการณ์ 2 + ไต้หวัน โคโซโว ฮ่องกง มาเก๊า)
เข้า "ตั้งค่าสัญชาติ" ผ่าน API จริง (POST /api/nationalities) ด้วยบัญชี admin จาก .env

ทำไมไม่ใส่ใน migration: ตารางมีคอลัมน์ created_by/updated_by (FK ไป users) ซึ่งบน DB ใหม่ยังไม่มีผู้ใช้ตอน
migration รัน (admin คนแรกถูกสร้างตอนแอปเริ่มทำงาน) และผ่าน API จึงใช้กฎ/audit เดียวกับที่ผู้ใช้กรอกเอง

รันซ้ำได้ปลอดภัย (idempotent): ข้ามเชื้อชาติ/สัญชาติที่มีอยู่แล้ว (ไม่สนตัวพิมพ์ นับเฉพาะแถวที่ยังไม่ถูกลบ)
ไม่แก้/ไม่ลบแถวที่มีอยู่ ใช้งาน:  python ops/seed/seed_nationalities.py [--base-url http://127.0.0.1:3000/api]
ข้อมูลอยู่ใน ops/seed/nationalities.json (ชื่อ/ประเทศภาษาไทย แก้ไขภายหลังได้ที่หน้าตั้งค่าสัญชาติ)
ไม่พิมพ์รหัสผ่านหรือ cookie ลงเอาต์พุต"""
import argparse
import json
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def read_env(path: Path) -> dict:
    env = {}
    for line in path.read_text(encoding='utf-8').splitlines():
        m = re.match(r'^([A-Z_][A-Z0-9_]*)=(.*)$', line.strip())
        if m:
            env[m.group(1)] = m.group(2)
    return env


class Api:
    def __init__(self, base_url: str):
        self.base = base_url.rstrip('/')
        self.cookie = None

    def call(self, method: str, path: str, body=None):
        data = json.dumps(body).encode('utf-8') if body is not None else None
        req = urllib.request.Request(self.base + path, data=data, method=method,
                                     headers={'Content-Type': 'application/json; charset=utf-8'})
        if self.cookie:
            req.add_header('Cookie', self.cookie)
        try:
            resp = urllib.request.urlopen(req)
            # cookie ของ backend มี Secure — ส่งเองเพราะ urllib ไม่ส่ง Secure cookie ผ่าน http
            set_cookie = resp.headers.get('Set-Cookie')
            if set_cookie and path.endswith('/login'):
                self.cookie = set_cookie.split(';')[0]
            raw = resp.read().decode('utf-8')
            return resp.status, (json.loads(raw) if raw else None)
        except urllib.error.HTTPError as err:
            raw = err.read().decode('utf-8', errors='replace')
            try:
                return err.code, json.loads(raw)
            except ValueError:
                return err.code, raw


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument('--base-url', default='http://127.0.0.1:3000/api')
    parser.add_argument('--env', default=str(ROOT / '.env'))
    parser.add_argument('--data', default=str(Path(__file__).with_name('nationalities.json')))
    args = parser.parse_args()

    env = read_env(Path(args.env))
    rows = json.loads(Path(args.data).read_text(encoding='utf-8'))

    api = Api(args.base_url)
    status, _ = api.call('POST', '/auth/login', {
        'username': env['INITIAL_ADMIN_USERNAME'],
        'password': env['INITIAL_ADMIN_PASSWORD'],
    })
    if status != 200:
        print(f'login failed (HTTP {status}) — ตรวจ INITIAL_ADMIN_* ใน .env และ --base-url', file=sys.stderr)
        return 1

    status, existing = api.call('GET', '/nationalities')
    if status != 200:
        print(f'cannot list nationalities (HTTP {status})', file=sys.stderr)
        return 1
    have = {r['nationalName'].strip().lower() for r in existing}

    added = skipped = failed = 0
    for row in rows:
        if row['nationalName'].strip().lower() in have:
            skipped += 1
            continue
        status, body = api.call('POST', '/nationalities', {
            'nationalName': row['nationalName'],
            'nation': row['nation'],
            'sortOrder': row['sortOrder'],
            'isActive': True,
        })
        if status == 201:
            added += 1
        else:
            failed += 1
            print(f"FAILED {row['en']}: HTTP {status} {body}", file=sys.stderr)

    api.call('POST', '/auth/logout')
    print(f'เพิ่ม {added} รายการ, ข้าม (มีอยู่แล้ว) {skipped}, ล้มเหลว {failed}, รวมในไฟล์ {len(rows)}')
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
