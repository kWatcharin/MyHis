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
import { notify } from '@/lib/notify'
import { hasCriteria, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'

import type { DiseaseFormValues } from './DiseaseFormDialog'
import { DiseaseFormDialog } from './DiseaseFormDialog'
import type { Disease } from './diseasesApi'
import { diseasesApi } from './diseasesApi'
import { useAdminList } from './useAdminList'

// ข้อความ error ที่รู้จักจาก backend (backend/src/disease/service.rs) แปลงเป็นไทย
function diseaseErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    switch (err.message) {
      case 'disease name already taken':
        return 'ชื่อโรคนี้มีอยู่แล้ว'
      case 'disease is still used':
        return 'ลบไม่ได้ ยังมีคนไข้ที่บันทึกโรคนี้เป็นโรคประจำตัวอยู่'
      case 'name is required':
        return 'กรอกชื่อโรค'
      case 'abbreviation is required':
        return 'กรอกคำย่อ'
    }
    if (err.message.endsWith('must be at most 100 characters')) {
      return 'ชื่อโรคหรือคำย่อยาวเกิน 100 ตัวอักษร'
    }
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}

type FormState = { mode: 'create' } | { mode: 'edit'; item: Disease } | null

// ต่อ API จริงเต็ม CRUD (backend/src/disease) — create/update คืนแค่ status ไม่มี body เลย
// refetch() ทั้ง list หลังทุก mutation เหมือนหน้าตั้งค่าเพจ/role/ปีงบประมาณ
export function DiseasesSettingsPage() {
  const { items, status, refetch } = useAdminList(diseasesApi.list)

  const [formState, setFormState] = useState<FormState>(null)
  const [pendingDelete, setPendingDelete] = useState<Disease | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  // เงื่อนไขค้นหาที่ใช้กรองจริง (เปลี่ยนเมื่อกดปุ่ม "ค้นหา") — ช่องที่กรอกต้องตรงทุกช่อง
  const [criteria, setCriteria] = useState<SearchValues>({})
  const visibleItems = items.filter(
    (item) =>
      textIncludes(item.name, criteria.name ?? '') &&
      textIncludes(item.abbreviation, criteria.abbreviation ?? ''),
  )

  // เรียงตามคอลัมน์ "ลำดับ" เมื่อกดหัวตาราง (หลังกรอง ก่อนแบ่งหน้า)
  const sort = useSort(visibleItems, (row) => row.sortOrder)
  const pagination = usePagination(sort.sorted)

  const handleFormSubmit = async (values: DiseaseFormValues) => {
    try {
      if (formState?.mode === 'edit') {
        await diseasesApi.update(formState.item.id, values)
      } else {
        await diseasesApi.create(values)
      }
    } catch (err) {
      throw new Error(diseaseErrorMessage(err))
    }
    await refetch()
    notify.saved(formState?.mode === 'edit', 'เพิ่มโรคแล้ว', 'แก้ไขโรคแล้ว')
  }

  const handleDelete = async () => {
    if (!pendingDelete) return
    setIsDeleting(true)
    try {
      await diseasesApi.remove(pendingDelete.id)
      await refetch()
      notify.deleted('ลบโรคแล้ว')
      setPendingDelete(null)
    } catch (err) {
      notify.error(diseaseErrorMessage(err))
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader title="ตั้งค่าโรค" description="รายชื่อโรคที่ใช้บันทึกเป็นโรคประจำตัวของคนไข้" />

      <SearchForm
        id="disease-search"
        ariaLabel="ค้นหาโรค"
        fields={[
          { name: 'name', label: 'ชื่อโรค' },
          { name: 'abbreviation', label: 'คำย่อ' },
        ]}
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
        <AddButton onClick={() => setFormState({ mode: 'create' })}>เพิ่มโรค</AddButton>
      </ListToolbar>

      <Card>
        <CardContent className="flex flex-col gap-3 px-0 sm:px-2">
          {status === 'loading' && (
            <p className="p-4 text-sm text-muted-foreground">กำลังโหลด...</p>
          )}
          {status === 'error' && (
            <p className="p-4 text-sm text-destructive">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</p>
          )}
          {status === 'ready' && items.length === 0 && (
            <p className="p-4 text-sm text-muted-foreground">
              ยังไม่มีโรค กด &quot;+ เพิ่มโรค&quot; เพื่อเริ่มต้น
            </p>
          )}
          {status === 'ready' && items.length > 0 && visibleItems.length === 0 && (
            <p className="p-4 text-sm text-muted-foreground">ไม่พบโรคที่ตรงกับเงื่อนไข — กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด</p>
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
                  <TableHead>ชื่อโรค</TableHead>
                  <TableHead>คำย่อ</TableHead>
                  <TableHead>สถานะ</TableHead>
                  <TableHead className="text-right">จัดการ</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((item) => (
                  <TableRow key={item.id}>
                    <TableCell className="text-muted-foreground">{item.sortOrder}</TableCell>
                    <TableCell className="font-medium">{item.name}</TableCell>
                    <TableCell className="text-muted-foreground">{item.abbreviation}</TableCell>
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

      <DiseaseFormDialog
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
            <AlertDialogTitle>ยืนยันลบโรค</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการลบโรค &quot;{pendingDelete?.name}&quot; ใช่ไหม? ถ้ายังมีคนไข้บันทึกโรคนี้เป็นโรคประจำตัวอยู่
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
