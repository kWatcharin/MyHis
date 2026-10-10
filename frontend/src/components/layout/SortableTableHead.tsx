import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-react'
import type { ReactNode } from 'react'

import { TableHead } from '@/components/ui/table'
import { cn } from '@/lib/utils'

import type { SortDirection } from './useSort'

interface SortableTableHeadProps {
  children: ReactNode
  // สถานะการเรียงปัจจุบันของคอลัมน์นี้ (null = ยังไม่ได้เรียง)
  direction: SortDirection
  onSort: () => void
  className?: string
}

// หัวคอลัมน์ที่กดเรียงได้: กดสลับ น้อย→มาก (asc) / มาก→น้อย (desc) ไอคอนลูกศรบอกสถานะ
// (ลูกศรขึ้น = asc, ลูกศรลง = desc, ลูกศรสองทาง = ยังไม่ได้เรียง) ใช้ใน <TableHeader> เหมือน <TableHead>
export function SortableTableHead({ children, direction, onSort, className }: SortableTableHeadProps) {
  const Icon = direction === 'asc' ? ArrowUp : direction === 'desc' ? ArrowDown : ArrowUpDown
  return (
    <TableHead
      className={className}
      aria-sort={direction === 'asc' ? 'ascending' : direction === 'desc' ? 'descending' : 'none'}
    >
      <button
        type="button"
        onClick={onSort}
        title={direction === 'asc' ? 'เรียงน้อย→มาก (กดเพื่อเรียงมาก→น้อย)' : 'กดเพื่อเรียงน้อย→มาก / มาก→น้อย'}
        className="-mx-1 inline-flex items-center gap-1 rounded px-1 py-0.5 hover:bg-muted hover:text-foreground focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
      >
        {children}
        <Icon className={cn('size-3.5', direction === null && 'text-muted-foreground/60')} />
      </button>
    </TableHead>
  )
}
