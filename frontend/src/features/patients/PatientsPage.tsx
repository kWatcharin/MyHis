import { useCallback, useEffect, useState } from 'react'

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
import { useAdminList } from '@/features/admin/useAdminList'
import { usePagePermissions } from '@/features/role-preview/usePagePermissions'
import { ApiError } from '@/lib/api'
import { notify } from '@/lib/notify'
import { allTokensIncluded, hasCriteria, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'

import { PatientFormDialog } from './PatientFormDialog'
import type { Patient, PatientInput, PatientOptions } from './patientsApi'
import { patientsApi } from './patientsApi'

// ข้อความ error ที่รู้จักจาก backend (backend/src/patient/service.rs) แปลงเป็นไทย
const KNOWN_ERRORS: Record<string, string> = {
  'hn already registered': 'HN นี้ลงทะเบียนแล้ว',
  'unknown prename': 'ไม่พบคำนำหน้านี้ (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า',
  'unknown nationality': 'ไม่พบสัญชาตินี้ (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า',
  'unknown disease': 'ไม่พบโรคที่เลือก (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า',
  'disease is inactive': 'มีโรคที่ปิดใช้งานแล้ว เพิ่มเป็นโรคประจำตัวใหม่ไม่ได้',
  'birth date must not be in the future': 'วันเกิดต้องไม่เป็นอนาคต',
  'birth date is too far in the past': 'วันเกิดเก่าเกินไป ตรวจสอบอีกครั้ง',
  'sex must be 1 or 2': 'เลือกเพศให้ถูกต้อง',
  'patient not found': 'ไม่พบผู้ป่วยนี้ (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า',
}

function errorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    if (KNOWN_ERRORS[err.message]) return KNOWN_ERRORS[err.message]
    if (err.message.endsWith(' is required')) return 'กรอกข้อมูลที่จำเป็นให้ครบ'
    if (/must be at most \d+ characters$/.test(err.message)) return 'ข้อความยาวเกินกำหนด'
    if (err.message.endsWith('is out of range')) return 'น้ำหนัก/ส่วนสูง/อายุ อยู่นอกช่วงที่รับได้'
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}

const SEX_LABEL: Record<number, string> = { 1: 'ชาย', 2: 'หญิง' }

function fullName(p: Patient): string {
  return `${p.prenameName}${p.firstName} ${p.lastName}`
}

type FormState = { mode: 'create' } | { mode: 'edit'; item: Patient } | null

// เงื่อนไขค้นหา 3 ช่อง (ชื่อ-สกุล, HN, เลขประจำตัวคนไข้; ว่าง = ไม่กรองช่องนั้น ช่องที่กรอกต้องตรงทุกช่อง)
// กรองฝั่ง client เพราะชื่อ/HN เข้ารหัสไว้ค้นใน SQL ไม่ได้ และจะไม่ส่งค่าเหล่านี้ผ่าน URL/query string
function matchesCriteria(patient: Patient, criteria: SearchValues): boolean {
  // ชื่อแยกคำได้ ("สมชาย ใจดี" หรือ "ใจดี สมชาย") ทุกคำต้องอยู่ในคำนำหน้า+ชื่อ+สกุลของผู้ป่วย
  const fullNameText = `${patient.prenameName} ${patient.firstName} ${patient.lastName}`
  return (
    allTokensIncluded(fullNameText, criteria.name ?? '') &&
    textIncludes(patient.hn, criteria.hn ?? '') &&
    textIncludes(patient.napNo, criteria.napNo ?? '')
  )
}

// หน้าลงทะเบียนผู้ป่วย: รายการผู้ป่วย (ข้อมูลระบุตัวถูกถอดรหัสโดย backend) + เพิ่ม/แก้ไข/ลบ + โรคประจำตัวเป็นป้ายต่อโรค
// ดูได้ด้วย patient.view; ปุ่มเพิ่ม/แก้ไข/ลบแสดงตาม patient.create/update/delete (สิทธิ์ที่ใช้งานอยู่ตามโหมดสลับ role)
// backend ตรวจสิทธิ์ซ้ำทุกครั้ง การซ่อนปุ่มเป็นแค่ความสะดวก ฟอร์มค้นหา (ชื่อ, HN, เลขประจำตัวคนไข้) กรองฝั่ง
// client เพราะข้อมูลเข้ารหัสไว้ค้นใน SQL ไม่ได้ (และไม่ส่ง HN/ชื่อผ่าน URL) ไม่เก็บข้อมูลผู้ป่วยลง
// localStorage หรือ console
export function PatientsPage() {
  const { canCreate, canUpdate, canDelete } = usePagePermissions('patient')

  const { items, status, refetch } = useAdminList(patientsApi.list)
  const [options, setOptions] = useState<PatientOptions | null>(null)
  const [optionsFailed, setOptionsFailed] = useState(false)

  const [formState, setFormState] = useState<FormState>(null)
  const [pendingDelete, setPendingDelete] = useState<Patient | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  // เงื่อนไขค้นหาที่ใช้กรองจริง (เปลี่ยนเมื่อกดปุ่ม "ค้นหา"/Enter ในฟอร์มค้นหา)
  const [criteria, setCriteria] = useState<SearchValues>({})
  // ตัวเลือกของฟอร์ม (คำนำหน้า, สัญชาติ, โรค) — โหลดตอนเปิดหน้าและโหลดใหม่ทุกครั้งที่เปิดฟอร์ม
  // เผื่อมีการแก้ master จากหน้าตั้งค่าระหว่างที่หน้านี้เปิดค้างอยู่
  const loadOptions = useCallback(async () => {
    try {
      setOptions(await patientsApi.options())
      setOptionsFailed(false)
    } catch {
      setOptionsFailed(true)
    }
  }, [])

  useEffect(() => {
    void loadOptions()
  }, [loadOptions])

  const openForm = (state: Exclude<FormState, null>) => {
    void loadOptions()
    setFormState(state)
  }

  const visibleItems = items.filter((patient) => matchesCriteria(patient, criteria))

  // เรียงตามคอลัมน์ "ลำดับ" เมื่อกดหัวตาราง (หลังกรอง ก่อนแบ่งหน้า)
  const sort = useSort(visibleItems, (row) => row.sortOrder)
  const pagination = usePagination(sort.sorted)

  const handleSearch = (values: SearchValues) => {
    setCriteria(values)
    pagination.reset()
  }

  const handleFormSubmit = async (input: PatientInput) => {
    try {
      if (formState?.mode === 'edit') {
        await patientsApi.update(formState.item.id, input)
      } else {
        await patientsApi.create(input)
      }
    } catch (err) {
      throw new Error(errorMessage(err))
    }
    await refetch()
    notify.saved(formState?.mode === 'edit', 'ลงทะเบียนผู้ป่วยแล้ว', 'แก้ไขข้อมูลผู้ป่วยแล้ว')
  }

  const handleDelete = async () => {
    if (!pendingDelete) return
    setIsDeleting(true)
    try {
      await patientsApi.remove(pendingDelete.id)
      await refetch()
      notify.deleted('ลบผู้ป่วยแล้ว')
      setPendingDelete(null)
    } catch (err) {
      notify.error(errorMessage(err))
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        fallbackTitle="ลงทะเบียนผู้ป่วย"
        description="ทะเบียนผู้ป่วยและโรคประจำตัว ข้อมูลระบุตัวถูกเข้ารหัสก่อนเก็บลงฐานข้อมูล"
      />

      <SearchForm
        id="patient-search"
        ariaLabel="ค้นหาผู้ป่วย"
        fields={[
          { name: 'name', label: 'ชื่อ-สกุล', placeholder: 'ชื่อ หรือ สกุล' },
          { name: 'hn', label: 'HN' },
          { name: 'napNo', label: 'เลขประจำตัวคนไข้' },
        ]}
        onSearch={handleSearch}
      />

      <ListToolbar
        ready={status === 'ready'}
        total={items.length}
        shown={visibleItems.length}
        filtered={hasCriteria(criteria)}
      >
        {canCreate && <AddButton onClick={() => openForm({ mode: 'create' })}>ลงทะเบียนผู้ป่วย</AddButton>}
      </ListToolbar>

      <Card>
        <CardContent className="flex flex-col gap-3 px-0 sm:px-2">
          {status === 'loading' && <ListState kind="loading" />}
          {status === 'error' && (
            <ListState kind="error">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</ListState>
          )}
          {status === 'ready' && items.length === 0 && (
            <ListState kind="empty">
              ยังไม่มีผู้ป่วย{canCreate ? ' กด "+ ลงทะเบียนผู้ป่วย" เพื่อเริ่มต้น' : ''}
            </ListState>
          )}
          {status === 'ready' && items.length > 0 && visibleItems.length === 0 && (
            <ListState kind="noMatch">
              ไม่พบผู้ป่วยที่ตรงกับเงื่อนไข กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด
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
                  <TableHead>HN</TableHead>
                  <TableHead>ชื่อ-สกุล</TableHead>
                  <TableHead>เพศ / อายุ</TableHead>
                  <TableHead>โรคประจำตัว</TableHead>
                  <TableHead>สถานะ</TableHead>
                  {(canUpdate || canDelete) && <TableHead className="text-right">จัดการ</TableHead>}
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((patient) => (
                  <TableRow key={patient.id}>
                    <TableCell className="text-muted-foreground">{patient.sortOrder}</TableCell>
                    <TableCell className="font-mono text-sm">{patient.hn}</TableCell>
                    <TableCell className="font-medium">
                      {fullName(patient)}
                      <div className="text-xs font-normal text-muted-foreground">
                        เลขประจำตัว {patient.napNo}
                      </div>
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {SEX_LABEL[patient.sex]}
                      {patient.age !== null ? ` · ${patient.age} ปี` : ''}
                    </TableCell>
                    <TableCell>
                      {patient.conditions.length === 0 ? (
                        <span className="text-sm text-muted-foreground">—</span>
                      ) : (
                        <div className="flex flex-wrap gap-1">
                          {patient.conditions.map((condition) => (
                            <Badge key={condition.id} variant="outline" title={condition.name}>
                              {condition.abbreviation}
                            </Badge>
                          ))}
                        </div>
                      )}
                    </TableCell>
                    <TableCell>
                      <Badge variant={patient.isActive ? 'success' : 'outline'}>
                        {patient.isActive ? 'ใช้งานอยู่' : 'ปิดใช้งาน'}
                      </Badge>
                    </TableCell>
                    {(canUpdate || canDelete) && (
                      <TableCell className="text-right">
                        {canUpdate && <EditButton onClick={() => openForm({ mode: 'edit', item: patient })} />}
                        {canDelete && <DeleteButton onClick={() => setPendingDelete(patient)} />}
                      </TableCell>
                    )}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          {status === 'ready' && visibleItems.length > 0 && <TablePagination {...pagination.props} />}
          {optionsFailed && (canCreate || canUpdate) && (
            <p className="px-4 pb-2 text-sm text-destructive">
              โหลดตัวเลือกของฟอร์ม (คำนำหน้า สัญชาติ โรค) ไม่สำเร็จ ลองรีเฟรชหน้า
            </p>
          )}
        </CardContent>
      </Card>

      <PatientFormDialog
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
            <AlertDialogTitle>ยืนยันลบผู้ป่วย</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการลบผู้ป่วย HN {pendingDelete?.hn} ใช่ไหม? โรคประจำตัวที่ผูกไว้จะถูกลบไปด้วย
              และ HN นี้จะใช้ลงทะเบียนใหม่ได้
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
