// error ของ login แปลเป็นภาษาไทย + ตัวนับจาก backend (docs/auth-design.md ข้อ 6, 9) — backend ตอบเหมือนกันทุกกรณี
// (ชื่อไม่มี/รหัสผิด/บัญชีปิด) จึงมีข้อความเดียว ไม่โชว์ string ดิบจาก backend ให้ผู้ใช้เห็น
import { ApiError } from '@/lib/api'

// ตรงกับ MAX_FAILED_ATTEMPTS และระยะล็อกฝั่ง backend (backend/src/auth/service.rs)
export const MAX_LOGIN_ATTEMPTS = 5
export const DEFAULT_LOCK_SECONDS = 60

export class LoginError extends Error {
  // 401: เหลือโอกาสอีกกี่ครั้งก่อนล็อก (null = ไม่รู้ เช่น error อื่น)
  remainingAttempts: number | null
  // 429: ล็อกอยู่อีกกี่วินาที (จาก Retry-After)
  lockedForSeconds: number | null

  constructor(message: string, remainingAttempts: number | null, lockedForSeconds: number | null) {
    super(message)
    this.name = 'LoginError'
    this.remainingAttempts = remainingAttempts
    this.lockedForSeconds = lockedForSeconds
  }
}

export function toLoginError(err: unknown): LoginError {
  if (err instanceof ApiError) {
    if (err.message === 'invalid credentials') {
      const remaining = (err.body as { remainingAttempts?: unknown } | null)?.remainingAttempts
      return new LoginError(
        'ชื่อผู้ใช้หรือรหัสผ่านไม่ถูกต้อง',
        typeof remaining === 'number' ? remaining : null,
        null,
      )
    }
    if (err.message === 'account temporarily locked') {
      return new LoginError(
        `ลองผิดครบ ${MAX_LOGIN_ATTEMPTS} ครั้ง — ล็อกชั่วคราว`,
        0,
        err.retryAfterSeconds ?? DEFAULT_LOCK_SECONDS,
      )
    }
  }
  return new LoginError('เกิดข้อผิดพลาด ลองใหม่อีกครั้ง', null, null)
}
