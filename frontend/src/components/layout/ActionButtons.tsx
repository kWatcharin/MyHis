import { Pencil, Plus, Trash2 } from 'lucide-react'
import type { ComponentProps } from 'react'

import { Button } from '@/components/ui/button'

type ButtonProps = Omit<ComponentProps<typeof Button>, 'variant'>

// ปุ่มการกระทำมาตรฐานของหน้ารายการทุกหน้า — สีและไอคอนตายตัวให้เหมือนกันทั้งระบบ:
//   เพิ่ม = เขียว + ไอคอนบวก, แก้ไข = ส้ม + ไอคอนดินสอ, ลบ = แดง + ไอคอนถังขยะ
// สีมาจาก token ของธีม (success / warning / danger ใน index.css — โทนเดียวกัน ตัวอักษรขาว) จึงปรับตามธีมที่เลือกเอง

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
    <Button variant="warning" size="sm" {...props}>
      <Pencil />
      {children}
    </Button>
  )
}

export function DeleteButton({ children = 'ลบ', ...props }: ButtonProps) {
  return (
    <Button variant="danger" size="sm" {...props}>
      <Trash2 />
      {children}
    </Button>
  )
}
