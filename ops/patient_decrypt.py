#!/usr/bin/env python3
"""ถอดรหัสคอลัมน์ที่เข้ารหัสของตาราง patient (hn, nap_no, first_name, last_name) นอก DB ด้วย key จาก .env
รูปแบบเดียวกับ backend/src/crypto.rs — SQL อย่างเดียวถอดไม่ได้ (DB ไม่มี key, pgcrypto ไม่มี AES-GCM)
ห้ามติดตั้ง pgcrypto หรือใส่ key ลงในคำสั่ง SQL แทน (ค้างใน pg_stat_activity/log/psql history)

ถอดรหัส: select uuid ตามด้วย encode(..., 'hex') ของ hn, nap_no, first_name, last_name ตามลำดับนี้
(คอลัมน์หลังจากนั้นใส่อะไรก็ได้ ส่งผ่านตามเดิม) ใส่ไฟล์ .sql นอก repo แล้วรันที่ root ของ repo:
    Get-Content q.sql -Raw | docker compose exec -T db sh -c 'psql -U $POSTGRES_USER -d $POSTGRES_DB -At -v ON_ERROR_STOP=1' | python -I ops/patient_decrypt.py decrypt

ค้นด้วย HN/เลขประจำตัวคนไข้: ถามค่าทางคีย์บอร์ด (ไม่รับทาง argument จะได้ไม่ค้างใน history) แล้วพิมพ์ blind index
เป็น hex ไปใช้ใน `where hn_idx = decode('<hex>', 'hex') and deleted_at is null` (nap_no ใช้ nap_no_idx)
    python -I ops/patient_decrypt.py index hn|nap_no

ระบุ .env อื่นได้เป็น argument ตัวสุดท้าย (ค่าเริ่มต้น .env ที่ root ของ repo) ต้องมี package `cryptography`
ไม่พิมพ์ key ออกมา ผลลัพธ์เป็นข้อมูลผู้ป่วย: รันใน terminal ปกติเท่านั้น (ไม่ใช่ผ่าน `!` ของ Claude Code) และอย่า
redirect ลงไฟล์ — อ่าน DB ตรงไม่ผ่านการตรวจสิทธิ์ patient.view ของหน้าเว็บ ใช้เฉพาะตอนจำเป็น"""
import hashlib
import hmac
import sys
from pathlib import Path

from cryptography.exceptions import InvalidTag
from cryptography.hazmat.primitives.ciphers.aead import AESGCM

ROOT = Path(__file__).resolve().parents[1]
ENCRYPTED = ('hn', 'nap_no', 'first_name', 'last_name')
BLOB_VERSION = 1
NONCE_LEN = 12


def read_key(env_path: Path, name: str) -> bytes:
    for line in env_path.read_text(encoding='utf-8').splitlines():
        k, sep, v = line.partition('=')
        if sep and k.strip() == name:
            try:
                key = bytes.fromhex(v.strip().strip('"\''))
            except ValueError:
                key = b''
            if len(key) != 32:
                sys.exit(f'{name} ใน {env_path} ต้องเป็น hex 64 ตัว')
            return key
    sys.exit(f'ไม่พบ {name} ใน {env_path}')


def decrypt(aead: AESGCM, column: str, hex_blob: str) -> str:
    # blob = version (1 byte) || nonce (12) || ciphertext+tag, aad = "patient.<คอลัมน์>"
    blob = bytes.fromhex(hex_blob)
    if len(blob) <= 1 + NONCE_LEN or blob[0] != BLOB_VERSION:
        raise ValueError
    nonce, ciphertext = blob[1:1 + NONCE_LEN], blob[1 + NONCE_LEN:]
    return aead.decrypt(nonce, ciphertext, f'patient.{column}'.encode()).decode('utf-8')


def run_decrypt(env_path: Path):
    aead = AESGCM(read_key(env_path, 'DATA_ENCRYPTION_KEY'))
    for line in sys.stdin:
        fields = line.rstrip('\r\n').split('|')
        if fields == ['']:
            continue
        if len(fields) < 1 + len(ENCRYPTED):
            sys.exit('ต้อง select uuid ตามด้วย encode(...,\'hex\') ของ hn, nap_no, first_name, last_name ตามลำดับ')
        out = [fields[0]]
        for column, hex_blob in zip(ENCRYPTED, fields[1:]):
            try:
                out.append(decrypt(aead, column, hex_blob))
            except (ValueError, InvalidTag):
                # key ไม่ตรง, ลำดับคอลัมน์สลับ หรือไม่ได้ encode เป็น hex — ไม่มีทางได้ค่าผิดออกมาเงียบ ๆ
                out.append(f'<ถอด {column} ไม่ได้>')
        out.extend(fields[1 + len(ENCRYPTED):])
        print(' | '.join(out))


def run_index(column: str, env_path: Path):
    if column not in ('hn', 'nap_no'):
        sys.exit('index ใช้ได้กับ hn หรือ nap_no เท่านั้น')
    key = read_key(env_path, 'BLIND_INDEX_KEY')
    value = input(f'{column}: ').strip()
    msg = f'patient.{column}'.encode() + b'\x00' + value.encode()
    print(hmac.new(key, msg, hashlib.sha256).hexdigest())


def main():
    # psql ส่ง UTF-8 มา, Windows PowerShell 5.1 แปะ BOM หน้าบรรทัดแรก
    sys.stdin.reconfigure(encoding='utf-8-sig')
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')
    args = sys.argv[1:]
    if args[:1] == ['decrypt'] and len(args) <= 2:
        run_decrypt(Path(args[1]) if len(args) == 2 else ROOT / '.env')
    elif args[:1] == ['index'] and 2 <= len(args) <= 3:
        run_index(args[1], Path(args[2]) if len(args) == 3 else ROOT / '.env')
    else:
        sys.exit(__doc__)


if __name__ == '__main__':
    main()
