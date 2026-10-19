# REA — กฎการใช้ในโปรเจกต์นี้

REA (`rea-agents`, https://github.com/morluto/rea) คือ MCP server + CLI สำหรับ reverse engineering ติดตั้งบนเครื่องนี้
**ระดับผู้ใช้** (MCP server `rea` ใน `~/.claude.json` + skill `reverse-engineer-anything` ใน `~/.claude/skills`) จึงโหลด
อยู่ในทุก session รวมถึงตอนทำงานกับ my-his

สองเรื่องที่ทำให้ต้องมีกฎ:

- ผลของ tool ทุกตัวเข้าบทสนทนา และถูกส่งไปผู้ให้บริการโมเดล — อะไรที่ REA อ่านได้ ถือว่าออกนอกเครื่องแล้ว
- tool บางตัว **รัน** เป้าหมาย (runtime capture, terminal, เบราว์เซอร์ผ่าน playwright) ด้วยสิทธิ์ของผู้ใช้

## เป้าหมายที่ใช้ได้

- ไฟล์ โปรแกรม หรือเว็บของบุคคลที่สาม ที่ผู้ใช้สั่งให้วิเคราะห์ชัดเจน วางไว้ **นอก repo** (เช่น โฟลเดอร์ scratchpad
  หรือโฟลเดอร์แยกของงานนั้น)
- build ของ my-his เอง (`frontend/dist`) เมื่อผู้ใช้สั่ง — เป็นโค้ดล้วน ไม่มีข้อมูลผู้ป่วย
- อ่าน source code ของ repo นี้ใช้ Read/Grep ตามปกติ ไม่ใช้ REA (skill ของ REA เองก็บอกให้ข้ามงาน source repository)

## ห้ามชี้ REA ไปที่

- `.env` หรือไฟล์ใดที่มี key/รหัสผ่าน (`DATA_ENCRYPTION_KEY`, `BLIND_INDEX_KEY`, `TURNSTILE_SECRET_KEY`,
  `POSTGRES_PASSWORD`, `INITIAL_ADMIN_PASSWORD`)
- ข้อมูลผู้ป่วยทุกรูปแบบ: dump/backup ของฐานข้อมูล, volume `pg_data`, `docs/lab-visit-example-*.jpg` (ข้อมูลคนไข้จริง),
  ไฟล์ที่ส่งออกจากระบบ (Excel/PDF นัดหมายแล็ป), ผลจาก `ops/patient_decrypt.py`
- ระบบที่รันอยู่จริง (`localhost:3000`, `127.0.0.1:8081`) แบบ runtime capture/เบราว์เซอร์ — จะเก็บ response ที่มีข้อมูลผู้ป่วย
  ถอดรหัสแล้วและ cookie session ถ้าต้องดูพฤติกรรมหน้าเว็บของตัวเอง ใช้ preview ที่ stub `/api` ทุกเส้นด้วยข้อมูลปลอม
  (แบบ browser check เดิม) แทน

## ก่อนเรียก tool ที่รันเป้าหมาย

ถามผู้ใช้ก่อนทุกครั้ง โดยบอกว่าจะรันอะไร และรันด้วยสิทธิ์ของใคร ไฟล์ที่ดาวน์โหลดมาจากแหล่งที่ไม่รู้จักใช้ได้แค่การตรวจแบบ
static (ไม่รัน)

## ติดตั้ง อัปเดต ถอนการติดตั้ง

- ต้องใช้ Node.js ≥ 22.19 (เครื่องนี้ 22.23.3) ดูสถานะทั้งหมดด้วย `rea doctor` (อ่านอย่างเดียว)
- `rea setup` แก้ config ของ Claude Code — ให้ผู้ใช้สั่งเอง (auto mode บล็อกเพราะเป็นการแก้ตัว Claude Code เอง)
  อัปเดต: `npx rea-agents@latest setup --client claude_code --yes` แล้ว `rea doctor`
- ติดตั้งระดับผู้ใช้เท่านั้น — `--scope project` จะเขียน `.mcp.json` ลง repo ซึ่งไม่ต้องการ
- ถอน: ลบ `mcpServers.rea` ออกจาก `~/.claude.json`, ลบ `~/.claude/skills/reverse-engineer-anything`,
  `npm uninstall -g rea-agents` — ไม่คืนไฟล์ `~/.claude.json.rea.backup` ทั้งไฟล์ (สำรองไว้ก่อนติดตั้ง คืนแล้วค่าอื่นของ
  Claude Code ที่เปลี่ยนหลังจากนั้นหายหมด)
