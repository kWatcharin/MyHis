import { Download, Eye, Pencil, Plus, Trash2, X } from 'lucide-react'
import type { ReactNode } from 'react'
import { toast } from 'sonner'

// ข้อความแจ้งผล (toast) มาตรฐานของระบบ — แต่ละการกระทำมีไอคอนในวงกลมสีเดียวกับปุ่มของมัน
// (เพิ่ม = เขียว+บวก, แก้ไข = ส้ม+ดินสอ, ลบ = แดง+ถังขยะ — สีจาก token ใน index.css ปรับตามธีมเอง)
// ใช้แทนการเรียก toast() ตรง ๆ ในหน้าต่าง ๆ เพื่อให้ไอคอน/สีเหมือนกันทั้งระบบ

function badge(className: string, icon: ReactNode) {
  return (
    <span className={`flex size-6 items-center justify-center rounded-full ${className}`}>
      {icon}
    </span>
  )
}

const iconClass = 'size-3.5'

export const notify = {
  created: (message: string) =>
    toast(message, { icon: badge('bg-success text-success-foreground', <Plus className={iconClass} />) }),
  updated: (message: string) =>
    toast(message, { icon: badge('bg-warning text-warning-foreground', <Pencil className={iconClass} />) }),
  deleted: (message: string) =>
    toast(message, { icon: badge('bg-danger text-danger-foreground', <Trash2 className={iconClass} />) }),
  // บันทึกจากฟอร์มเพิ่ม/แก้ไขเดียวกัน — เลือกข้อความและไอคอนตามโหมด
  saved: (isEdit: boolean, createdMessage: string, updatedMessage: string) =>
    isEdit ? notify.updated(updatedMessage) : notify.created(createdMessage),
  // ทำรายการไม่สำเร็จ
  error: (message: string) =>
    toast(message, { icon: badge('bg-danger text-danger-foreground', <X className={iconClass} />) }),
  // แจ้งสถานะทั่วไป เช่น กำลังดูในฐานะ role ที่สลับไป
  info: (message: string) =>
    toast(message, { icon: badge('bg-primary text-primary-foreground', <Eye className={iconClass} />) }),
  // ส่งออกไฟล์/เอกสารแล้ว (Excel, หน้าพิมพ์ PDF)
  exported: (message: string) =>
    toast(message, { icon: badge('bg-primary text-primary-foreground', <Download className={iconClass} />) }),
}
