import { useState } from 'react'

import { AddButton, DeleteButton, EditButton } from '@/components/layout/ActionButtons'
import { ListToolbar } from '@/components/layout/ListToolbar'
import { PageHeader } from '@/components/layout/PageHeader'
import { SearchForm } from '@/components/layout/SearchForm'
import { SortableTableHead } from '@/components/layout/SortableTableHead'
import { TablePagination } from '@/components/layout/TablePagination'
import { usePagination } from '@/components/layout/usePagination'
import { useSort } from '@/components/layout/useSort'
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
import { useAuth } from '@/features/auth/AuthContext'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'
import { resolveIcon } from '@/lib/icons'
import { ApiError } from '@/lib/api'
import { notify } from '@/lib/notify'
import { hasCriteria, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'

import type { PageFormValues } from './PageFormDialog'
import { PageFormDialog } from './PageFormDialog'
import type { Page } from './pagesApi'
import { pagesApi } from './pagesApi'
import { useAdminList } from './useAdminList'

// ข้อความ error ที่รู้จักจาก backend (backend/src/users/service.rs) แปลเป็นไทย
function pageErrorMessage(err: unknown): string {
  if (err instanceof ApiError && err.message === 'route already taken') {
    return 'route นี้มีเพจอื่นใช้อยู่แล้ว'
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}

type FormState = { mode: 'create' } | { mode: 'edit'; page: Page } | null

// ต่อ API จริงเต็ม CRUD (backend/src/users/{handlers,service}.rs) — create/update คืนแค่ status
// ไม่มี body เลย refetch() ทั้ง list ของหน้านี้เองและ catalog ของ RoleSwitcher หลังทุก mutation
export function PagesSettingsPage() {
  const { items: pages, status, refetch } = useAdminList(pagesApi.list)
  const { refetchCatalog } = useRolePreview()
  // หลังแก้ role/เพจ ต้องโหลดสิทธิ์ของผู้ใช้เองและเมนูซ้ายใหม่ด้วย (ไม่ใช่แค่ตารางในหน้านี้)
  const { refresh: refreshAuth } = useAuth()

  const [formState, setFormState] = useState<FormState>(null)
  const [pendingDelete, setPendingDelete] = useState<Page | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  // เงื่อนไขค้นหาที่ใช้กรองจริง (เปลี่ยนเมื่อกดปุ่ม "ค้นหา") — ช่องที่กรอกต้องตรงทุกช่อง
  const [criteria, setCriteria] = useState<SearchValues>({})
  const visiblePages = pages.filter(
    (page) =>
      textIncludes(page.label, criteria.label ?? '') &&
      textIncludes(page.groupLabel, criteria.groupLabel ?? '') &&
      textIncludes(page.route, criteria.route ?? ''),
  )

  const existingGroupLabels = [
    ...new Set(
      pages
        .map((page) => page.groupLabel)
        .filter((label): label is string => label !== null && label !== ''),
    ),
  ]

  // เรียงตามคอลัมน์ "ลำดับ" เมื่อกดหัวตาราง (หลังกรอง ก่อนแบ่งหน้า)
  const sort = useSort(visiblePages, (row) => row.sortOrder)
  const pagination = usePagination(sort.sorted)

  const handleFormSubmit = async (values: PageFormValues) => {
    try {
      if (formState?.mode === 'edit') {
        await pagesApi.update(formState.page.id, values)
      } else {
        await pagesApi.create(values)
      }
    } catch (err) {
      throw new Error(pageErrorMessage(err))
    }
    await refetch()
    refetchCatalog()
    await refreshAuth()
    notify.saved(formState?.mode === 'edit', 'เพิ่มเพจแล้ว', 'แก้ไขเพจแล้ว')
  }

  const handleDelete = async () => {
    if (!pendingDelete) return
    setIsDeleting(true)
    try {
      await pagesApi.remove(pendingDelete.id)
      await refetch()
      refetchCatalog()
      await refreshAuth()
      notify.deleted('ลบเพจแล้ว')
      setPendingDelete(null)
    } catch (err) {
      notify.error(pageErrorMessage(err))
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        title="การตั้งค่าเพจ"
        description="สร้าง แก้ไข ลบเพจ และกำหนดว่าใครเห็นเพจไหนในเมนู"
      />

      <SearchForm
        id="page-search"
        ariaLabel="ค้นหาเพจ"
        fields={[
          { name: 'label', label: 'ชื่อเพจ' },
          { name: 'groupLabel', label: 'กลุ่ม' },
          { name: 'route', label: 'route', placeholder: 'เช่น /settings' },
        ]}
        onSearch={(values) => {
          setCriteria(values)
          pagination.reset()
        }}
      />

      <ListToolbar
        ready={status === 'ready'}
        total={pages.length}
        shown={visiblePages.length}
        filtered={hasCriteria(criteria)}
      >
        <AddButton onClick={() => setFormState({ mode: 'create' })}>เพิ่มเพจ</AddButton>
      </ListToolbar>

      <Card>
        <CardContent className="px-0 sm:px-2">
          {status === 'loading' && (
            <p className="p-4 text-sm text-muted-foreground">กำลังโหลด...</p>
          )}
          {status === 'error' && (
            <p className="p-4 text-sm text-destructive">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</p>
          )}
          {status === 'ready' && pages.length > 0 && visiblePages.length === 0 && (
            <p className="p-4 text-sm text-muted-foreground">
              ไม่พบเพจที่ตรงกับเงื่อนไข — กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด
            </p>
          )}
          {status === 'ready' && visiblePages.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <SortableTableHead
                    direction={sort.direction}
                    onSort={() => {
                      sort.toggle()
                      pagination.reset()
                    }}
                  >
                    ลำดับ
                  </SortableTableHead>
                  <TableHead>เพจ</TableHead>
                  <TableHead>กลุ่ม</TableHead>
                  <TableHead>route</TableHead>
                  <TableHead>สิทธิ์ที่ต้องมี</TableHead>
                  <TableHead className="text-right">จัดการ</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((page) => {
                  const Icon = resolveIcon(page.icon)
                  return (
                    <TableRow key={page.id}>
                      <TableCell className="text-muted-foreground">{page.sortOrder}</TableCell>
                      <TableCell className="flex items-center gap-2 font-medium">
                        <span className="flex size-6 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
                          <Icon className="size-3.5" />
                        </span>
                        {page.label}
                      </TableCell>
                      <TableCell className="text-muted-foreground">
                        {page.groupLabel ?? '—'}
                      </TableCell>
                      <TableCell className="font-mono text-sm text-muted-foreground">
                        {page.route}
                      </TableCell>
                      <TableCell>
                        {page.permissions.length === 0 ? (
                          <span className="text-sm text-muted-foreground">สาธารณะ</span>
                        ) : (
                          <div className="flex flex-wrap gap-1">
                            {page.permissions.map((permission) => (
                              <Badge key={permission} variant="outline">
                                {permission}
                              </Badge>
                            ))}
                          </div>
                        )}
                      </TableCell>
                      <TableCell className="text-right">
                        <EditButton onClick={() => setFormState({ mode: 'edit', page })} />
                        <DeleteButton onClick={() => setPendingDelete(page)} />
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          )}
          {status === 'ready' && visiblePages.length > 0 && <TablePagination {...pagination.props} />}
        </CardContent>
      </Card>

      <PageFormDialog
        open={formState !== null}
        onOpenChange={(open) => {
          if (!open) setFormState(null)
        }}
        mode={formState?.mode ?? 'create'}
        initialValue={formState?.mode === 'edit' ? formState.page : undefined}
        onSubmit={handleFormSubmit}
        existingGroupLabels={existingGroupLabels}
      />

      <AlertDialog
        open={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>ยืนยันลบเพจ</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการลบเพจ &quot;{pendingDelete?.label}&quot; ใช่ไหม? การลบนี้ย้อนกลับไม่ได้
              ถ้าเป็นเพจที่อยู่ในเมนูอยู่ตอนนี้ จะหายจากเมนูของทุกคนทันที
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
