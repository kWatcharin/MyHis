import { ArrowLeft, Eye, EyeOff } from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useNavigate, useParams } from 'react-router'

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
import type { Patient } from '@/features/patients/patientsApi'
import { patientsApi } from '@/features/patients/patientsApi'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'
import { usePagePermissions } from '@/features/role-preview/usePagePermissions'
import { ApiError } from '@/lib/api'
import { formatThaiDate, todayIso } from '@/lib/date'
import { notify } from '@/lib/notify'
import { allTokensIncluded, hasCriteria, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'

import type { JoinedVisit, LabDocumentKind } from './documents'
import {
  LAB_DOCUMENTS,
  buildLabDocumentData,
  joinVisits,
  patientDisplayId,
  patientDisplayName,
} from './documents'
import { labVisitErrorMessage } from './errors'
import { downloadLabExcel } from './excel'
import { ExportMenu } from './ExportMenu'
import { LabVisitFormDialog } from './LabVisitFormDialog'
import { LabVisitGroupFormDialog } from './LabVisitGroupFormDialog'
import type {
  LabVisit,
  LabVisitGroupDetail,
  LabVisitGroupInput,
  LabVisitInput,
  LabVisitOptions,
} from './labVisitsApi'
import { labVisitsApi } from './labVisitsApi'
import type { PreviousRound } from './PatientPickerDialog'
import { PatientPickerDialog } from './PatientPickerDialog'

type LoadStatus = 'loading' | 'ready' | 'error' | 'notFound'

// โหมดซ่อนชื่อ, HN, NAP NO (ปุ่มรูปตา) จำไว้ในเบราว์เซอร์นี้ — เก็บแค่ค่าเปิด/ปิด ไม่ใช่ข้อมูลคนไข้ เปิดไว้แล้ว
// เปลี่ยนหน้าหรือรีเฟรชก็ยังซ่อนอยู่ (กันข้อมูลโผล่ระหว่างแชร์หน้าจอ) ค่า key คงชื่อเดิมตั้งแต่ตอนซ่อนแค่ชื่อ อย่าเปลี่ยน
// ไม่งั้นคนที่เปิดซ่อนไว้แล้วจะเห็นข้อมูลเต็มทันทีหลังอัปเดต
const MASK_IDENTITY_KEY = 'my-his-lab-visit-mask-names'

function readMaskIdentity(): boolean {
  try {
    return localStorage.getItem(MASK_IDENTITY_KEY) === '1'
  } catch {
    return false
  }
}

function takeMedText(visit: LabVisit): string {
  const parts = [
    visit.takeMedYearRange !== null ? `${visit.takeMedYearRange} ปี` : null,
    visit.takeMedMonthRange !== null ? `${visit.takeMedMonthRange} เดือน` : null,
  ].filter(Boolean)
  return parts.length > 0 ? parts.join(' ') : '—'
}

// ชื่อ (แยกคำ สลับลำดับได้), HN, NAP NO — ช่องที่กรอกต้องตรงทุกช่อง กรองฝั่ง client (ข้อมูลเข้ารหัสใน DB)
function matchesCriteria({ patient }: JoinedVisit, criteria: SearchValues): boolean {
  const fullNameText = patient ? `${patient.prenameName} ${patient.firstName} ${patient.lastName}` : ''
  return (
    allTokensIncluded(fullNameText, criteria.name ?? '') &&
    textIncludes(patient?.hn, criteria.hn ?? '') &&
    textIncludes(patient?.napNo, criteria.napNo ?? '')
  )
}

// รายชื่อคนไข้ของนัดหมายหนึ่งรายการ: หัวนัดหมาย (วันนัด CD4/VL) + คนไข้ในนัดหมาย (ข้อมูลระบุตัวมาจาก GET /patients
// แล้ว join ด้วย patientId — API ของนัดหมายไม่มีชื่อ/HN) + เพิ่ม/แก้/เอาออก + ส่งออกเอกสาร Excel/PDF
// ไม่เก็บข้อมูลคนไข้ลง localStorage/URL/console (localStorage มีแค่ค่าเปิด/ปิดโหมดซ่อนชื่อ, HN, NAP NO)
export function LabVisitGroupDetailPage() {
  const { id = '' } = useParams()
  const navigate = useNavigate()
  const { hasEffectivePermission } = useRolePreview()
  // แก้หัวนัดหมาย + เพิ่ม/แก้/เอาคนไข้ออก = แก้ไขนัดหมาย (lab_visit.update) ปุ่มส่งออก = lab_visit.export
  const { canUpdate, canExport } = usePagePermissions('lab_visit')
  // ชื่อ/HN มาจากทะเบียนผู้ป่วย (GET /patients) — ไม่มีสิทธิ์ดูผู้ป่วยก็ใช้หน้านี้ไม่ได้ ไม่ยิง API ให้โดน 403
  const canViewPatients = hasEffectivePermission('patient.view')

  const [detail, setDetail] = useState<LabVisitGroupDetail | null>(null)
  const [patients, setPatients] = useState<Patient[]>([])
  const [status, setStatus] = useState<LoadStatus>('loading')
  const [options, setOptions] = useState<LabVisitOptions | null>(null)

  const [headFormOpen, setHeadFormOpen] = useState(false)
  const [pickerOpen, setPickerOpen] = useState(false)
  const [previousRound, setPreviousRound] = useState<PreviousRound | null>(null)
  const [editing, setEditing] = useState<JoinedVisit | null>(null)
  const [pendingRemove, setPendingRemove] = useState<JoinedVisit | null>(null)
  const [isRemoving, setIsRemoving] = useState(false)
  const [criteria, setCriteria] = useState<SearchValues>({})
  const [maskIdentity, setMaskIdentity] = useState(readMaskIdentity)

  const toggleMaskIdentity = () => {
    const next = !maskIdentity
    setMaskIdentity(next)
    try {
      localStorage.setItem(MASK_IDENTITY_KEY, next ? '1' : '0')
    } catch {
      // localStorage ใช้ไม่ได้ (private mode ฯลฯ) — ยังสลับได้ปกติ แค่ไม่จำข้ามรีเฟรช
    }
  }

  // โหลดนัดหมาย + ทะเบียนคนไข้พร้อมกัน — หลังแก้ไขเรียกซ้ำโดยไม่กลับไปสถานะ loading (ตารางไม่กระพริบ)
  const load = useCallback(async () => {
    try {
      const [group, registry] = await Promise.all([labVisitsApi.get(id), patientsApi.list()])
      setDetail(group)
      setPatients(registry)
      setStatus('ready')
    } catch (err) {
      setStatus(err instanceof ApiError && err.status === 404 ? 'notFound' : 'error')
    }
  }, [id])

  useEffect(() => {
    if (canViewPatients) void load()
  }, [load, canViewPatients])

  // เปลี่ยนไปนัดหมายอื่น (id ใน URL เปลี่ยน) → กลับไปสถานะกำลังโหลด ไม่โชว์ข้อมูลของนัดหมายเดิมค้าง
  const [shownId, setShownId] = useState(id)
  if (shownId !== id) {
    setShownId(id)
    setStatus('loading')
    setDetail(null)
  }

  const joined = useMemo(
    () => (detail ? joinVisits(detail.visits, patients) : []),
    [detail, patients],
  )
  const excludedIds = useMemo(
    () => new Set(detail?.visits.map((visit) => visit.patientId) ?? []),
    [detail],
  )
  const visibleRows = joined.filter((row) => matchesCriteria(row, criteria))
  const sort = useSort(visibleRows, (row) => row.visit.sortOrder)
  const pagination = usePagination(sort.sorted)

  const openHeadForm = async () => {
    setHeadFormOpen(true)
    try {
      setOptions(await labVisitsApi.options())
    } catch {
      notify.error('โหลดตัวเลือกปีงบประมาณ/กลุ่มคนไข้ไม่สำเร็จ ลองรีเฟรชหน้า')
    }
  }

  // ปุ่มลัด "เลือกชุดเดียวกับนัดหมายครั้งก่อน": นัดหมายของกลุ่มเดียวกันที่ปีงบล่าสุดก่อนปีนี้ (ถ้ามี)
  const openPicker = async () => {
    setPreviousRound(null)
    setPickerOpen(true)
    if (!detail) return
    try {
      const previous = (await labVisitsApi.list())
        .filter(
          (group) =>
            group.patientGroupId === detail.patientGroupId &&
            Number(group.budgetYear) < Number(detail.budgetYear),
        )
        .sort((a, b) => Number(b.budgetYear) - Number(a.budgetYear))[0]
      if (!previous) return
      const previousDetail = await labVisitsApi.get(previous.id)
      setPreviousRound({
        label: `นัดหมายปีงบ ${previous.budgetYear}`,
        patientIds: previousDetail.visits.map((visit) => visit.patientId),
      })
    } catch {
      // ไม่มีปุ่มลัดก็ยังเลือกเองได้ตามปกติ
    }
  }

  const handleHeadSubmit = async (input: LabVisitGroupInput) => {
    try {
      await labVisitsApi.update(id, input)
    } catch (err) {
      throw new Error(labVisitErrorMessage(err))
    }
    await load()
    notify.updated('แก้ไขนัดหมายแล้ว')
  }

  const handleAddPatients = async (patientIds: string[]) => {
    try {
      await labVisitsApi.addVisits(id, patientIds)
    } catch (err) {
      throw new Error(labVisitErrorMessage(err))
    }
    await load()
    notify.created(`เพิ่มคนไข้เข้านัดหมาย ${patientIds.length} คนแล้ว`)
  }

  const handleVisitSubmit = async (input: LabVisitInput) => {
    if (!editing) return
    try {
      await labVisitsApi.updateVisit(editing.visit.id, input)
    } catch (err) {
      throw new Error(labVisitErrorMessage(err))
    }
    await load()
    notify.updated('แก้ไขข้อมูลคนไข้ในนัดหมายแล้ว')
  }

  const handleRemove = async () => {
    if (!pendingRemove) return
    setIsRemoving(true)
    try {
      await labVisitsApi.removeVisit(pendingRemove.visit.id)
      await load()
      notify.deleted('เอาคนไข้ออกจากนัดหมายแล้ว')
      setPendingRemove(null)
    } catch (err) {
      notify.error(labVisitErrorMessage(err))
    } finally {
      setIsRemoving(false)
    }
  }

  // บันทึกการส่งออก (ประวัติการใช้งาน + backend ตรวจ lab_visit.export) ก่อนสร้างไฟล์ — บันทึกไม่ได้ = ไม่ส่งออก
  const handleExcel = async (kind: LabDocumentKind) => {
    if (!detail) return
    try {
      await labVisitsApi.recordExport(id, kind, 'excel')
    } catch (err) {
      notify.error(`ส่งออกไม่สำเร็จ: ${labVisitErrorMessage(err)}`)
      return
    }
    try {
      await downloadLabExcel(kind, buildLabDocumentData(detail, patients, todayIso()))
      notify.exported(`ส่งออก${LAB_DOCUMENTS[kind].title}เป็น Excel แล้ว`)
    } catch {
      notify.error('สร้างไฟล์ Excel ไม่สำเร็จ ลองใหม่อีกครั้ง')
    }
  }

  // PDF: เปิดหน้าเอกสารพร้อมพิมพ์ในแท็บใหม่ (URL มีแค่ uuid ของนัดหมาย ไม่มีข้อมูลคนไข้)
  const handlePdf = (kind: LabDocumentKind) => {
    const printWindow = window.open(`/lab/appointments/${id}/print?doc=${kind}`, '_blank')
    if (!printWindow) {
      notify.error('เบราว์เซอร์บล็อกหน้าต่างใหม่ — อนุญาต pop-up ของเว็บนี้แล้วลองอีกครั้ง')
      return
    }
    printWindow.opener = null
    notify.exported('เปิดหน้าพิมพ์แล้ว — เลือก “บันทึกเป็น PDF” ในหน้าต่างพิมพ์')
  }

  const backButton = (
    <Button variant="outline" onClick={() => navigate('/lab/appointments')}>
      <ArrowLeft />
      กลับไปรายการนัดหมาย
    </Button>
  )
  const maskLabel = maskIdentity ? 'แสดงชื่อ, HN, NAP NO' : 'ซ่อนชื่อ, HN, NAP NO'

  if (!canViewPatients) {
    return (
      <div className="flex flex-col items-start gap-4">
        <p className="text-sm text-muted-foreground">
          รายชื่อในนัดหมายแสดงชื่อ/HN จากทะเบียนผู้ป่วย — ต้องมีสิทธิ์ &quot;ดู&quot; ผู้ป่วยด้วยถึงจะเปิดหน้านี้ได้
          (ตั้งได้ที่หน้าการตั้งค่าสิทธิ์)
        </p>
        {backButton}
      </div>
    )
  }
  if (status === 'loading') {
    return <p className="p-4 text-sm text-muted-foreground">กำลังโหลด...</p>
  }
  if (status === 'notFound' || status === 'error' || !detail) {
    return (
      <div className="flex flex-col items-start gap-4">
        <p className={status === 'notFound' ? 'text-sm text-muted-foreground' : 'text-sm text-destructive'}>
          {status === 'notFound'
            ? 'ไม่พบนัดหมายนี้ (อาจถูกลบไปแล้ว)'
            : 'โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า'}
        </p>
        {backButton}
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        fallbackTitle="นัดหมายแล็ป"
        titleSuffix={`: กลุ่ม ${detail.patientGroupName}`}
        description={`ปีงบประมาณ ${detail.budgetYear}`}
        action={
          <div className="flex flex-wrap gap-2">
            {backButton}
            {/* ซ่อนชื่อ, HN, NAP NO บนหน้าจอ (ตาราง + dialog ในหน้านี้) — ค้นหายังใช้ค่าจริง เอกสารที่ส่งออกไม่เปลี่ยน */}
            <Button
              variant={maskIdentity ? 'default' : 'outline'}
              size="icon"
              onClick={toggleMaskIdentity}
              title={maskLabel}
            >
              {maskIdentity ? <EyeOff /> : <Eye />}
              <span className="sr-only">{maskLabel}</span>
            </Button>
            {canExport && (
              <ExportMenu
                disabled={detail.visits.length === 0}
                onExcel={(kind) => void handleExcel(kind)}
                onPdf={handlePdf}
              />
            )}
            {canUpdate && <EditButton size="default" onClick={() => void openHeadForm()}>แก้ไขนัดหมาย</EditButton>}
          </div>
        }
      />

      <Card>
        <CardContent className="grid grid-cols-2 gap-4 sm:grid-cols-4">
          {[
            ['วันนัดตรวจ CD4 ครั้งที่ 1', formatThaiDate(detail.cd4FirstDate)],
            ['วันนัดตรวจ CD4 ครั้งที่ 2', formatThaiDate(detail.cd4SecondDate)],
            ['วันนัดตรวจ VL', formatThaiDate(detail.vlDate)],
            ['จำนวนคนไข้', `${detail.patientCount} คน`],
          ].map(([label, value]) => (
            <div key={label}>
              <p className="text-xs text-muted-foreground">{label}</p>
              <p className="font-medium">{value}</p>
            </div>
          ))}
        </CardContent>
      </Card>

      <SearchForm
        id="lab-visit-search"
        ariaLabel="ค้นหาคนไข้ในนัดหมาย"
        fields={[
          { name: 'name', label: 'ชื่อ-สกุล', placeholder: 'ชื่อ หรือ สกุล' },
          { name: 'hn', label: 'HN' },
          { name: 'napNo', label: 'NAP NO' },
        ]}
        onSearch={(values) => {
          setCriteria(values)
          pagination.reset()
        }}
      />

      <ListToolbar
        ready
        total={joined.length}
        shown={visibleRows.length}
        filtered={hasCriteria(criteria)}
      >
        {canUpdate && <AddButton onClick={() => void openPicker()}>เพิ่มคนไข้</AddButton>}
      </ListToolbar>

      <Card>
        <CardContent className="flex flex-col gap-3 px-0 sm:px-2">
          {joined.length === 0 && (
            <p className="p-4 text-sm text-muted-foreground">
              ยังไม่มีคนไข้ในนัดหมายนี้{canUpdate ? ' กด "+ เพิ่มคนไข้" เพื่อเลือกจากทะเบียนผู้ป่วย' : ''}
            </p>
          )}
          {joined.length > 0 && visibleRows.length === 0 && (
            <p className="p-4 text-sm text-muted-foreground">
              ไม่พบคนไข้ที่ตรงกับเงื่อนไข — กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด
            </p>
          )}
          {visibleRows.length > 0 && (
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
                  <TableHead>อายุ</TableHead>
                  <TableHead>ระยะเวลากินยา</TableHead>
                  <TableHead>ผล VL</TableHead>
                  <TableHead>สูตรยา</TableHead>
                  <TableHead>โรคร่วม</TableHead>
                  {canUpdate && <TableHead className="text-right">จัดการ</TableHead>}
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((row) => (
                  <TableRow key={row.visit.id}>
                    <TableCell className="text-muted-foreground">{row.visit.sortOrder}</TableCell>
                    <TableCell className="font-mono text-sm">
                      {row.patient ? patientDisplayId(row.patient.hn, maskIdentity) : '—'}
                    </TableCell>
                    <TableCell className="font-medium">
                      {row.patient ? patientDisplayName(row.patient, maskIdentity) : 'ไม่พบข้อมูลคนไข้'}
                      {row.patient && (
                        <div className="text-xs font-normal text-muted-foreground">
                          NAP NO {patientDisplayId(row.patient.napNo, maskIdentity)}
                        </div>
                      )}
                    </TableCell>
                    <TableCell className="text-muted-foreground">
                      {row.patient?.age != null ? `${row.patient.age} ปี` : '—'}
                    </TableCell>
                    <TableCell className="text-muted-foreground">{takeMedText(row.visit)}</TableCell>
                    <TableCell>{row.visit.vlResult ?? '—'}</TableCell>
                    <TableCell className="text-muted-foreground">{row.visit.drugRecipe ?? '—'}</TableCell>
                    <TableCell>
                      {!row.patient || row.patient.conditions.length === 0 ? (
                        <span className="text-sm text-muted-foreground">—</span>
                      ) : (
                        <div className="flex flex-wrap gap-1">
                          {row.patient.conditions.map((condition) => (
                            <Badge key={condition.id} variant="outline" title={condition.name}>
                              {condition.abbreviation}
                            </Badge>
                          ))}
                        </div>
                      )}
                    </TableCell>
                    {canUpdate && (
                      <TableCell className="text-right whitespace-nowrap">
                        <EditButton onClick={() => setEditing(row)} />
                        <DeleteButton onClick={() => setPendingRemove(row)}>เอาออก</DeleteButton>
                      </TableCell>
                    )}
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          {visibleRows.length > 0 && <TablePagination {...pagination.props} />}
        </CardContent>
      </Card>

      <LabVisitGroupFormDialog
        open={headFormOpen}
        onOpenChange={setHeadFormOpen}
        mode="edit"
        initialValue={detail}
        options={options}
        onSubmit={handleHeadSubmit}
      />

      <PatientPickerDialog
        open={pickerOpen}
        onOpenChange={setPickerOpen}
        patients={patients}
        excludedIds={excludedIds}
        previousRound={previousRound}
        maskIdentity={maskIdentity}
        onSubmit={handleAddPatients}
      />

      <LabVisitFormDialog
        open={editing !== null}
        onOpenChange={(open) => {
          if (!open) setEditing(null)
        }}
        visit={editing?.visit ?? null}
        patient={editing?.patient}
        maskIdentity={maskIdentity}
        onSubmit={handleVisitSubmit}
      />

      <AlertDialog
        open={pendingRemove !== null}
        onOpenChange={(open) => {
          if (!open) setPendingRemove(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>เอาคนไข้ออกจากนัดหมาย</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการเอา HN{' '}
              {pendingRemove?.patient ? patientDisplayId(pendingRemove.patient.hn, maskIdentity) : '—'}{' '}
              ออกจากนัดหมายนี้ใช่ไหม? ข้อมูลในทะเบียนผู้ป่วยไม่ถูกลบ
              และเพิ่มกลับเข้านัดหมายได้ภายหลัง (ผล VL/สูตรยาที่กรอกไว้ในนัดหมายนี้จะหายไป)
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>ยกเลิก</AlertDialogCancel>
            <AlertDialogAction
              variant="danger"
              onClick={() => void handleRemove()}
              disabled={isRemoving}
            >
              {isRemoving ? 'กำลังเอาออก...' : 'เอาออก'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
