import { useState } from 'react'

// null = ยังไม่ได้เลือกเรียง (ลำดับตามที่ backend ส่งมา = sort_order น้อย→มาก แล้วตามชื่อ)
export type SortDirection = 'asc' | 'desc' | null

// เรียงรายการฝั่ง client ตามค่าตัวเลขหนึ่งค่า (เช่น ลำดับการแสดงผล) ใช้คู่กับ <SortableTableHead> —
// กดครั้งแรกเรียงน้อย→มาก จากนั้นสลับ มาก→น้อย / น้อย→มาก ไปเรื่อย ๆ การเรียงของ Array.sort คงลำดับเดิม
// ของแถวที่ค่าเท่ากัน (stable) เรียกต่อจากการกรองและก่อนแบ่งหน้า (usePagination)
export function useSort<T>(items: T[], accessor: (item: T) => number) {
  const [direction, setDirection] = useState<SortDirection>(null)

  const sorted =
    direction === null
      ? items
      : [...items].sort((a, b) => (direction === 'asc' ? 1 : -1) * (accessor(a) - accessor(b)))

  return {
    sorted,
    direction,
    toggle: () => setDirection((current) => (current === 'asc' ? 'desc' : 'asc')),
  }
}
