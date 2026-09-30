import type { ReactNode } from 'react'

interface PageHeaderProps {
  title: string
  description?: string
  action?: ReactNode
}

// หัวข้อหน้ามาตรฐาน ใช้ร่วมกันทุกหน้าที่ต้อง login (Dashboard, การตั้งค่า role/เพจ ฯลฯ) กัน
// แต่ละหน้าต้องมาคุมสไตล์ h1/description เอง — ย่อลงเป็นแนวตั้งบนจอแคบ ปุ่ม action (ถ้ามี) อยู่
// ขวาบนจอกว้าง
export function PageHeader({ title, description, action }: PageHeaderProps) {
  return (
    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
      <div>
        <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
        {description && <p className="text-sm text-muted-foreground">{description}</p>}
      </div>
      {action}
    </div>
  )
}
