import { Pencil, Plus, Trash2 } from 'lucide-react'
import type { ComponentProps } from 'react'

import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

type ButtonProps = Omit<ComponentProps<typeof Button>, 'variant'>

// ปุ่มการกระทำมาตรฐานของหน้ารายการทุกหน้า — ไอคอนตายตัวให้เหมือนกันทั้งระบบ:
//   เพิ่ม = ปุ่มหลักของหน้า สีเขียวเต็ม (success) + ไอคอนบวก — มีหน้าละปุ่มเดียวที่แถบเหนือตาราง
//   แก้ไข = ปุ่ม outline สีกลาง + ไอคอนดินสอ, ลบ = ปุ่มโปร่ง ตัวอักษรสีแดง (danger-ink) + ไอคอนถังขยะ
// ปุ่มในแถวตารางสีเรียบโดยตั้งใจ (2026-11-24): ตารางยาว ๆ ที่ทุกแถวมีปุ่มสีทึบสองปุ่มลายตาและแย่งความสนใจจากข้อมูล
// สีแดงทึบเก็บไว้ให้ปุ่มยืนยันลบใน dialog — การกระทำที่ย้อนไม่ได้ต้องเด่นตอนยืนยัน ไม่ใช่ตอนไล่ดูรายการ
// สีมาจาก token ของธีม (index.css) จึงปรับตามธีมที่เลือกเอง

export function AddButton({ children, ...props }: ButtonProps) {
  return (
    <Button variant="success" {...props}>
      <Plus />
      {children}
    </Button>
  )
}

export function EditButton({ children = 'แก้ไข', ...props }: ButtonProps) {
  return (
    <Button variant="outline" size="sm" {...props}>
      <Pencil />
      {children}
    </Button>
  )
}

export function DeleteButton({ children = 'ลบ', className, ...props }: ButtonProps) {
  return (
    <Button
      variant="ghost"
      size="sm"
      className={cn(
        'text-danger-ink hover:bg-danger/10 hover:text-danger-ink dark:hover:bg-danger/20',
        className,
      )}
      {...props}
    >
      <Trash2 />
      {children}
    </Button>
  )
}
