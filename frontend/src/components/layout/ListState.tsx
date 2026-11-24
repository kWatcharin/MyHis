import { CircleAlert } from 'lucide-react'
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

// สถานะของรายการในการ์ดตาราง ใช้ร่วมกันทุกหน้ารายการ — โหลด = แถว skeleton รูปทรงเดียวกับตาราง
// (ไม่ใช่ spinner กลางจอ) ข้อความยังมองเห็นได้, ว่าง/ไม่พบ = ข้อความของหน้านั้น ๆ อย่างเดียว (ข้อความบอกวิธีทำต่อเอง
// ไม่ต้องมีไอคอนในวงกลมประดับ), ผิดพลาด = ไอคอนเตือนเล็กหน้าข้อความ สี danger-ink
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

  const isError = kind === 'error'
  return (
    <div role={isError ? 'alert' : undefined} className="flex justify-center px-4 py-8 text-center">
      <p
        className={cn(
          'max-w-md text-sm text-pretty',
          isError ? 'text-danger-ink' : 'text-muted-foreground',
        )}
      >
        {isError && <CircleAlert aria-hidden className="mr-1.5 inline size-4 align-[-0.2em]" />}
        {children}
      </p>
    </div>
  )
}
