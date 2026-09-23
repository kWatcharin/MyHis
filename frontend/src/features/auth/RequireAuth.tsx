import type { ReactNode } from 'react'
import { Navigate, useLocation } from 'react-router'

import { useRolePreview } from '@/features/role-preview/RolePreviewContext'

import { useAuth } from './AuthContext'

interface RequireAuthProps {
  children: ReactNode
  // ใส่เมื่ออยากบังคับ permission เฉพาะด้วย (เช่น /settings/*) — ไม่ใส่ = แค่เช็คว่า login แล้ว
  permission?: string
}

// Guard หลัก: หน้า public (ทุกหน้ายกเว้น /login) ต้อง redirect ไปหน้า login ทุกครั้งถ้ายังไม่ login
// ระหว่างที่ยังไม่รู้ผล (สถานะ loading — กำลังเช็ค /auth/me ตอนเพิ่งโหลดแอป) ยังไม่ redirect ไปไหน
// ไม่งั้น refresh หน้าที่ login อยู่จริงจะกระพริบไป /login ก่อนเช็คเสร็จ
export function RequireAuth({ children, permission }: RequireAuthProps) {
  const { status } = useAuth()
  // เช็คด้วยสิทธิ์ "effective" (สิทธิ์จริง หรือสิทธิ์ที่กำลัง preview อยู่ผ่าน RoleSwitcher — ดู
  // RolePreviewContext.tsx) ไม่ใช่สิทธิ์จริงตรง ๆ เพื่อให้ preview mode เด้งหน้าเหมือนที่ role
  // นั้นจะโดนจริง ไม่ใช่แค่ซ่อนเมนูอย่างเดียว
  const { hasEffectivePermission } = useRolePreview()
  const location = useLocation()

  if (status === 'loading') {
    return null
  }

  if (status === 'unauthenticated') {
    return <Navigate to="/login" replace state={{ from: location.pathname }} />
  }

  // login แล้วแต่ไม่มีสิทธิ์นี้ — เด้งไป dashboard ไม่ใช่ login (ตรงกับ 403 ไม่ใช่ 401 ฝั่ง backend)
  if (permission && !hasEffectivePermission(permission)) {
    return <Navigate to="/dashboard" replace />
  }

  return <>{children}</>
}
