import { useState } from 'react'

import { AddButton, DeleteButton, EditButton } from '@/components/layout/ActionButtons'
import { ListToolbar } from '@/components/layout/ListToolbar'
import { PageHeader } from '@/components/layout/PageHeader'
import { SearchForm } from '@/components/layout/SearchForm'
import { TablePagination } from '@/components/layout/TablePagination'
import { usePagination } from '@/components/layout/usePagination'
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
import { Card, CardContent } from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { usePagePermissions } from '@/features/role-preview/usePagePermissions'
import { useAuth } from '@/features/auth/AuthContext'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'
import { ApiError } from '@/lib/api'
import { notify } from '@/lib/notify'
import { hasCriteria, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'

import type { RoleFormValues } from './RoleFormDialog'
import { RoleFormDialog } from './RoleFormDialog'
import type { Role } from './rolesApi'
import { rolesApi } from './rolesApi'
import type { PermissionGroup } from './permissions'
import { ACTION_LABELS, groupPermissionResources, permissionKey } from './permissions'
import { useAdminList } from './useAdminList'

// ข้อความ error ที่รู้จักจาก backend (backend/src/users/service/roles.rs) แปลเป็นไทย — อื่นนอกจากนี้
// (เช่น forbidden ซึ่งปุ่มถูกซ่อนตามสิทธิ์อยู่แล้ว) ใช้ข้อความกลาง ๆ
function roleErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    if (err.message === 'role name already taken') return 'ชื่อ role นี้มีอยู่แล้ว'
    if (err.message === 'cannot rename a system role') return 'เปลี่ยนชื่อ role ของระบบไม่ได้'
    if (err.message === 'cannot delete a system role') return 'ลบ role ของระบบไม่ได้'
    if (err.message === 'cannot remove your own role management permission') {
      return 'เอาสิทธิ์ "ดู" หรือ "แก้ไข" การตั้งค่าสิทธิ์ออกไม่ได้ เพราะบัญชีของคุณจะไม่เหลือสิทธิ์นี้จาก role ไหนเลย (กันล็อกตัวเองออกจากหน้าจัดการ)'
    }
    // ห้ามให้สิทธิ์เกินตัวเอง (docs/auth-design.md ข้อ 10)
    if (err.message === 'cannot grant permissions you do not have') {
      return 'ติ๊กสิทธิ์ที่บัญชีของคุณไม่มีไม่ได้ (ห้ามให้สิทธิ์เกินสิทธิ์ของตัวเอง)'
    }
    if (err.message === 'cannot manage a role with permissions you do not have') {
      return 'แก้หรือลบ role นี้ไม่ได้ เพราะ role นี้มีสิทธิ์ที่บัญชีของคุณไม่มี (เช่น admin)'
    }
    if (err.message === 'role is still assigned to at least one user') {
      return 'ลบไม่ได้ เพราะยังมีผู้ใช้ผูกกับ role นี้อยู่ — ย้ายผู้ใช้ออกได้ที่หน้า "ผู้ใช้งาน"'
    }
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}

// สรุปสิทธิ์ของ role เป็นรายเพจ เช่น "ผู้ป่วย: ดู เพิ่ม แก้ไข" (ครบทุก action = "ทั้งหมด") ชื่อเพจตามเมนู
// เรียงตามลำดับในฟอร์ม role — แทน badge ทีละคีย์ที่ยาวถึง 47 อัน
function summarizePermissions(permissions: string[], groups: PermissionGroup[]): string[] {
  return groups.flatMap((group) =>
    group.resources.flatMap(({ resource, label }) => {
      const granted = resource.actions.filter((action) =>
        permissions.includes(permissionKey(resource.key, action)),
      )
      if (granted.length === 0) return []
      const actions =
        granted.length === resource.actions.length
          ? 'ทั้งหมด'
          : granted.map((action) => ACTION_LABELS[action]).join(' ')
      return [`${label}: ${actions}`]
    }),
  )
}

type FormState = { mode: 'create' } | { mode: 'edit'; role: Role } | null

// ต่อ API จริงเต็ม CRUD (backend/src/users/{handlers,service}.rs) — create/update คืนแค่ status
// ไม่มี body เลย refetch() ทั้ง list ของหน้านี้เองและ catalog ของ RoleSwitcher หลังทุก mutation
// ให้สอดคล้องกันโดยไม่ต้อง refresh หน้า
export function RolesSettingsPage() {
  const { canCreate, canUpdate, canDelete } = usePagePermissions('role')
  const { items: roles, status, refetch } = useAdminList(rolesApi.list)
  const { refetchCatalog } = useRolePreview()
  // หลังแก้ role/เพจ ต้องโหลดสิทธิ์ของผู้ใช้เองและเมนูซ้ายใหม่ด้วย (ไม่ใช่แค่ตารางในหน้านี้)
  const { refresh: refreshAuth, menu } = useAuth()

  const [formState, setFormState] = useState<FormState>(null)
  const [pendingDelete, setPendingDelete] = useState<Role | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  // เงื่อนไขค้นหาที่ใช้กรองจริง (เปลี่ยนเมื่อกดปุ่ม "ค้นหา") — ช่องที่กรอกต้องตรงทุกช่อง
  const [criteria, setCriteria] = useState<SearchValues>({})
  const permissionGroups = groupPermissionResources(menu)
  const summaries = new Map(
    roles.map((role) => [role.id, summarizePermissions(role.permissions, permissionGroups)]),
  )
  // ช่อง "สิทธิ์" ค้นได้ทั้งคีย์ (patient.view) และข้อความสรุปภาษาไทย (ผู้ป่วย: ดู)
  const visibleRoles = roles.filter(
    (role) =>
      textIncludes(role.name, criteria.name ?? '') &&
      textIncludes(role.description, criteria.description ?? '') &&
      (role.permissions.length === 0
        ? !(criteria.permission ?? '').trim()
        : [...role.permissions, ...(summaries.get(role.id) ?? [])].some((text) =>
            textIncludes(text, criteria.permission ?? ''),
          )),
  )

  const pagination = usePagination(visibleRoles)

  const handleFormSubmit = async (values: RoleFormValues) => {
    try {
      if (formState?.mode === 'edit') {
        // ส่งชื่อเฉพาะตอนเปลี่ยนจริง — role ของระบบ (ช่องชื่อถูกล็อก) แก้แค่คำอธิบาย/สิทธิ์
        const { name, ...rest } = values
        await rolesApi.update(formState.role.id, name === formState.role.name ? rest : values)
      } else {
        await rolesApi.create(values)
      }
    } catch (err) {
      throw new Error(roleErrorMessage(err))
    }
    await refetch()
    refetchCatalog()
    await refreshAuth()
    notify.saved(formState?.mode === 'edit', 'เพิ่ม role แล้ว', 'แก้ไข role แล้ว')
  }

  const handleDelete = async () => {
    if (!pendingDelete) return
    setIsDeleting(true)
    try {
      await rolesApi.remove(pendingDelete.id)
      await refetch()
      refetchCatalog()
      await refreshAuth()
      notify.deleted('ลบ role แล้ว')
      setPendingDelete(null)
    } catch (err) {
      notify.error(roleErrorMessage(err))
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader fallbackTitle="การตั้งค่า role" description="สร้าง แก้ไข ลบ role และกำหนดสิทธิ์ในแต่ละ role" />

      <SearchForm
        id="role-search"
        ariaLabel="ค้นหา role"
        fields={[
          { name: 'name', label: 'ชื่อ role' },
          { name: 'description', label: 'คำอธิบาย' },
          { name: 'permission', label: 'สิทธิ์', placeholder: 'เช่น ผู้ป่วย หรือ patient.view' },
        ]}
        onSearch={(values) => {
          setCriteria(values)
          pagination.reset()
        }}
      />

      <ListToolbar
        ready={status === 'ready'}
        total={roles.length}
        shown={visibleRoles.length}
        filtered={hasCriteria(criteria)}
      >
        {canCreate && <AddButton onClick={() => setFormState({ mode: 'create' })}>เพิ่ม role</AddButton>}
      </ListToolbar>

      <Card>
        <CardContent className="px-0 sm:px-2">
          {status === 'loading' && (
            <p className="p-4 text-sm text-muted-foreground">กำลังโหลด...</p>
          )}
          {status === 'error' && (
            <p className="p-4 text-sm text-destructive">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</p>
          )}
          {status === 'ready' && roles.length > 0 && visibleRoles.length === 0 && (
            <p className="p-4 text-sm text-muted-foreground">
              ไม่พบ role ที่ตรงกับเงื่อนไข — กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด
            </p>
          )}
          {status === 'ready' && visibleRoles.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>ชื่อ role</TableHead>
                  <TableHead>คำอธิบาย</TableHead>
                  <TableHead>สิทธิ์</TableHead>
                  {(canUpdate || canDelete) && <TableHead className="text-right">จัดการ</TableHead>}
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((role) => (
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
                        {(summaries.get(role.id) ?? []).map((summary) => (
                          <Badge key={summary} variant="outline">
                            {summary}
                          </Badge>
                        ))}
                      </div>
                    </TableCell>
                    {(canUpdate || canDelete) && (
                      <TableCell className="text-right">
                        {canUpdate && <EditButton onClick={() => setFormState({ mode: 'edit', role })} />}
                        {canDelete && <DeleteButton disabled={role.isSystem} onClick={() => setPendingDelete(role)} />}
                      </TableCell>
                    )}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          {status === 'ready' && visibleRoles.length > 0 && <TablePagination {...pagination.props} />}
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
            <AlertDialogAction
              variant="danger"
              onClick={() => void handleDelete()}
              disabled={isDeleting}
            >
              {isDeleting ? 'กำลังลบ...' : 'ลบ'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
