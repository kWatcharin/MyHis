import type { ReactNode } from 'react'

interface ListToolbarProps {
  // false ระหว่างโหลด/โหลดไม่สำเร็จ (ไม่แสดงจำนวน)
  ready: boolean
  total: number
  shown: number
  // มีเงื่อนไขค้นหาอยู่ไหม — มี: "พบ x จาก y รายการ", ไม่มี: "ทั้งหมด y รายการ"
  filtered: boolean
  // ปุ่มเพิ่มข้อมูล (ว่างได้ถ้าไม่มีสิทธิ์เพิ่ม)
  children?: ReactNode
}

// แถวใต้ฟอร์มค้นหา: จำนวนรายการ (ซ้าย) + ปุ่มเพิ่มข้อมูล (ขวา) — ฟอร์มค้นหาอยู่เหนือแถวนี้เสมอ
export function ListToolbar({ ready, total, shown, filtered, children }: ListToolbarProps) {
  return (
    <div className="flex items-center justify-between gap-2">
      <p className="text-sm text-muted-foreground">
        {ready ? (filtered ? `พบ ${shown} จาก ${total} รายการ` : `ทั้งหมด ${total} รายการ`) : ''}
      </p>
      {children}
    </div>
  )
}
