import type { ReactNode } from 'react'
import { useLocation } from 'react-router'

import { useAuth } from '@/features/auth/AuthContext'
import { findMenuPage } from '@/features/auth/menu'

interface PageHeaderProps {
  // ชื่อหน้าสำรอง ใช้เมื่อ route นี้ไม่อยู่ในการตั้งค่าเพจ (ปกติชื่อหน้ามาจาก label ของเพจใน GET /menu)
  fallbackTitle: string
  // ต่อท้ายชื่อหน้า เช่น ": กลุ่ม A" ในหน้ารายละเอียดนัดหมาย
  titleSuffix?: string
  description?: string
  action?: ReactNode
}

// หัวข้อหน้ามาตรฐาน ใช้ร่วมกันทุกหน้าที่ต้อง login (Dashboard, การตั้งค่า role/เพจ ฯลฯ) กัน
// แต่ละหน้าต้องมาคุมสไตล์ h1/description เอง — ย่อลงเป็นแนวตั้งบนจอแคบ ปุ่ม action (ถ้ามี) อยู่
// ขวาบนจอกว้าง
// ชื่อหน้า = ชื่อเพจจากการตั้งค่าเพจ (เมนูของผู้ใช้จาก GET /menu — โหลดก่อนแอป render แล้ว และโหลดใหม่หลังแก้เพจ)
// จับคู่ด้วย route ของหน้าปัจจุบัน แก้ชื่อเพจในหน้าการตั้งค่าเพจแล้วหัวข้อเปลี่ยนตาม ใช้เมนูจริง ไม่ใช่เมนูตอน
// preview role (ชุด preview เป็นสับเซตของเมนูจริงเสมอ หน้าที่เข้าได้จึงมีชื่ออยู่แล้ว)
export function PageHeader({ fallbackTitle, titleSuffix, description, action }: PageHeaderProps) {
  const { menu } = useAuth()
  const { pathname } = useLocation()
  const title = (findMenuPage(menu, pathname)?.label ?? fallbackTitle) + (titleSuffix ?? '')

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
