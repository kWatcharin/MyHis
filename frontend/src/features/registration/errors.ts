import { ApiError } from '@/lib/api'

// ข้อความกลางของ backend เมื่อลิงก์ใช้ไม่ได้ (ไม่มี/หมดอายุ/ใช้แล้ว/ถูกแทน/บัญชีถูกปิด — ไม่บอกว่าเพราะอะไร)
export const INVALID_LINK = 'invalid or expired registration link'

export function isInvalidLink(err: unknown): boolean {
  return err instanceof ApiError && err.message === INVALID_LINK
}

// ข้อความ error ที่รู้จักจาก backend (backend/src/registration/service.rs) แปลเป็นไทย
const KNOWN_ERRORS: Record<string, string> = {
  'prename is required': 'เลือกคำนำหน้า',
  'unknown prename': 'ไม่พบคำนำหน้านี้ (อาจถูกลบไปแล้ว) ลองโหลดหน้าใหม่',
  'first name is required': 'กรอกชื่อ',
  'last name is required': 'กรอกนามสกุล',
  'first name must be at most 100 characters': 'ชื่อยาวเกิน 100 ตัวอักษร',
  'last name must be at most 100 characters': 'นามสกุลยาวเกิน 100 ตัวอักษร',
  'position must be at most 100 characters': 'ตำแหน่งยาวเกิน 100 ตัวอักษร',
}

export function registrationErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    if (KNOWN_ERRORS[err.message]) return KNOWN_ERRORS[err.message]
    // validator ของ DTO: "new_password: new password must be at least 12 characters"
    if (err.message.includes('at least 12 characters')) {
      return 'รหัสผ่านต้องยาวอย่างน้อย 12 ตัวอักษร'
    }
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}
