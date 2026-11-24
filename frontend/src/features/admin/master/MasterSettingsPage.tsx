import { useState } from 'react'

import { AddButton, DeleteButton, EditButton } from '@/components/layout/ActionButtons'
import { ListState } from '@/components/layout/ListState'
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
import { ApiError } from '@/lib/api'
import { notify } from '@/lib/notify'
import { STATUS_SEARCH_FIELD, hasCriteria, statusMatches, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'

import { useAdminList } from '../useAdminList'
import type { MasterFormValues } from './MasterFormDialog'
import { MasterFormDialog } from './MasterFormDialog'
import type { MasterConfig } from './types'

// error ที่ backend ส่งรูปแบบเดียวกันทุก master (ข้อความ validation ของ service) แปลงแบบรวม ส่วน
// "<entity> name already taken" / "<entity> is still used" เฉพาะแต่ละ master อยู่ใน config.errorMessages
function errorMessage(err: unknown, known: Record<string, string>): string {
  if (err instanceof ApiError) {
    if (known[err.message]) return known[err.message]
    if (err.message.endsWith('must be at most 100 characters')) {
      return 'ข้อความยาวเกิน 100 ตัวอักษร'
    }
    if (err.message.endsWith(' is required')) return 'กรอกข้อมูลที่จำเป็นให้ครบ'
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}

type FormState<T> = { mode: 'create' } | { mode: 'edit'; item: T } | null

// หน้าตั้งค่า master แบบ CRUD เต็ม (โครงเดียวกับหน้าตั้งค่าโรค) — config กำหนดคอลัมน์/ฟิลด์/ข้อความ
// ต่อ API จริง: create/update คืนแค่ status ไม่มี body จึง refetch() ทั้ง list หลังทุก mutation
export function MasterSettingsPage<T extends { id: string }>({ config }: { config: MasterConfig<T> }) {
  const { canCreate, canUpdate, canDelete } = usePagePermissions(config.resource)
  // config.api.list ต้องเป็นฟังก์ชันตัวเดิมทุก render (config เป็นค่าคงที่ระดับ module และ api เป็น object
  // ของ arrow function) — useAdminList ใช้เป็น dependency ของ effect ถ้าสร้างใหม่ทุกครั้งจะโหลดวนไม่จบ
  const { items, status, refetch } = useAdminList(config.api.list)

  const [formState, setFormState] = useState<FormState<T>>(null)
  const [pendingDelete, setPendingDelete] = useState<T | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  // เงื่อนไขค้นหาที่ใช้กรองจริง (เปลี่ยนเมื่อกดปุ่ม "ค้นหา" ในฟอร์ม) — ช่องที่กรอกต้องตรงทุกช่อง
  const [criteria, setCriteria] = useState<SearchValues>({})

  const visibleItems = items.filter(
    (item) =>
      config.searchFields.every((field) => textIncludes(field.value(item), criteria[field.name] ?? '')) &&
      (!config.statusOf || statusMatches(config.statusOf(item), criteria.status)),
  )
  const searchFields = config.statusOf ? [...config.searchFields, STATUS_SEARCH_FIELD] : config.searchFields

  // คอลัมน์ที่กดเรียงได้ (ถ้า config มี sortValue เช่น "ลำดับ") — เรียงหลังกรอง ก่อนแบ่งหน้า
  const sortColumn = config.columns.find((column) => column.sortValue)
  const sort = useSort(visibleItems, (item) => sortColumn?.sortValue?.(item) ?? 0)
  const pagination = usePagination(sort.sorted)

  const handleFormSubmit = async (values: MasterFormValues) => {
    try {
      if (formState?.mode === 'edit') {
        await config.api.update(formState.item.id, values)
      } else {
        await config.api.create(values)
      }
    } catch (err) {
      throw new Error(errorMessage(err, config.errorMessages))
    }
    await refetch()
    notify.saved(formState?.mode === 'edit', `เพิ่ม${config.entityLabel}แล้ว`, `แก้ไข${config.entityLabel}แล้ว`)
  }

  const handleDelete = async () => {
    if (!pendingDelete) return
    setIsDeleting(true)
    try {
      await config.api.remove(pendingDelete.id)
      await refetch()
      notify.deleted(`ลบ${config.entityLabel}แล้ว`)
      setPendingDelete(null)
    } catch (err) {
      notify.error(errorMessage(err, config.errorMessages))
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader fallbackTitle={config.title} description={config.description} />

      <SearchForm
        id="master-search"
        ariaLabel={`ค้นหา${config.entityLabel}`}
        fields={searchFields}
        onSearch={(values) => {
          setCriteria(values)
          pagination.reset()
        }}
      />

      <ListToolbar
        ready={status === 'ready'}
        total={items.length}
        shown={visibleItems.length}
        filtered={hasCriteria(criteria)}
      >
        {canCreate && <AddButton onClick={() => setFormState({ mode: 'create' })}>เพิ่ม{config.entityLabel}</AddButton>}
      </ListToolbar>

      <Card>
        <CardContent className="flex flex-col gap-3 px-0 sm:px-2">
          {status === 'loading' && <ListState kind="loading" />}
          {status === 'error' && (
            <ListState kind="error">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</ListState>
          )}
          {status === 'ready' && items.length === 0 && (
            <ListState kind="empty">
              ยังไม่มี{config.entityLabel} กด &quot;+ เพิ่ม{config.entityLabel}&quot; เพื่อเริ่มต้น
            </ListState>
          )}
          {status === 'ready' && items.length > 0 && visibleItems.length === 0 && (
            <ListState kind="noMatch">
              ไม่พบ{config.entityLabel}ที่ตรงกับเงื่อนไข กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด
            </ListState>
          )}
          {status === 'ready' && visibleItems.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  {config.columns.map((column) => (
                    column.sortValue ? (
                      <SortableTableHead
                        key={column.header}
                        direction={sort.direction}
                        onSort={() => {
                          sort.toggle()
                          pagination.reset()
                        }}
                      >
                        {column.header}
                      </SortableTableHead>
                    ) : (
                      <TableHead key={column.header}>{column.header}</TableHead>
                    )
                  ))}
                  {(canUpdate || canDelete) && <TableHead className="text-right">จัดการ</TableHead>}
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((item) => (
                  <TableRow key={item.id}>
                    {config.columns.map((column) => (
                      <TableCell
                        key={column.header}
                        className={column.className ?? 'text-muted-foreground'}
                      >
                        {column.cell(item)}
                      </TableCell>
                    ))}
                    {(canUpdate || canDelete) && (
                      <TableCell className="text-right">
                        {canUpdate && <EditButton onClick={() => setFormState({ mode: 'edit', item })} />}
                        {canDelete && <DeleteButton onClick={() => setPendingDelete(item)} />}
                      </TableCell>
                    )}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          {status === 'ready' && visibleItems.length > 0 && <TablePagination {...pagination.props} />}
        </CardContent>
      </Card>

      <MasterFormDialog
        open={formState !== null}
        onOpenChange={(open) => {
          if (!open) setFormState(null)
        }}
        mode={formState?.mode ?? 'create'}
        entityLabel={config.entityLabel}
        fields={config.fields}
        initialItem={formState?.mode === 'edit' ? formState.item : undefined}
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
            <AlertDialogTitle>ยืนยันลบ{config.entityLabel}</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการลบ{config.entityLabel} &quot;{pendingDelete ? config.itemLabel(pendingDelete) : ''}
              &quot; ใช่ไหม? {config.deleteNote}
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
