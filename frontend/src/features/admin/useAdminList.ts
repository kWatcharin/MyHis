import { useCallback, useEffect, useState } from 'react'

type Status = 'loading' | 'ready' | 'error'

// ดึง list ตอน mount + refetch ได้เอง — ใช้ร่วมกันทั้ง RolesSettingsPage/PagesSettingsPage เพราะ
// สอง endpoint นี้มี shape การโหลด/refetch เหมือนกันเป๊ะ ต่างแค่ฟังก์ชัน list() ที่ส่งเข้ามา
// silent = โหลดใหม่โดยไม่สลับเป็นสถานะ loading (list เดิมค้างอยู่บนจอ) — ใช้หลังลากจัดลำดับที่จอแสดงผลใหม่ไปแล้ว
export function useAdminList<T>(list: () => Promise<T[]>) {
  const [items, setItems] = useState<T[]>([])
  const [status, setStatus] = useState<Status>('loading')

  const refetch = useCallback(
    async (options?: { silent?: boolean }) => {
      if (!options?.silent) setStatus('loading')
      try {
        const data = await list()
        setItems(data)
        setStatus('ready')
      } catch {
        setStatus('error')
      }
    },
    [list],
  )

  useEffect(() => {
    void refetch()
  }, [refetch])

  return { items, status, refetch }
}
