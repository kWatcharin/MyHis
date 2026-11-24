import { ArrowLeft } from 'lucide-react'
import { useLocation, useNavigate } from 'react-router'

import { PageHeader } from '@/components/layout/PageHeader'
import { Button } from '@/components/ui/button'

// หน้า fallback เมื่อเข้า path ที่ไม่มีอยู่จริง — อยู่ในโครงหน้าหลัก (sidebar + header) ของผู้ที่ login แล้ว
// ส่วนผู้ที่ยังไม่ login จะถูก RequireAuth พาไปหน้า login ก่อนตามปกติ
// หัวหน้าแบบเดียวกับหน้าอื่น (PageHeader ชิดซ้าย) ไม่มีเลข 404 ตัวใหญ่หรือไอคอนประดับ: บอกว่าเกิดอะไร + ทางกลับ
export function NotFoundPage() {
  const { pathname } = useLocation()
  const navigate = useNavigate()

  return (
    <div className="flex flex-col items-start gap-4">
      <PageHeader
        fallbackTitle="ไม่พบหน้าที่ต้องการ"
        description="หน้านี้อาจถูกย้ายหรือลบไปแล้ว หรือพิมพ์ที่อยู่ผิด"
      />
      <p className="text-xs break-all text-muted-foreground">
        ที่อยู่ที่เปิด <span className="font-mono">{pathname}</span>
      </p>
      <Button variant="outline" onClick={() => navigate('/dashboard', { replace: true })}>
        <ArrowLeft />
        กลับไปหน้า Dashboard
      </Button>
    </div>
  )
}
