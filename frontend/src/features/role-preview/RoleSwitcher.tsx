import { useState } from 'react'
import { toast } from 'sonner'

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

import { useRolePreview } from './RolePreviewContext'

// label ภาษาไทยของแต่ละ role — เก็บแยกจาก mocks/fixtures.ts ตั้งใจ (ไฟล์นั้นหน้าตาต้องตรงกับ
// RoleResponse ของ backend เป๊ะ ๆ ไม่อยากปนข้อมูล UI-only เข้าไป)
const ROLE_LABELS: Record<string, string> = {
  admin: 'ผู้ดูแลระบบ (admin)',
  nurse: 'พยาบาล (nurse)',
}

const REAL_LABEL = 'สิทธิ์จริงของฉัน'

function labelFor(roleName: string | null): string {
  if (!roleName) return REAL_LABEL
  return ROLE_LABELS[roleName] ?? roleName
}

// ปุ่มมุมขวาบนของ header ใน AppShell — preview ว่า role อื่นที่ตัวเองมีสิทธิ์พอจะเห็นเมนู/เข้า
// หน้าอะไรได้บ้าง ไม่ใช่ login เป็นคนอื่นจริง เป็น client-side mock ล้วน ๆ (ดู
// RolePreviewContext.tsx ว่าทำไมถึงทำแบบนี้ไม่ใช่ของจริง)
export function RoleSwitcher() {
  const { activeRoleName, availableRoles, requestSwitch } = useRolePreview()
  // undefined = ปิด dialog, null = กำลังยืนยัน "กลับไปสิทธิ์จริง", string = กำลังยืนยัน role นั้น
  const [pendingRole, setPendingRole] = useState<string | null | undefined>(undefined)

  const handleSelect = (value: string) => {
    setPendingRole(value === '__real__' ? null : value)
  }

  const handleConfirm = () => {
    const target = pendingRole ?? null
    requestSwitch(target)
    toast(`กำลังดูในฐานะ: ${labelFor(target)}`)
    setPendingRole(undefined)
  }

  return (
    <>
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button variant={activeRoleName ? 'default' : 'outline'} size="sm">
              {activeRoleName ? `กำลังดูในฐานะ: ${labelFor(activeRoleName)}` : labelFor(null)}
            </Button>
          }
        />
        <DropdownMenuContent align="end">
          <DropdownMenuLabel>สลับมุมมองสิทธิ์</DropdownMenuLabel>
          <DropdownMenuSeparator />
          <DropdownMenuRadioGroup value={activeRoleName ?? '__real__'} onValueChange={handleSelect}>
            <DropdownMenuRadioItem value="__real__">{REAL_LABEL}</DropdownMenuRadioItem>
            {availableRoles.map((role) => (
              <DropdownMenuRadioItem key={role.id} value={role.name}>
                {labelFor(role.name)}
              </DropdownMenuRadioItem>
            ))}
          </DropdownMenuRadioGroup>
        </DropdownMenuContent>
      </DropdownMenu>

      <AlertDialog
        open={pendingRole !== undefined}
        onOpenChange={(open) => {
          if (!open) setPendingRole(undefined)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>ยืนยันสลับมุมมองสิทธิ์</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการสลับไปดูในฐานะ &quot;{labelFor(pendingRole ?? null)}&quot; ใช่ไหม? เมนูและ
              การเข้าถึงหน้าต่าง ๆ จะเปลี่ยนตามสิทธิ์นี้ทันที
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>ยกเลิก</AlertDialogCancel>
            <AlertDialogAction onClick={handleConfirm}>ยืนยัน</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </>
  )
}
