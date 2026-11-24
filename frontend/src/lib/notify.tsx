import { CircleAlert, Download, Eye, Pencil, Plus, Trash2 } from 'lucide-react'
import { toast } from 'sonner'

// ข้อความแจ้งผล (toast) มาตรฐานของระบบ — แต่ละการกระทำมีไอคอนของมัน (เพิ่ม = บวก, แก้ไข = ดินสอ, ลบ = ถังขยะ ฯลฯ)
// สีตัวไอคอนตามความหมายจาก token *-ink ใน index.css (อ่านได้บนพื้น toast ทุกธีม) ไม่มีวงกลมสีทึบรองพื้น
// ใช้แทนการเรียก toast() ตรง ๆ ในหน้าต่าง ๆ เพื่อให้ไอคอน/สีเหมือนกันทั้งระบบ

const iconClass = 'size-4'

export const notify = {
  created: (message: string) =>
    toast(message, { icon: <Plus className={`${iconClass} text-success-ink`} /> }),
  updated: (message: string) =>
    toast(message, { icon: <Pencil className={`${iconClass} text-warning-ink`} /> }),
  deleted: (message: string) =>
    toast(message, { icon: <Trash2 className={`${iconClass} text-danger-ink`} /> }),
  // บันทึกจากฟอร์มเพิ่ม/แก้ไขเดียวกัน — เลือกข้อความและไอคอนตามโหมด
  saved: (isEdit: boolean, createdMessage: string, updatedMessage: string) =>
    isEdit ? notify.updated(updatedMessage) : notify.created(createdMessage),
  // ทำรายการไม่สำเร็จ
  error: (message: string) =>
    toast(message, { icon: <CircleAlert className={`${iconClass} text-danger-ink`} /> }),
  // แจ้งสถานะทั่วไป เช่น กำลังดูในฐานะ role ที่สลับไป
  info: (message: string) =>
    toast(message, { icon: <Eye className={`${iconClass} text-info-ink`} /> }),
  // ส่งออกไฟล์/เอกสารแล้ว (Excel, หน้าพิมพ์ PDF)
  exported: (message: string) =>
    toast(message, { icon: <Download className={`${iconClass} text-info-ink`} /> }),
}
