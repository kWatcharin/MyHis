// วันที่ล้วน (ไม่มีเวลา) ในรูป "YYYY-MM-DD" ตามที่ backend ส่งมา — แยกสตริงเองทุกฟังก์ชัน ไม่ผ่าน
// new Date("YYYY-MM-DD") ที่ตีความเป็น UTC แล้วทำให้วันเลื่อนตาม timezone ของเบราว์เซอร์ แสดงผลเป็นปี พ.ศ.

const THAI_MONTHS_SHORT = [
  'ม.ค.',
  'ก.พ.',
  'มี.ค.',
  'เม.ย.',
  'พ.ค.',
  'มิ.ย.',
  'ก.ค.',
  'ส.ค.',
  'ก.ย.',
  'ต.ค.',
  'พ.ย.',
  'ธ.ค.',
]

export const THAI_MONTHS_FULL = [
  'มกราคม',
  'กุมภาพันธ์',
  'มีนาคม',
  'เมษายน',
  'พฤษภาคม',
  'มิถุนายน',
  'กรกฎาคม',
  'สิงหาคม',
  'กันยายน',
  'ตุลาคม',
  'พฤศจิกายน',
  'ธันวาคม',
]

export const BUDDHIST_ERA_OFFSET = 543

export function parseIsoDate(isoDate: string): [number, number, number] | null {
  const [year, month, day] = isoDate.split('-').map(Number)
  if (!year || !month || !day || month > 12 || day > 31) return null
  return [year, month, day]
}

const pad = (n: number) => String(n).padStart(2, '0')

export function toIso(year: number, month: number, day: number): string {
  return `${year}-${pad(month)}-${pad(day)}`
}

// "2026-10-01" → "1 ต.ค. 2569"
export function formatThaiDate(isoDate: string): string {
  const parts = parseIsoDate(isoDate)
  if (!parts) return isoDate
  const [year, month, day] = parts
  return `${day} ${THAI_MONTHS_SHORT[month - 1]} ${year + BUDDHIST_ERA_OFFSET}`
}

// "2026-01-15" → "15/01/2569" (ค่าในช่องกรอกวันที่ — พิมพ์แก้ได้ตรง ๆ)
export function formatThaiNumericDate(isoDate: string): string {
  const parts = parseIsoDate(isoDate)
  if (!parts) return ''
  const [year, month, day] = parts
  return `${pad(day)}/${pad(month)}/${year + BUDDHIST_ERA_OFFSET}`
}

// ข้อความที่พิมพ์ในช่องวันที่ (ปี พ.ศ. 4 หลัก) → "YYYY-MM-DD" ค.ศ. หรือ null ถ้าไม่ใช่วันที่จริง
// รับ "15/1/2569", "15-01-2569", "15.01.2569", "15 01 2569" และตัวเลขติดกัน "15012569" — ปีต่ำกว่า 2400 ถือว่าไม่ใช่ พ.ศ.
export function parseThaiNumericDate(text: string): string | null {
  const trimmed = text.trim()
  const parts = /^\d{8}$/.test(trimmed)
    ? [trimmed.slice(0, 2), trimmed.slice(2, 4), trimmed.slice(4)]
    : trimmed.split(/[/.\-\s]+/)
  if (parts.length !== 3 || !parts.every((part) => /^\d+$/.test(part)) || parts[2].length !== 4) return null
  const [day, month, buddhistYear] = parts.map(Number)
  const year = buddhistYear - BUDDHIST_ERA_OFFSET
  if (buddhistYear < 2400 || month < 1 || month > 12 || day < 1 || day > daysInMonth(year, month)) return null
  return toIso(year, month, day)
}

// "2025-12-09" → "9 ธ.ค. 68" (ปี พ.ศ. 2 หลักแบบที่เขียนในเอกสารกระดาษ)
export function formatThaiShortDate(isoDate: string): string {
  const parts = parseIsoDate(isoDate)
  if (!parts) return isoDate
  const [year, month, day] = parts
  const shortYear = String((year + BUDDHIST_ERA_OFFSET) % 100).padStart(2, '0')
  return `${day} ${THAI_MONTHS_SHORT[month - 1]} ${shortYear}`
}

// "2026-01-15" → "มกราคม 2569"
export function formatThaiMonthYear(isoDate: string): string {
  const parts = parseIsoDate(isoDate)
  if (!parts) return isoDate
  const [year, month] = parts
  return `${THAI_MONTHS_FULL[month - 1]} ${year + BUDDHIST_ERA_OFFSET}`
}

export function daysInMonth(year: number, month: number): number {
  // วันที่ 0 ของเดือนถัดไป = วันสุดท้ายของเดือนนี้ (Date.UTC ไม่ขึ้นกับ timezone เครื่อง)
  return new Date(Date.UTC(year, month, 0)).getUTCDate()
}

// บวกเดือนแบบปฏิทิน: วันที่เดิม ถ้าเดือนปลายทางไม่มีวันนั้นใช้วันสุดท้ายของเดือน
// เช่น "2026-08-31" + 6 → "2027-02-28", "2027-08-31" + 6 → "2028-02-29" (ไม่ล้นไปเดือนถัดไปแบบ Date.setMonth)
// คืน null ถ้าวันที่ไม่ถูกต้อง
export function addMonthsClamped(isoDate: string, months: number): string | null {
  const parts = parseIsoDate(isoDate)
  if (!parts) return null
  const [year, month, day] = parts
  const monthIndex = year * 12 + (month - 1) + months
  const targetYear = Math.floor(monthIndex / 12)
  const targetMonth = (monthIndex % 12) + 1
  return toIso(targetYear, targetMonth, Math.min(day, daysInMonth(targetYear, targetMonth)))
}

// แยกส่วนวัน/เวลาตามเวลาไทยเสมอ (backend เก็บและส่งเวลาเป็น UTC — แปลงเป็นเวลาไทยตอนแสดงผลเท่านั้น)
const BANGKOK_PARTS = new Intl.DateTimeFormat('en-GB', {
  timeZone: 'Asia/Bangkok',
  year: 'numeric',
  month: 'numeric',
  day: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
  hourCycle: 'h23',
})

// timestamp จาก backend (ISO, UTC) → "10 ต.ค. 2569 14:35" ตามเวลาไทย หรือ "10 ต.ค. 2569" ถ้า time = false
export function formatThaiDateTime(isoTimestamp: string, { time = true }: { time?: boolean } = {}): string {
  const date = new Date(isoTimestamp)
  if (Number.isNaN(date.getTime())) return isoTimestamp
  const parts = Object.fromEntries(
    BANGKOK_PARTS.formatToParts(date).map((part) => [part.type, part.value]),
  )
  const datePart = `${Number(parts.day)} ${THAI_MONTHS_SHORT[Number(parts.month) - 1]} ${
    Number(parts.year) + BUDDHIST_ERA_OFFSET
  }`
  return time ? `${datePart} ${parts.hour}:${parts.minute}` : datePart
}

// วันนี้ตามเวลาเครื่องผู้ใช้ในรูป YYYY-MM-DD (ไม่ใช้ toISOString เพราะเป็น UTC ทำให้วันเลื่อนช่วงเช้าตรู่ไทย)
export function todayIso(): string {
  const now = new Date()
  return toIso(now.getFullYear(), now.getMonth() + 1, now.getDate())
}
