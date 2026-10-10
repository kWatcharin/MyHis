import { ArrowLeft, FileQuestionMark } from 'lucide-react'
import { useLocation, useNavigate } from 'react-router'

import { Button } from '@/components/ui/button'

// หน้า fallback เมื่อเข้า path ที่ไม่มีอยู่จริง — อยู่ในโครงหน้าหลัก (sidebar + header) ของผู้ที่ login แล้ว
// ส่วนผู้ที่ยังไม่ login จะถูก RequireAuth พาไปหน้า login ก่อนตามปกติ ปุ่มด้านล่างพากลับ Dashboard
export function NotFoundPage() {
  const { pathname } = useLocation()
  const navigate = useNavigate()

  return (
    <div className="flex min-h-[60vh] flex-col items-center justify-center gap-4 text-center">
      <span className="flex size-20 items-center justify-center rounded-full bg-primary/10 text-primary">
        <FileQuestionMark className="size-10" />
      </span>
      <p className="text-5xl font-semibold tracking-tight text-muted-foreground">404</p>
      <div className="flex flex-col gap-1">
        <h1 className="text-xl font-semibold">ไม่พบหน้าที่คุณต้องการ</h1>
        <p className="max-w-md text-sm text-muted-foreground">
          หน้านี้อาจถูกย้ายหรือลบไปแล้ว หรือพิมพ์ที่อยู่ผิด
        </p>
        <p className="break-all font-mono text-xs text-muted-foreground">{pathname}</p>
      </div>
      <Button onClick={() => navigate('/dashboard', { replace: true })}>
        <ArrowLeft />
        กลับไปหน้า Dashboard
      </Button>
    </div>
  )
}
