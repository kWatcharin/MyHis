import { ApiError } from '@/lib/api'

// ข้อความ error ที่รู้จักจาก backend (backend/src/lab_visit/service.rs) แปลงเป็นไทย
const KNOWN_ERRORS: Record<string, string> = {
  'unknown budget year': 'ไม่พบปีงบประมาณนี้ (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า',
  'unknown patient group': 'ไม่พบกลุ่มคนไข้นี้ (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า',
  'budget year is inactive': 'ปีงบประมาณนี้ปิดใช้งานแล้ว เลือกปีอื่น',
  'patient group is inactive': 'กลุ่มคนไข้นี้ปิดใช้งานแล้ว เลือกกลุ่มอื่น',
  'cd4 second date must be after first date': 'วันนัด CD4 ครั้งที่ 2 ต้องหลังครั้งที่ 1',
  'lab visit group already exists': 'มีนัดหมายของกลุ่มนี้ในปีงบประมาณนี้แล้ว',
  'patient ids are required': 'เลือกคนไข้อย่างน้อย 1 คน',
  'too many patients': 'เลือกคนไข้มากเกินไปในครั้งเดียว (ไม่เกิน 500 คน)',
  'unknown patient': 'มีคนไข้ที่ไม่พบในทะเบียน (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า',
  'patient is inactive': 'มีคนไข้ที่ปิดใช้งานอยู่ เพิ่มเข้านัดหมายไม่ได้',
  'patient already in group': 'มีคนไข้ที่อยู่ในนัดหมายนี้แล้ว ลองรีเฟรชหน้า',
  'take med years is out of range': 'ระยะเวลากินยา (ปี) ต้องอยู่ระหว่าง 0–100',
  'take med months is out of range': 'ระยะเวลากินยา (เดือน) ต้องอยู่ระหว่าง 0–11',
  'vl result must be at most 100 characters': 'ผล VL ยาวได้ไม่เกิน 100 ตัวอักษร',
  'drug recipe must be at most 200 characters': 'สูตรยายาวได้ไม่เกิน 200 ตัวอักษร',
}

export function labVisitErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    if (KNOWN_ERRORS[err.message]) return KNOWN_ERRORS[err.message]
    if (err.status === 404) return 'ไม่พบรายการนี้ (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า'
    if (err.status === 403) return 'ไม่มีสิทธิ์ทำรายการนี้'
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}
