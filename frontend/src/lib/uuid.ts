// id ของทุกอย่างที่ backend ส่งออกมาเป็น uuid (ไม่ส่ง id int ภายใน) — เพจที่รับ id จาก URL ใช้ตรวจก่อนยิง API
// ไม่ใช่ uuid = ลิงก์ผิด/ถูกแก้ ให้แสดงหน้าไม่พบแทนการยิงคำขอ
const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i

export function isUuid(value: string): boolean {
  return UUID_PATTERN.test(value)
}
