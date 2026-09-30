import { useState } from 'react'
import { toast } from 'sonner'

import { PageHeader } from '@/components/layout/PageHeader'
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
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'
import { ApiError } from '@/lib/api'

import type { RoleFormValues } from './RoleFormDialog'
import { RoleFormDialog } from './RoleFormDialog'
import type { Role } from './rolesApi'
import { rolesApi } from './rolesApi'
import { useAdminList } from './useAdminList'

// ข้อความ error ที่รู้จักจาก backend (backend/src/users/service.rs) แปลเป็นไทย — อื่นนอกจากนี้
// (เช่น forbidden ซึ่งไม่ควรเกิดเพราะหน้านี้ gate ด้วย user.manage อยู่แล้ว) ใช้ข้อความกลาง ๆ
function roleErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    if (err.message === 'role name already taken') return 'ชื่อ role นี้มีอยู่แล้ว'
    if (err.message === 'cannot rename a system role') return 'เปลี่ยนชื่อ role ของระบบไม่ได้'
    if (err.message === 'cannot delete a system role') return 'ลบ role ของระบบไม่ได้'
    if (err.message === 'role is still assigned to at least one user') {
      return 'ลบไม่ได้ เพราะยังมีผู้ใช้ผูกกับ role นี้อยู่'
    }
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}

type FormState = { mode: 'create' } | { mode: 'edit'; role: Role } | null

// ต่อ API จริงเต็ม CRUD (backend/src/users/{handlers,service}.rs) — create/update คืนแค่ status
// ไม่มี body เลย refetch() ทั้ง list ของหน้านี้เองและ catalog ของ RoleSwitcher หลังทุก mutation
// ให้สอดคล้องกันโดยไม่ต้อง refresh หน้า
export function RolesSettingsPage() {
  const { items: roles, status, refetch } = useAdminList(rolesApi.list)
  const { refetchCatalog } = useRolePreview()

  const [formState, setFormState] = useState<FormState>(null)
  const [pendingDelete, setPendingDelete] = useState<Role | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)

  const handleFormSubmit = async (values: RoleFormValues) => {
    try {
      if (formState?.mode === 'edit') {
        await rolesApi.update(formState.role.id, values)
      } else {
        await rolesApi.create(values)
      }
    } catch (err) {
      throw new Error(roleErrorMessage(err))
    }
    await refetch()
    refetchCatalog()
    toast(formState?.mode === 'edit' ? 'แก้ไข role แล้ว' : 'เพิ่ม role แล้ว')
  }

  const handleDelete = async () => {
    if (!pendingDelete) return
    setIsDeleting(true)
    try {
      await rolesApi.remove(pendingDelete.id)
      await refetch()
      refetchCatalog()
      toast('ลบ role แล้ว')
      setPendingDelete(null)
    } catch (err) {
      toast.error(roleErrorMessage(err))
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        title="การตั้งค่า role"
        description="สร้าง แก้ไข ลบ role และกำหนดสิทธิ์ในแต่ละ role"
        action={<Button onClick={() => setFormState({ mode: 'create' })}>+ เพิ่ม role</Button>}
      />

      <Card>
        <CardContent className="px-0 sm:px-2">
          {status === 'loading' && (
            <p className="p-4 text-sm text-muted-foreground">กำลังโหลด...</p>
          )}
          {status === 'error' && (
            <p className="p-4 text-sm text-destructive">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</p>
          )}
          {status === 'ready' && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>ชื่อ role</TableHead>
                  <TableHead>คำอธิบาย</TableHead>
                  <TableHead>สิทธิ์</TableHead>
                  <TableHead className="text-right">จัดการ</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {roles.map((role) => (
                  <TableRow key={role.id}>
                    <TableCell className="font-medium">
                      {role.name}
                      {role.isSystem && (
                        <Badge variant="secondary" className="ml-2">
                          system
                        </Badge>
                      )}
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {role.description ?? '—'}
                    </TableCell>
                    <TableCell>
                      <div className="flex flex-wrap gap-1">
                        {role.permissions.map((permission) => (
                          <Badge key={permission} variant="outline">
                            {permission}
                          </Badge>
                        ))}
                      </div>
                    </TableCell>
                    <TableCell className="text-right">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => setFormState({ mode: 'edit', role })}
                      >
                        แก้ไข
                      </Button>
                      <Button
                        variant="ghost"
                        size="sm"
                        disabled={role.isSystem}
                        onClick={() => setPendingDelete(role)}
                      >
                        ลบ
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <RoleFormDialog
        open={formState !== null}
        onOpenChange={(open) => {
          if (!open) setFormState(null)
        }}
        mode={formState?.mode ?? 'create'}
        initialValue={formState?.mode === 'edit' ? formState.role : undefined}
        onSubmit={handleFormSubmit}
      />

      <AlertDialog
        open={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>ยืนยันลบ role</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการลบ role &quot;{pendingDelete?.name}&quot; ใช่ไหม? การลบนี้ย้อนกลับไม่ได้
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>ยกเลิก</AlertDialogCancel>
            <AlertDialogAction onClick={() => void handleDelete()} disabled={isDeleting}>
              {isDeleting ? 'กำลังลบ...' : 'ลบ'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
