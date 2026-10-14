// ช่วงวันที่ของหน้าใต้ "ประวัติ" (การเข้าออก + การใช้งาน) — ค่าเริ่มต้น 30 วันล่าสุด
// backend รับช่วงได้ไม่เกิน 366 วัน (backend/src/audit/service.rs) วันที่เป็นวันตามเวลาไทย "YYYY-MM-DD"
import { todayIso } from '@/lib/date'

const DEFAULT_DAYS = 30
const MAX_DAYS = 366

// วันที่ "YYYY-MM-DD" บวก/ลบวัน (คิดแบบ UTC ล้วน ไม่ขึ้นกับ timezone เครื่อง)
export function addDays(isoDate: string, days: number): string {
  const [year, month, day] = isoDate.split('-').map(Number)
  return new Date(Date.UTC(year, month - 1, day + days)).toISOString().slice(0, 10)
}

export function defaultRange(): { from: string; to: string } {
  const to = todayIso()
  return { from: addDays(to, -(DEFAULT_DAYS - 1)), to }
}

// ข้อความผิดพลาดของช่วงวัน (null = ใช้ได้) — ตรวจก่อนยิง API
export function rangeError(from: string, to: string): string | null {
  if (!from || !to) return 'เลือกวันที่ให้ครบทั้งสองช่อง'
  if (from > to) return 'วันที่เริ่มต้องไม่หลังวันที่สิ้นสุด'
  if ((Date.parse(to) - Date.parse(from)) / 86_400_000 + 1 > MAX_DAYS) return 'เลือกช่วงได้ไม่เกิน 366 วัน'
  return null
}
