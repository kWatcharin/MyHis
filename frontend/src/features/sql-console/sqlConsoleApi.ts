import { ApiError, api } from '@/lib/api'

// หน้า SQL (backend/src/sql_console/) — รัน SQL อ่านอย่างเดียวด้วย role ฐานข้อมูลแยก ผลลัพธ์อาจมีข้อมูลผู้ป่วยที่
// ถอดรหัสแล้ว: เก็บแค่ใน state ของหน้า ห้ามลง localStorage/URL/console

export interface SchemaColumn {
  name: string
  type: string
  // คอลัมน์ที่เข้ารหัส (patient.hn ฯลฯ) — แสดงไอคอนแม่กุญแจ
  encrypted: boolean
}

export interface SchemaTable {
  name: string
  columns: SchemaColumn[]
}

// ตาราง/คอลัมน์ที่ role ของหน้า SQL อ่านได้ (ไม่มีคอลัมน์ลับ เช่น password_hash) + ขีดจำกัดจาก backend
export interface SqlSchema {
  tables: SchemaTable[]
  maxRows: number
  statementTimeoutMs: number
}

export interface ResultColumn {
  name: string
  type: string
  // false = ชนิดที่ backend ยังแปลงไม่ได้ (ค่าเป็น null) — ให้ cast เป็น ::text
  supported: boolean
  encrypted: boolean
  decrypted: boolean
}

export type Cell = string | number | boolean | null | Record<string, unknown> | unknown[]

export interface QueryResult {
  columns: ResultColumn[]
  rows: Cell[][]
  rowCount: number
  truncated: boolean
  durationMs: number
  // ช่องที่ถอดรหัสไม่ได้ (แสดงเป็นค่าว่าง)
  decryptErrors: number
}

// Postgres ปฏิเสธคำสั่ง — position = ตัวอักษรที่เท่าไร (เริ่ม 1) ในข้อความที่ส่งไป
export interface QueryFailure {
  message: string
  code: string | null
  position: number | null
}

export const sqlConsoleApi = {
  schema: () => api.get<SqlSchema>('/sql/schema'),
  run: (sql: string, decrypt: boolean) => api.post<QueryResult>('/sql/query', { sql, decrypt }),
}

// error ของ POST /sql/query ที่ backend ส่งรายละเอียดของ Postgres มา (400 "query failed") — อื่น ๆ = null
export function queryFailure(err: unknown): QueryFailure | null {
  if (!(err instanceof ApiError) || err.message !== 'query failed') return null
  const body = (err.body ?? {}) as { message?: string; code?: string; position?: number }
  return {
    // ผลลัพธ์ใหญ่เกินเพดานของ backend (backend/src/sql_console/service.rs RESULT_TOO_LARGE) — ไม่ใช่ error ของ Postgres จึงแปลเป็นไทย
    message: body.message?.startsWith('result too large')
      ? 'ผลลัพธ์ใหญ่เกินกำหนด (16 MiB หรือเกิน 200 คอลัมน์) เลือกคอลัมน์/แถวให้น้อยลง หรือตัดค่ายาว ๆ เช่น left(col::text, 100)'
      : (body.message ?? 'query failed'),
    code: body.code ?? null,
    position: typeof body.position === 'number' ? body.position : null,
  }
}

// คำสั่งที่หน้า SQL ไม่รัน (400 "statement not allowed") — read_only = ไม่ใช่การอ่านข้อมูล (keyword = ชื่อคำสั่ง เช่น
// INSERT, null = รู้แค่ว่ามีส่วนที่แก้ข้อมูล เช่น DELETE ใน WITH), multiple_statements = มี ; คั่นหลายคำสั่ง
export interface BlockedStatement {
  reason: 'read_only' | 'multiple_statements'
  keyword: string | null
}

export function blockedStatement(err: unknown): BlockedStatement | null {
  if (!(err instanceof ApiError) || err.message !== 'statement not allowed') return null
  const body = (err.body ?? {}) as { reason?: string; keyword?: string }
  return {
    reason: body.reason === 'multiple_statements' ? 'multiple_statements' : 'read_only',
    keyword: body.keyword ?? null,
  }
}

const KNOWN_ERRORS: Record<string, string> = {
  'sql is required': 'พิมพ์คำสั่ง SQL ก่อน',
  'invalid :hn(\'...\') helper': 'รูปแบบ :hn(\'...\') ไม่ถูกต้อง ใส่ค่าในเครื่องหมาย \' \' เช่น :hn(\'66471\')',
  'invalid :nap_no(\'...\') helper':
    'รูปแบบ :nap_no(\'...\') ไม่ถูกต้อง ใส่ค่าในเครื่องหมาย \' \' เช่น :nap_no(\'A1\')',
  'helpers need patient.view': 'ใช้ :hn() / :nap_no() ต้องมีสิทธิ์ดูข้อมูลผู้ป่วย (patient.view) เพราะใช้ค้นเลขระบุตัวผู้ป่วย',
  'sql console not configured': 'ยังไม่ได้ตั้งค่าหน้า SQL (ผู้ดูแลระบบต้องตั้งรหัสของ role ฐานข้อมูลก่อน)',
  'could not connect to the database in time': 'เชื่อมต่อฐานข้อมูลไม่ทัน ลองใหม่อีกครั้ง',
}

// error อื่นที่ไม่ใช่ข้อความของ Postgres → ข้อความไทย
export function consoleErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    if (KNOWN_ERRORS[err.message]) return KNOWN_ERRORS[err.message]
    if (err.message.startsWith('sql must be at most')) return 'คำสั่งยาวเกินกำหนด (20,000 ตัวอักษร)'
    if (err.status === 403) return 'ไม่มีสิทธิ์ใช้หน้า SQL'
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}
