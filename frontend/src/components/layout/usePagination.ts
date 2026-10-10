import { useState } from 'react'

import { DEFAULT_PAGE_SIZE } from './TablePagination'

// แบ่งหน้าฝั่ง client สำหรับรายการที่โหลดมาทั้งชุดแล้ว (หน้าตั้งค่า/ลงทะเบียนผู้ป่วย) — เริ่มที่ 10 แถวต่อหน้า
// ใช้คู่กับ <TablePagination {...pagination.props} /> และ map ตารางจาก pagination.pagedItems
// เรียก pagination.reset() เมื่อเงื่อนไขค้นหาเปลี่ยน (กลับไปหน้า 1)
export function usePagination<T>(items: T[]) {
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(DEFAULT_PAGE_SIZE)

  // clamp หน้าปัจจุบันตอน render (ไม่ใช้ effect) — เช่น ลบรายการสุดท้ายของหน้าสุดท้ายแล้วหน้านั้นหายไป
  const totalPages = Math.max(1, Math.ceil(items.length / pageSize))
  const currentPage = Math.min(page, totalPages)
  const pagedItems = items.slice((currentPage - 1) * pageSize, currentPage * pageSize)

  return {
    pagedItems,
    reset: () => setPage(1),
    props: {
      total: items.length,
      page: currentPage,
      pageSize,
      onPageChange: setPage,
      onPageSizeChange: (size: number) => {
        setPageSize(size)
        setPage(1)
      },
    },
  }
}
