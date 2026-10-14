import { api } from '@/lib/api'

// ตรงกับ AuthEventsResponse ของ backend (backend/src/audit/dto.rs) — เวลาเป็น UTC แปลงเป็นเวลาไทยตอนแสดงผล
export type AuthEventKind = 'login_succeeded' | 'login_failed' | 'logout' | 'logout_all'

export interface AuthEvent {
  occurredAt: string
  event: AuthEventKind
  // null = ไม่พบชื่อผู้ใช้ (login ด้วยชื่อที่ไม่มีในระบบ — backend ไม่ส่งชื่อที่พิมพ์มา)
  user: { id: string; username: string; displayName: string } | null
  ip: string | null
  userAgent: string | null
}

export interface AuthEventsResult {
  events: AuthEvent[]
  // เกินจำนวนสูงสุดของ backend — ได้แค่ส่วนที่ใหม่สุด
  truncated: boolean
}

// ตรงกับ ActivitiesResponse ของ backend — ประวัติการใช้งาน (ทุก action ยกเว้นเข้า/ออก)
// targetLabel = ชื่อรายการที่ไม่ใช่ข้อมูลผู้ป่วย (แถวผู้ป่วย = null เสมอ) ไม่มี uuid ของรายการ
export interface Activity {
  occurredAt: string
  action: string
  user: { id: string; username: string; displayName: string } | null
  targetType: string | null
  targetLabel: string | null
  exportDocument: string | null
  exportFormat: string | null
}

export interface ActivitiesResult {
  events: Activity[]
  truncated: boolean
}

function rangeQuery(from: string, to: string): string {
  return `from=${encodeURIComponent(from)}&to=${encodeURIComponent(to)}`
}

export const auditApi = {
  // from/to = "YYYY-MM-DD" วันตามเวลาไทย รวมทั้งสองวัน (URL มีแค่วันที่ ไม่มีข้อมูลระบุตัว)
  authEvents: (from: string, to: string) =>
    api.get<AuthEventsResult>(`/audit/auth-events?${rangeQuery(from, to)}`),
  activities: (from: string, to: string) =>
    api.get<ActivitiesResult>(`/audit/activities?${rangeQuery(from, to)}`),
}
