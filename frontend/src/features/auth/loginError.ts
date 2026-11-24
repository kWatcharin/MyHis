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
    // Turnstile (ตรวจก่อนดูชื่อผู้ใช้ — backend ไม่นับเป็นครั้งที่ผิด จึงไม่แตะตัวนับ)
    if (err.message === 'captcha failed') {
      return new LoginError('ยืนยันว่าไม่ใช่บอทไม่สำเร็จ ลองใหม่อีกครั้ง', null, null)
    }
    if (err.message === 'captcha unavailable') {
      return new LoginError('ระบบตรวจสอบความปลอดภัยไม่ตอบ ลองใหม่อีกครั้ง', null, null)
    }
    // มี login ค้างคิวเต็ม (503) — ยังไม่ได้ลองรหัสใด ๆ ไม่นับเป็นครั้งที่ผิด
    if (err.message === 'server busy') {
      return new LoginError('ระบบกำลังยุ่ง ลองใหม่อีกครั้งในไม่กี่วินาที', null, null)
    }
    // เพดาน 10 นาทีของ backend มี 2 แบบที่ตอบข้อความเดียวกัน (docs/auth-design.md ข้อ 9.2, 9.3): IP นี้ลองไม่สำเร็จครบเพดาน
    // (นับตามเครื่อง ไม่ใช่ตามชื่อผู้ใช้) หรือบัญชีนี้เข้าสำเร็จครบเพดานของหน้าต่าง — ข้อความจึงเป็นกลาง
    // ไม่ตั้ง lockedForSeconds: ตัวนับถอยหลังของหน้า login ผูกกับล็อกชื่อ 60 วินาที ("ลองผิดครบ 5 ครั้ง") ใช้กับบล็อก 10 นาทีไม่ได้
    if (err.message === 'too many attempts') {
      const minutes = Math.max(1, Math.ceil((err.retryAfterSeconds ?? 600) / 60))
      return new LoginError(
        `ลองเข้าสู่ระบบบ่อยเกินไป รอประมาณ ${minutes} นาทีแล้วลองใหม่`,
        null,
        null,
      )
    }
    if (err.message === 'account temporarily locked') {
      return new LoginError(
        `ลองผิดครบ ${MAX_LOGIN_ATTEMPTS} ครั้ง ล็อกชั่วคราว`,
        0,
        err.retryAfterSeconds ?? DEFAULT_LOCK_SECONDS,
      )
    }
  }
  return new LoginError('เกิดข้อผิดพลาด ลองใหม่อีกครั้ง', null, null)
}
