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
  'cannot reset your own password': 'รีเซ็ตรหัสผ่านของตัวเองไม่ได้ — ใช้ปุ่ม "เปลี่ยนรหัสผ่าน" ท้ายเมนูแทน',
  'cannot remove your own user management permission':
    'ต้องคง role ที่มีสิทธิ์จัดการผู้ใช้ไว้กับบัญชีของตัวเอง (กันล็อกตัวเองออกจากระบบจัดการ)',
  'not found': 'ไม่พบผู้ใช้หรือ role นี้ (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า',
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
