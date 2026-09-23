import { createContext, useCallback, useContext, useState } from 'react'
import type { ReactNode } from 'react'
import { useNavigate } from 'react-router'

import { useAuth } from '@/features/auth/AuthContext'
import type { MenuItem } from '@/features/auth/AuthContext'
import { mockPages, mockRoles } from '@/mocks/fixtures'
import type { MockRole } from '@/mocks/fixtures'

interface RolePreviewContextValue {
  // null = ใช้สิทธิ์จริงของตัวเอง (ค่าเริ่มต้นเสมอ ไม่ persist ข้าม refresh — ตั้งใจ เป็น
  // เครื่องมือ preview ชั่วคราว ไม่ใช่การตั้งค่าถาวร)
  activeRoleName: string | null
  effectivePermissions: string[]
  effectiveMenu: MenuItem[]
  // role ที่ permission ทั้งหมด ⊆ สิทธิ์จริง — คำนวณจากสิทธิ์จริงเสมอ ไม่ใช่ effective กัน
  // "สลับลงแล้วสลับกลับขึ้นไม่ได้"
  availableRoles: MockRole[]
  hasEffectivePermission: (permission: string) => boolean
  requestSwitch: (roleName: string | null) => void
}

const RolePreviewContext = createContext<RolePreviewContextValue | null>(null)

export function RolePreviewProvider({ children }: { children: ReactNode }) {
  const { user, menu } = useAuth()
  const [activeRoleName, setActiveRoleName] = useState<string | null>(null)
  const navigate = useNavigate()

  // เปลี่ยนตัว user จริง (login คนละคนในแท็บเดียวกัน) ต้องรีเซ็ต preview กลับเป็นสิทธิ์จริงเสมอ —
  // ปรับ state ระหว่าง render ตรง ๆ ตามแนวทางของ React แทนการใช้ effect (กัน render เกินความจำเป็น)
  const [lastUserId, setLastUserId] = useState(user?.id)
  if (user?.id !== lastUserId) {
    setLastUserId(user?.id)
    setActiveRoleName(null)
  }

  const realPermissions = user?.permissions ?? []

  const availableRoles = mockRoles.filter((role) =>
    role.permissions.every((permission) => realPermissions.includes(permission)),
  )

  const activeRole = activeRoleName
    ? (mockRoles.find((role) => role.name === activeRoleName) ?? null)
    : null

  const effectivePermissions = activeRole ? activeRole.permissions : realPermissions

  const effectiveMenu: MenuItem[] = activeRole
    ? mockPages
        .filter(
          (page) =>
            page.permissions.length === 0 ||
            page.permissions.some((permission) => effectivePermissions.includes(permission)),
        )
        .sort((a, b) => a.sortOrder - b.sortOrder)
        .map((page) => ({ id: page.id, label: page.label, icon: page.icon, route: page.route }))
    : menu

  const hasEffectivePermission = useCallback(
    (permission: string) => effectivePermissions.includes(permission),
    [effectivePermissions],
  )

  const requestSwitch = useCallback(
    (roleName: string | null) => {
      setActiveRoleName(roleName)
      navigate('/dashboard', { replace: true })
    },
    [navigate],
  )

  return (
    <RolePreviewContext.Provider
      value={{
        activeRoleName,
        effectivePermissions,
        effectiveMenu,
        availableRoles,
        hasEffectivePermission,
        requestSwitch,
      }}
    >
      {children}
    </RolePreviewContext.Provider>
  )
}

export function useRolePreview() {
  const ctx = useContext(RolePreviewContext)
  if (!ctx) {
    throw new Error('useRolePreview ต้องถูกเรียกภายใน <RolePreviewProvider>')
  }
  return ctx
}
