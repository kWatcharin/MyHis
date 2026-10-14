import { History, Search } from 'lucide-react'
import { useMemo, useState } from 'react'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import type { Patient } from '@/features/patients/patientsApi'
import { allTokensIncluded, textIncludes } from '@/lib/search'

import { patientDisplayId, patientDisplayName } from './documents'

// รายชื่อจากนัดหมายครั้งก่อนของกลุ่มเดียวกัน (ปีงบก่อนหน้า) — ใช้เลือกคนชุดเดิมได้ในคลิกเดียว
export interface PreviousRound {
  label: string
  patientIds: string[]
}

interface PatientPickerDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  // คนไข้ทั้งหมดจากทะเบียน (GET /patients)
  patients: Patient[]
  // คนไข้ที่อยู่ในนัดหมายนี้แล้ว (ไม่ให้เลือกซ้ำ)
  excludedIds: ReadonlySet<string>
  previousRound: PreviousRound | null
  // true = โหมดซ่อนข้อมูลระบุตัวของหน้ารายชื่อนัดหมาย (ปุ่มรูปตา) — ชื่อ, HN, NAP NO ในรายการนี้ซ่อนด้วย
  // แต่ยังค้นด้วยค่าจริงได้
  maskIdentity: boolean
  onSubmit: (patientIds: string[]) => Promise<void>
}

function matchesQuery(patient: Patient, query: string): boolean {
  return (
    allTokensIncluded(`${patient.prenameName} ${patient.firstName} ${patient.lastName}`, query) ||
    textIncludes(patient.hn, query) ||
    textIncludes(patient.napNo, query)
  )
}

// เลือกคนไข้จากทะเบียนเข้านัดหมาย (เลือกได้หลายคน) — แสดงข้อมูลที่เกี่ยวข้อง (HN, อายุ, โรคประจำตัว) ช่วยเลือก
// ค้นหาฝั่ง client (ชื่อ/HN เข้ารหัสในฐานข้อมูล) เลือกได้เฉพาะคนที่ใช้งานอยู่และยังไม่อยู่ในนัดหมายนี้
export function PatientPickerDialog({
  open,
  onOpenChange,
  patients,
  excludedIds,
  previousRound,
  maskIdentity,
  onSubmit,
}: PatientPickerDialogProps) {
  const [query, setQuery] = useState('')
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set())
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // เปิดใหม่ทุกครั้งเริ่มจากว่าง — ปรับ state ระหว่าง render ตอน prop open เปลี่ยน (ไม่ใช้ effect)
  const [wasOpen, setWasOpen] = useState(open)
  if (open !== wasOpen) {
    setWasOpen(open)
    if (open) {
      setQuery('')
      setSelected(new Set())
      setError(null)
    }
  }

  const candidates = useMemo(
    () => patients.filter((patient) => patient.isActive && !excludedIds.has(patient.id)),
    [patients, excludedIds],
  )
  const visible = candidates.filter((patient) => matchesQuery(patient, query))
  const previousCandidates = useMemo(() => {
    if (!previousRound) return []
    const ids = new Set(candidates.map((patient) => patient.id))
    return previousRound.patientIds.filter((id) => ids.has(id))
  }, [previousRound, candidates])

  const toggle = (id: string, checked: boolean) =>
    setSelected((current) => {
      const next = new Set(current)
      if (checked) next.add(id)
      else next.delete(id)
      return next
    })

  const allVisibleSelected = visible.length > 0 && visible.every((patient) => selected.has(patient.id))

  const handleSubmit = async () => {
    // ลำดับที่ส่ง = ลำดับในนัดหมายครั้งก่อน (ถ้ามี) แล้วตามลำดับทะเบียน — backend ต่อท้ายตามลำดับนี้
    const previousRank = new Map(previousRound?.patientIds.map((id, index) => [id, index]))
    const ordered = candidates
      .map((patient, index) => ({ id: patient.id, index }))
      .filter(({ id }) => selected.has(id))
      .sort(
        (a, b) =>
          (previousRank.get(a.id) ?? Number.MAX_SAFE_INTEGER) -
            (previousRank.get(b.id) ?? Number.MAX_SAFE_INTEGER) || a.index - b.index,
      )
      .map(({ id }) => id)
    setIsSubmitting(true)
    setError(null)
    try {
      await onSubmit(ordered)
      onOpenChange(false)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง')
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex max-h-[90vh] flex-col overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>เพิ่มคนไข้เข้านัดหมาย</DialogTitle>
          <DialogDescription>
            เลือกได้หลายคน — แสดงเฉพาะคนไข้ที่ใช้งานอยู่และยังไม่อยู่ในนัดหมายนี้ ({candidates.length} คน)
          </DialogDescription>
        </DialogHeader>

        <div className="relative">
          <Search className="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="ค้นหาชื่อ-สกุล, HN หรือ NAP NO"
            aria-label="ค้นหาคนไข้"
            className="pl-8"
          />
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={visible.length === 0}
            onClick={() =>
              setSelected((current) => {
                const next = new Set(current)
                for (const patient of visible) {
                  if (allVisibleSelected) next.delete(patient.id)
                  else next.add(patient.id)
                }
                return next
              })
            }
          >
            {allVisibleSelected ? 'ไม่เลือกที่แสดงอยู่' : 'เลือกทั้งหมดที่แสดง'}
          </Button>
          {previousRound && previousCandidates.length > 0 && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => setSelected((current) => new Set([...current, ...previousCandidates]))}
            >
              <History />
              เลือกชุดเดียวกับ{previousRound.label} ({previousCandidates.length} คน)
            </Button>
          )}
          {selected.size > 0 && (
            <Button type="button" variant="ghost" size="sm" onClick={() => setSelected(new Set())}>
              ล้างที่เลือก
            </Button>
          )}
        </div>

        {/* รายชื่อเลื่อนในกรอบตัวเอง (สูงไม่เกินครึ่งจอ) ปุ่ม "เพิ่ม N คน" ด้านล่างจึงเห็นเสมอแม้คนไข้หลายร้อยคน */}
        <div className="max-h-[50vh] min-h-0 flex-1 overflow-y-auto rounded-lg border">
          {candidates.length === 0 ? (
            <p className="p-4 text-sm text-muted-foreground">
              ไม่มีคนไข้ให้เลือก — ทุกคนอยู่ในนัดหมายนี้แล้ว หรือยังไม่ได้ลงทะเบียนผู้ป่วย
            </p>
          ) : visible.length === 0 ? (
            <p className="p-4 text-sm text-muted-foreground">ไม่พบคนไข้ที่ค้นหา</p>
          ) : (
            <ul className="divide-y">
              {visible.map((patient) => (
                <li key={patient.id}>
                  <label className="flex cursor-pointer items-start gap-3 px-3 py-2 hover:bg-muted/50">
                    <Checkbox
                      className="mt-0.5"
                      checked={selected.has(patient.id)}
                      onCheckedChange={(checked) => toggle(patient.id, checked === true)}
                    />
                    <span className="flex min-w-0 flex-1 flex-col gap-1">
                      <span className="text-sm font-medium">
                        <span className="font-mono text-muted-foreground">
                          {patientDisplayId(patient.hn, maskIdentity)}
                        </span>{' '}
                        {patientDisplayName(patient, maskIdentity)}
                      </span>
                      <span className="flex flex-wrap items-center gap-1 text-xs text-muted-foreground">
                        NAP NO {patientDisplayId(patient.napNo, maskIdentity)}
                        {patient.age !== null && ` · ${patient.age} ปี`}
                        {patient.conditions.map((condition) => (
                          <Badge key={condition.id} variant="outline" title={condition.name}>
                            {condition.abbreviation}
                          </Badge>
                        ))}
                      </span>
                    </span>
                  </label>
                </li>
              ))}
            </ul>
          )}
        </div>

        {error && <p className="text-sm text-destructive">{error}</p>}

        <DialogFooter>
          <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
            ยกเลิก
          </Button>
          <Button
            type="button"
            variant="success"
            disabled={selected.size === 0 || isSubmitting}
            onClick={() => void handleSubmit()}
          >
            {isSubmitting ? 'กำลังเพิ่ม...' : `เพิ่ม ${selected.size} คน`}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
