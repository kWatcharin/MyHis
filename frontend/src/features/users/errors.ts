import { ApiError } from '@/lib/api'

// ข้อความ error ที่รู้จักจาก backend (backend/src/users/service/users.rs) แปลเป็นไทย
const KNOWN_ERRORS: Record<string, string> = {
  'username already taken': 'ชื่อผู้ใช้นี้มีอยู่แล้ว',
  'invalid username': 'ชื่อผู้ใช้ต้องยาว 3–32 ตัว ใช้ได้เฉพาะ a-z, 0-9, จุด (.), ขีดล่าง (_) และขีด (-)',
  'display name is required': 'กรอกชื่อที่แสดง',
  'display name must be at most 100 characters': 'ชื่อที่แสดงยาวเกิน 100 ตัวอักษร',
  'first name must be at most 100 characters': 'ชื่อยาวเกิน 100 ตัวอักษร',
  'last name must be at most 100 characters': 'นามสกุลยาวเกิน 100 ตัวอักษร',
  'position must be at most 100 characters': 'ตำแหน่งยาวเกิน 100 ตัวอักษร',
  'unknown prename': 'ไม่พบคำนำหน้านี้ (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า',
  'a user must have at least one role': 'ต้องเลือกอย่างน้อย 1 role',
  'cannot deactivate your own account': 'ปิดใช้งานบัญชีของตัวเองไม่ได้',
  'cannot reset your own password': 'รีเซ็ตรหัสผ่านของตัวเองไม่ได้ — กดชื่อผู้ใช้ท้ายเมนูแล้วเลือก "เปลี่ยนรหัสผ่าน" แทน',
  'cannot remove your own role management permission':
    'บัญชีของตัวเองต้องเหลือสิทธิ์ "ดู" และ "แก้ไข" การตั้งค่าสิทธิ์ (กันล็อกตัวเองออกจากระบบจัดการ)',
  'not found': 'ไม่พบผู้ใช้หรือ role นี้ (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า',
  // ห้ามให้สิทธิ์เกินตัวเอง (docs/auth-design.md ข้อ 10)
  'cannot grant permissions you do not have':
    'ให้ role ที่มีสิทธิ์ซึ่งบัญชีของคุณไม่มีไม่ได้ (ห้ามให้สิทธิ์เกินสิทธิ์ของตัวเอง)',
  'cannot manage a user with permissions you do not have':
    'จัดการบัญชีนี้ไม่ได้ เพราะบัญชีนี้มีสิทธิ์ที่บัญชีของคุณไม่มี (เช่น admin)',
}

export function userErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    if (KNOWN_ERRORS[err.message]) return KNOWN_ERRORS[err.message]
    // ข้อความจาก validator ของ DTO เช่น "username: username is required"
    if (err.message.includes('is required') || err.message.includes('cannot be empty')) {
      return 'กรอกข้อมูลที่จำเป็นให้ครบ'
    }
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}
