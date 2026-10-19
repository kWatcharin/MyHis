import { CircleAlert, Inbox, SearchX } from 'lucide-react'
import type { ReactNode } from 'react'

import { Skeleton } from '@/components/ui/skeleton'
import { cn } from '@/lib/utils'

type ListStateKind = 'loading' | 'empty' | 'noMatch' | 'error'

interface ListStateProps {
  // loading = กำลังโหลด, empty = ยังไม่มีข้อมูลเลย, noMatch = มีข้อมูลแต่ไม่ตรงเงื่อนไขค้นหา, error = โหลดไม่สำเร็จ
  kind: ListStateKind
  // ข้อความ (loading ไม่ต้องส่ง ใช้ "กำลังโหลด..." เหมือนกันทุกหน้า)
  children?: ReactNode
}

const ICONS = { empty: Inbox, noMatch: SearchX, error: CircleAlert }

// สถานะของรายการในการ์ดตาราง ใช้ร่วมกันทุกหน้ารายการ — โหลด = แถว skeleton รูปทรงเดียวกับตาราง
// (ไม่ใช่ spinner กลางจอ) ข้อความยังมองเห็นได้, ว่าง/ไม่พบ/ผิดพลาด = ไอคอน + ข้อความของหน้านั้น ๆ
export function ListState({ kind, children }: ListStateProps) {
  if (kind === 'loading') {
    return (
      <div role="status" className="flex flex-col gap-2 px-4 py-3">
        <div aria-hidden className="flex flex-col gap-2">
          <Skeleton className="h-9 w-full" />
          <Skeleton className="h-9 w-full" />
          <Skeleton className="h-9 w-2/3" />
        </div>
        <p className="text-sm text-muted-foreground">{children ?? 'กำลังโหลด...'}</p>
      </div>
    )
  }

  const Icon = ICONS[kind]
  const isError = kind === 'error'
  return (
    <div
      role={isError ? 'alert' : undefined}
      className="flex flex-col items-center gap-3 px-4 py-10 text-center"
    >
      <span
        aria-hidden
        className={cn(
          'flex size-10 items-center justify-center rounded-full',
          isError ? 'bg-danger/15 text-danger-ink' : 'bg-muted text-muted-foreground',
        )}
      >
        <Icon className="size-5" />
      </span>
      <p className={cn('max-w-md text-sm', isError ? 'text-danger-ink' : 'text-muted-foreground')}>
        {children}
      </p>
    </div>
  )
}
