import { Badge } from '@/components/ui/badge'

import type { MasterColumn } from './types'

// คอลัมน์สถานะ "ใช้งานอยู่/ปิดใช้งาน" ใช้ร่วมกันทุก master ที่มี isActive
export function statusColumn<T extends { isActive: boolean }>(): MasterColumn<T> {
  return {
    header: 'สถานะ',
    cell: (item) => (
      <Badge variant={item.isActive ? 'default' : 'outline'}>
        {item.isActive ? 'ใช้งานอยู่' : 'ปิดใช้งาน'}
      </Badge>
    ),
  }
}

// คอลัมน์ข้อความธรรมดา ค่า null/ว่างแสดงเป็น "—"
export function textColumn<T>(
  header: string,
  pick: (item: T) => string | null,
  className?: string,
): MasterColumn<T> {
  return {
    header,
    className,
    cell: (item) => pick(item) || '—',
  }
}

export function sortOrderColumn<T extends { sortOrder: number }>(): MasterColumn<T> {
  return {
    header: 'ลำดับ',
    cell: (item) => item.sortOrder,
    className: 'text-muted-foreground',
    sortValue: (item) => item.sortOrder,
  }
}
