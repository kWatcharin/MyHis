import { ChevronLeft, ChevronRight, ChevronsLeft, ChevronsRight } from 'lucide-react'

import { Button } from '@/components/ui/button'
import { NativeSelect } from '@/components/ui/native-select'

export const DEFAULT_PAGE_SIZE = 10
const PAGE_SIZE_OPTIONS = [10, 20, 50]

interface TablePaginationProps {
  // จำนวนรายการทั้งหมด (หลังกรอง) — ตารางแบ่งหน้าฝั่ง client เพราะข้อมูลถูกโหลดมาทั้งชุดอยู่แล้ว
  total: number
  // หน้าปัจจุบัน เริ่มที่ 1 (ผู้เรียกควร clamp ให้อยู่ในช่วง 1..จำนวนหน้า)
  page: number
  pageSize: number
  onPageChange: (page: number) => void
  onPageSizeChange: (pageSize: number) => void
}

// แถบแบ่งหน้าใต้ตาราง: "แสดง a–b จาก n" + จำนวนแถวต่อหน้า + ปุ่มหน้าแรก/ก่อนหน้า/ถัดไป/หน้าสุดท้าย
// ใช้ซ้ำได้กับตารางอื่นที่ข้อมูลอยู่ฝั่ง client
export function TablePagination({
  total,
  page,
  pageSize,
  onPageChange,
  onPageSizeChange,
}: TablePaginationProps) {
  const totalPages = Math.max(1, Math.ceil(total / pageSize))
  const from = total === 0 ? 0 : (page - 1) * pageSize + 1
  const to = Math.min(page * pageSize, total)

  return (
    <div className="flex flex-col items-center gap-3 px-4 pb-1 text-sm tabular-nums sm:flex-row sm:justify-between sm:px-2">
      <p className="text-muted-foreground">
        แสดง {from}–{to} จาก {total} รายการ
      </p>
      <div className="flex flex-wrap items-center justify-center gap-3">
        <label className="flex items-center gap-2 text-muted-foreground">
          แถวต่อหน้า
          <NativeSelect
            value={pageSize}
            onChange={(e) => onPageSizeChange(Number(e.target.value))}
            className="h-8 w-auto"
            aria-label="จำนวนแถวต่อหน้า"
          >
            {PAGE_SIZE_OPTIONS.map((size) => (
              <option key={size} value={size}>
                {size}
              </option>
            ))}
          </NativeSelect>
        </label>
        <span className="text-muted-foreground">
          หน้า {page} / {totalPages}
        </span>
        <div className="flex items-center gap-1">
          <Button
            variant="outline"
            size="icon-sm"
            onClick={() => onPageChange(1)}
            disabled={page <= 1}
            aria-label="หน้าแรก"
          >
            <ChevronsLeft />
          </Button>
          <Button
            variant="outline"
            size="icon-sm"
            onClick={() => onPageChange(page - 1)}
            disabled={page <= 1}
            aria-label="หน้าก่อนหน้า"
          >
            <ChevronLeft />
          </Button>
          <Button
            variant="outline"
            size="icon-sm"
            onClick={() => onPageChange(page + 1)}
            disabled={page >= totalPages}
            aria-label="หน้าถัดไป"
          >
            <ChevronRight />
          </Button>
          <Button
            variant="outline"
            size="icon-sm"
            onClick={() => onPageChange(totalPages)}
            disabled={page >= totalPages}
            aria-label="หน้าสุดท้าย"
          >
            <ChevronsRight />
          </Button>
        </div>
      </div>
    </div>
  )
}
