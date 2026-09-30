// ชุด permission ปิดตายฝั่ง backend (Permission enum, backend/src/domain/mod.rs) — ใช้ค่าพวกนี้
// เป็น checkbox ในฟอร์ม role/page แทนให้พิมพ์เองเพื่อกันส่งค่าที่ backend ไม่รู้จัก (400 unknown
// permission) รายชื่อนี้ต้อง sync มือกับ backend เอง เพราะ backend ไม่มี endpoint คืนรายชื่อนี้
export const ALL_PERMISSIONS = ['user.manage', 'shift.read', 'shift.write', 'audit.read'] as const

export const PERMISSION_LABELS: Record<string, string> = {
  'user.manage': 'จัดการผู้ใช้ / role / เพจ',
  'shift.read': 'ดูตารางเวร',
  'shift.write': 'แก้ไขตารางเวร',
  'audit.read': 'ดู audit log',
}
