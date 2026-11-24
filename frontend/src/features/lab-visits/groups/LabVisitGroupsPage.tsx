import { ListChecks } from 'lucide-react'
import { useCallback, useState } from 'react'
import { Link, useNavigate } from 'react-router'

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
import { useAdminList } from '@/features/admin/useAdminList'
import { usePagePermissions } from '@/features/role-preview/usePagePermissions'
import { formatThaiDate } from '@/lib/date'
import { notify } from '@/lib/notify'
import { hasCriteria, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'

import { labVisitErrorMessage } from '../errors'
import { LabVisitGroupFormDialog } from '../LabVisitGroupFormDialog'
import type { LabVisitGroup, LabVisitGroupInput, LabVisitOptions } from '../labVisitsApi'
import { labVisitsApi } from '../labVisitsApi'

type FormState = { mode: 'create' } | { mode: 'edit'; item: LabVisitGroup } | null

// หน้า "นัดหมายแล็ป" (เมนูกลุ่ม Lab): รายการนัดหมายต่อปีงบประมาณ + กลุ่มคนไข้ (head) — กดดูรายชื่อเพื่อไปหน้า
// รายชื่อคนไข้ของนัดหมายนั้น ดูได้ด้วย lab_visit.view; สร้าง/แก้ไข/ลบต้องมี lab_visit.create/update/delete (ซ่อนปุ่มเป็นแค่ความสะดวก
// backend ตรวจสิทธิ์ซ้ำทุกครั้ง) หน้านี้ไม่มีข้อมูลระบุตัวคนไข้ มีแค่จำนวนคน
export function LabVisitGroupsPage() {
  const navigate = useNavigate()
  const { canCreate, canUpdate, canDelete } = usePagePermissions('lab_visit')

  const { items, status, refetch } = useAdminList(labVisitsApi.list)
  const [options, setOptions] = useState<LabVisitOptions | null>(null)
  const [formState, setFormState] = useState<FormState>(null)
  const [pendingDelete, setPendingDelete] = useState<LabVisitGroup | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  const [criteria, setCriteria] = useState<SearchValues>({})

  // ตัวเลือกของฟอร์ม โหลดใหม่ทุกครั้งที่เปิดฟอร์ม (เผื่อมีการแก้ master ระหว่างที่หน้านี้เปิดค้าง)
  const loadOptions = useCallback(async () => {
    try {
      setOptions(await labVisitsApi.options())
    } catch {
      notify.error('โหลดตัวเลือกปีงบประมาณ/กลุ่มคนไข้ไม่สำเร็จ ลองรีเฟรชหน้า')
    }
  }, [])

  const openForm = (state: Exclude<FormState, null>) => {
    void loadOptions()
    setFormState(state)
  }

  const visibleItems = items.filter(
    (item) =>
      textIncludes(item.budgetYear, criteria.budgetYear ?? '') &&
      textIncludes(item.patientGroupName, criteria.patientGroup ?? ''),
  )
  const sort = useSort(visibleItems, (row) => row.sortOrder)
  const pagination = usePagination(sort.sorted)

  const handleFormSubmit = async (input: LabVisitGroupInput) => {
    let createdId: string | null = null
    try {
      if (formState?.mode === 'edit') {
        await labVisitsApi.update(formState.item.id, input)
      } else {
        createdId = (await labVisitsApi.create(input)).id
      }
    } catch (err) {
      throw new Error(labVisitErrorMessage(err))
    }
    if (createdId) {
      notify.created('สร้างนัดหมายแล้ว เพิ่มคนไข้เข้านัดหมายได้เลย')
      navigate(`/lab/appointments/${createdId}`)
      return
    }
    await refetch()
    notify.updated('แก้ไขนัดหมายแล้ว')
  }

  const handleDelete = async () => {
    if (!pendingDelete) return
    setIsDeleting(true)
    try {
      await labVisitsApi.remove(pendingDelete.id)
      await refetch()
      notify.deleted('ลบนัดหมายแล้ว')
      setPendingDelete(null)
    } catch (err) {
      notify.error(labVisitErrorMessage(err))
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        fallbackTitle="นัดหมายแล็ป"
        description="นัดตรวจ CD4/VL ของกลุ่มคนไข้ต่อปีงบประมาณ กด &quot;ดูรายชื่อ&quot; เพื่อเพิ่ม/แก้ไขคนไข้และส่งออกเอกสาร"
      />

      <SearchForm
        id="lab-group-search"
        ariaLabel="ค้นหานัดหมายแล็ป"
        fields={[
          { name: 'budgetYear', label: 'ปีงบประมาณ', placeholder: 'เช่น 2569' },
          { name: 'patientGroup', label: 'กลุ่มคนไข้' },
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
        {canCreate && <AddButton onClick={() => openForm({ mode: 'create' })}>สร้างนัดหมาย</AddButton>}
      </ListToolbar>

      <Card>
        <CardContent className="flex flex-col gap-3 px-0 sm:px-2">
          {status === 'loading' && <ListState kind="loading" />}
          {status === 'error' && (
            <ListState kind="error">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</ListState>
          )}
          {status === 'ready' && items.length === 0 && (
            <ListState kind="empty">
              ยังไม่มีนัดหมาย{canCreate ? ' กด "+ สร้างนัดหมาย" เพื่อเริ่มต้น' : ''}
            </ListState>
          )}
          {status === 'ready' && items.length > 0 && visibleItems.length === 0 && (
            <ListState kind="noMatch">
              ไม่พบนัดหมายที่ตรงกับเงื่อนไข กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด
            </ListState>
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
                  <TableHead>กลุ่มคนไข้</TableHead>
                  <TableHead>CD4 ครั้งที่ 1</TableHead>
                  <TableHead>CD4 ครั้งที่ 2</TableHead>
                  <TableHead>นัดตรวจ VL</TableHead>
                  <TableHead className="text-right">คนไข้</TableHead>
                  <TableHead className="text-right">จัดการ</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((item) => (
                  <TableRow key={item.id}>
                    <TableCell className="text-muted-foreground">{item.sortOrder}</TableCell>
                    <TableCell>{item.budgetYear}</TableCell>
                    <TableCell className="font-medium">
                      <Link to={`/lab/appointments/${item.id}`} className="hover:underline">
                        {item.patientGroupName}
                      </Link>
                    </TableCell>
                    <TableCell className="text-muted-foreground">{formatThaiDate(item.cd4FirstDate)}</TableCell>
                    <TableCell className="text-muted-foreground">{formatThaiDate(item.cd4SecondDate)}</TableCell>
                    <TableCell className="text-muted-foreground">{formatThaiDate(item.vlDate)}</TableCell>
                    <TableCell className="text-right">{item.patientCount} คน</TableCell>
                    <TableCell className="text-right whitespace-nowrap">
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={() => navigate(`/lab/appointments/${item.id}`)}
                      >
                        <ListChecks />
                        ดูรายชื่อ
                      </Button>
                      {canUpdate && <EditButton onClick={() => openForm({ mode: 'edit', item })} />}
                      {canDelete && <DeleteButton onClick={() => setPendingDelete(item)} />}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          {status === 'ready' && visibleItems.length > 0 && <TablePagination {...pagination.props} />}
        </CardContent>
      </Card>

      <LabVisitGroupFormDialog
        open={formState !== null}
        onOpenChange={(open) => {
          if (!open) setFormState(null)
        }}
        mode={formState?.mode ?? 'create'}
        initialValue={formState?.mode === 'edit' ? formState.item : undefined}
        options={options}
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
            <AlertDialogTitle>ยืนยันลบนัดหมาย</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการลบนัดหมายกลุ่ม &quot;{pendingDelete?.patientGroupName}&quot; ปีงบประมาณ{' '}
              {pendingDelete?.budgetYear} ใช่ไหม? รายชื่อคนไข้ {pendingDelete?.patientCount ?? 0} คน
              ในนัดหมายนี้จะถูกลบไปด้วย (ข้อมูลในทะเบียนผู้ป่วยไม่ถูกลบ)
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
