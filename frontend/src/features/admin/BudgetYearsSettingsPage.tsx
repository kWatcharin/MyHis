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
import { ApiError } from '@/lib/api'
import { formatThaiDate } from '@/lib/date'
import { notify } from '@/lib/notify'
import { hasCriteria, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'

import type { BudgetYearFormValues } from './BudgetYearFormDialog'
import { BudgetYearFormDialog } from './BudgetYearFormDialog'
import type { BudgetYear } from './budgetYearsApi'
import { budgetYearsApi } from './budgetYearsApi'
import { useAdminList } from './useAdminList'

// ข้อความ error ที่รู้จักจาก backend (backend/src/budget_year/service.rs) แปลงเป็นไทย
function budgetYearErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    switch (err.message) {
      case 'year already taken':
        return 'ปีงบประมาณนี้มีอยู่แล้ว'
      case 'budget year is still used':
        return 'ลบไม่ได้ ยังมีการนัดหมาย lab ใช้ปีงบประมาณนี้อยู่'
      case 'end date must not be before start date':
        return 'วันที่สิ้นสุดต้องไม่ก่อนวันที่เริ่มต้น'
      case 'year must be 4 digits':
        return 'ปีงบประมาณต้องเป็นเลข 4 หลัก'
    }
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}

type FormState = { mode: 'create' } | { mode: 'edit'; item: BudgetYear } | null

// ต่อ API จริงเต็ม CRUD (backend/src/budget_year) — create/update คืนแค่ status ไม่มี body เลย
// refetch() ทั้ง list หลังทุก mutation เหมือนหน้าตั้งค่าเพจ/role
export function BudgetYearsSettingsPage() {
  const { items, status, refetch } = useAdminList(budgetYearsApi.list)

  const [formState, setFormState] = useState<FormState>(null)
  const [pendingDelete, setPendingDelete] = useState<BudgetYear | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  // เงื่อนไขค้นหาที่ใช้กรองจริง (เปลี่ยนเมื่อกดปุ่ม "ค้นหา")
  const [criteria, setCriteria] = useState<SearchValues>({})
  const visibleItems = items.filter((item) => textIncludes(item.year, criteria.year ?? ''))

  // เรียงตามคอลัมน์ "ลำดับ" เมื่อกดหัวตาราง (หลังกรอง ก่อนแบ่งหน้า)
  const sort = useSort(visibleItems, (row) => row.sortOrder)
  const pagination = usePagination(sort.sorted)

  const handleFormSubmit = async (values: BudgetYearFormValues) => {
    try {
      if (formState?.mode === 'edit') {
        await budgetYearsApi.update(formState.item.id, values)
      } else {
        await budgetYearsApi.create(values)
      }
    } catch (err) {
      throw new Error(budgetYearErrorMessage(err))
    }
    await refetch()
    notify.saved(formState?.mode === 'edit', 'เพิ่มปีงบประมาณแล้ว', 'แก้ไขปีงบประมาณแล้ว')
  }

  const handleDelete = async () => {
    if (!pendingDelete) return
    setIsDeleting(true)
    try {
      await budgetYearsApi.remove(pendingDelete.id)
      await refetch()
      notify.deleted('ลบปีงบประมาณแล้ว')
      setPendingDelete(null)
    } catch (err) {
      notify.error(budgetYearErrorMessage(err))
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        title="ตั้งค่าปีงบประมาณ"
        description="กำหนดปีงบประมาณ (พ.ศ.) และช่วงวันที่ของแต่ละปี ใช้อ้างอิงในนัดหมาย lab"
      />

      <SearchForm
        id="budget-year-search"
        ariaLabel="ค้นหาปีงบประมาณ"
        fields={[{ name: 'year', label: 'ปีงบประมาณ (พ.ศ.)', placeholder: 'เช่น 2570' }]}
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
        <AddButton onClick={() => setFormState({ mode: 'create' })}>เพิ่มปีงบประมาณ</AddButton>
      </ListToolbar>

      <Card>
        <CardContent className="px-0 sm:px-2">
          {status === 'loading' && (
            <p className="p-4 text-sm text-muted-foreground">กำลังโหลด...</p>
          )}
          {status === 'error' && (
            <p className="p-4 text-sm text-destructive">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</p>
          )}
          {status === 'ready' && items.length === 0 && (
            <p className="p-4 text-sm text-muted-foreground">
              ยังไม่มีปีงบประมาณ กด &quot;+ เพิ่มปีงบประมาณ&quot; เพื่อเริ่มต้น
            </p>
          )}
          {status === 'ready' && items.length > 0 && visibleItems.length === 0 && (
            <p className="p-4 text-sm text-muted-foreground">
              ไม่พบปีงบประมาณที่ตรงกับเงื่อนไข — กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด
            </p>
          )}
          {status === 'ready' && visibleItems.length > 0 && (
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
                  <TableHead>ปีงบประมาณ</TableHead>
                  <TableHead>ช่วงวันที่</TableHead>
                  <TableHead>สถานะ</TableHead>
                  <TableHead className="text-right">จัดการ</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((item) => (
                  <TableRow key={item.id}>
                    <TableCell className="text-muted-foreground">{item.sortOrder}</TableCell>
                    <TableCell className="font-medium">{item.year}</TableCell>
                    <TableCell className="text-muted-foreground">
                      {formatThaiDate(item.startDate)} – {formatThaiDate(item.endDate)}
                    </TableCell>
                    <TableCell>
                      <Badge variant={item.isActive ? 'default' : 'outline'}>
                        {item.isActive ? 'ใช้งานอยู่' : 'ปิดใช้งาน'}
                      </Badge>
                    </TableCell>
                    <TableCell className="text-right">
                      <EditButton onClick={() => setFormState({ mode: 'edit', item })} />
                      <DeleteButton onClick={() => setPendingDelete(item)} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          {status === 'ready' && visibleItems.length > 0 && <TablePagination {...pagination.props} />}
        </CardContent>
      </Card>

      <BudgetYearFormDialog
        open={formState !== null}
        onOpenChange={(open) => {
          if (!open) setFormState(null)
        }}
        mode={formState?.mode ?? 'create'}
        initialValue={formState?.mode === 'edit' ? formState.item : undefined}
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
            <AlertDialogTitle>ยืนยันลบปีงบประมาณ</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการลบปีงบประมาณ {pendingDelete?.year} ใช่ไหม? ถ้ายังมีนัดหมาย lab ใช้ปีนี้อยู่
              ระบบจะไม่ให้ลบ
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
