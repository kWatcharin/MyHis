import { ApiError } from '@/lib/api'

// ข้อความ error ที่รู้จักจาก backend (backend/src/stock/service/receipts.rs) แปลงเป็นไทย
const RECEIPT_ERRORS: Record<string, string> = {
  'at least one line is required': 'เพิ่มรายการอย่างน้อย 1 บรรทัด',
  'too many lines': 'ใบรับหนึ่งใบมีได้ไม่เกิน 100 บรรทัด',
  'unknown budget year': 'ไม่พบปีงบประมาณที่เลือก (อาจถูกลบไปแล้ว) ปิดแล้วเปิดฟอร์มใหม่',
  'budget year is inactive': 'ปีงบประมาณที่เลือกถูกปิดใช้งาน เลือกปีอื่น',
  'received date is outside the budget year': 'วันที่รับไม่อยู่ในช่วงของปีงบประมาณที่เลือก',
  'received date must not be in the future': 'วันที่รับต้องไม่เกินวันนี้',
  'unknown item': 'ไม่พบรายการที่เลือก (อาจถูกลบไปแล้ว) ปิดแล้วเปิดฟอร์มใหม่',
  'item is inactive': 'มีรายการที่ถูกปิดใช้งาน ปิดแล้วเปิดฟอร์มใหม่',
  'unit is not a unit of this item': 'หน่วยที่เลือกไม่ใช่หน่วยของรายการ (หน่วยแปลงอาจถูกแก้) ปิดแล้วเปิดฟอร์มใหม่',
  'quantity must be greater than 0': 'จำนวนต้องมากกว่า 0',
  'quantity is too large': 'จำนวนมากเกินไป (ไม่เกิน 10,000,000 หน่วยฐานต่อบรรทัด)',
  'item does not track lots': 'รายการนี้ไม่ติดตามล็อต — ไม่ต้องกรอกเลขล็อต',
  'item does not track expiry': 'รายการนี้ไม่ติดตามวันหมดอายุ — ไม่ต้องกรอกวันหมดอายุ',
  'lot number is required': 'กรอกเลขล็อตให้ครบทุกบรรทัดที่ติดตามล็อต',
  'expiry date is required': 'กรอกวันหมดอายุให้ครบทุกบรรทัดที่ติดตามวันหมดอายุ',
  'item is already expired': 'มีของที่หมดอายุแล้ว (วันหมดอายุก่อนวันที่รับ) รับเข้าไม่ได้',
  'duplicate lot in receipt': 'ล็อตเดียวกันซ้ำในใบ — รวมเป็นบรรทัดเดียว',
  'expiry date does not match the existing lot that still has stock':
    'วันหมดอายุไม่ตรงกับล็อตเดิมที่ยังมีของในคลัง — ตรวจเลขล็อต/วันหมดอายุอีกครั้ง',
  'item changed while saving, please try again': 'รายการถูกแก้ไขระหว่างบันทึก ปิดแล้วเปิดฟอร์มใหม่',
  'receipt is cancelled': 'ใบรับนี้ถูกยกเลิกแล้ว แก้ไขไม่ได้',
  'receipt is already cancelled': 'ใบรับนี้ถูกยกเลิกไปแล้ว',
  'stock from this receipt has already been issued':
    'ของจากใบนี้ถูกเบิกออกไปแล้วบางส่วน ยกเลิกทั้งใบไม่ได้',
  'cancel reason is required': 'กรอกเหตุผลที่ยกเลิก',
}

export function receiptErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    const known = RECEIPT_ERRORS[err.message]
    if (known) return known
    if (err.message.endsWith('characters')) return 'ข้อความยาวเกินกำหนด'
    if (err.status === 404) return 'ไม่พบใบรับนี้ (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า'
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}
