import { useEffect } from 'react'
import { Controller, useForm } from 'react-hook-form'
import type { Control } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import * as z from 'zod'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import type { Patient } from '@/features/patients/patientsApi'

import { patientDisplayId, patientDisplayName } from '../documents'
import type { LabVisit, LabVisitInput } from '../labVisitsApi'

// ขีดจำกัดตรงกับ backend (backend/src/lab_visit/service.rs) — backend ตรวจซ้ำอีกชั้นเสมอ
const MAX_YEARS = 100
const MAX_MONTHS = 11
const MAX_VL_RESULT = 100
const MAX_DRUG_RECIPE = 200

const optionalInt = (max: number, message: string) =>
  z.string().refine((value) => {
    if (value === '') return true
    const n = Number(value)
    return Number.isInteger(n) && n >= 0 && n <= max
  }, message)

const visitFormSchema = z.object({
  takeMedYearRange: optionalInt(MAX_YEARS, `ปีต้องเป็นจำนวนเต็ม 0–${MAX_YEARS}`),
  takeMedMonthRange: optionalInt(MAX_MONTHS, `เดือนต้องเป็นจำนวนเต็ม 0–${MAX_MONTHS}`),
  vlResult: z.string().trim().max(MAX_VL_RESULT, `ยาวได้ไม่เกิน ${MAX_VL_RESULT} ตัวอักษร`),
  drugRecipe: z.string().trim().max(MAX_DRUG_RECIPE, `ยาวได้ไม่เกิน ${MAX_DRUG_RECIPE} ตัวอักษร`),
  sortOrder: z.string().refine((value) => value === '' || Number.isInteger(Number(value)), 'ลำดับต้องเป็นจำนวนเต็ม'),
})

type VisitFormValues = z.infer<typeof visitFormSchema>

function valuesFromVisit(visit: LabVisit): VisitFormValues {
  const text = (value: number | null) => (value === null ? '' : String(value))
  return {
    takeMedYearRange: text(visit.takeMedYearRange),
    takeMedMonthRange: text(visit.takeMedMonthRange),
    vlResult: visit.vlResult ?? '',
    drugRecipe: visit.drugRecipe ?? '',
    sortOrder: String(visit.sortOrder),
  }
}

function toInput(v: VisitFormValues): LabVisitInput {
  const optionalNumber = (value: string) => (value === '' ? null : Number(value))
  return {
    takeMedYearRange: optionalNumber(v.takeMedYearRange),
    takeMedMonthRange: optionalNumber(v.takeMedMonthRange),
    vlResult: v.vlResult.trim() || null,
    drugRecipe: v.drugRecipe.trim() || null,
    sortOrder: optionalNumber(v.sortOrder),
  }
}

interface TextFieldProps {
  control: Control<VisitFormValues>
  name: keyof VisitFormValues
  label: string
  type?: string
  inputMode?: 'text' | 'numeric'
  placeholder?: string
  maxLength?: number
  description?: string
}

function TextField({ control, name, label, description, ...inputProps }: TextFieldProps) {
  const id = `lab-visit-${name}`
  return (
    <Controller
      name={name}
      control={control}
      render={({ field, fieldState }) => (
        <Field data-invalid={fieldState.invalid}>
          <FieldLabel htmlFor={id}>{label}</FieldLabel>
          <Input {...field} id={id} aria-invalid={fieldState.invalid} {...inputProps} />
          {description && <FieldDescription>{description}</FieldDescription>}
          {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
        </Field>
      )}
    />
  )
}

const SEX_LABEL: Record<number, string> = { 1: 'ชาย', 2: 'หญิง' }

// ข้อมูลคนไข้ที่เกี่ยวข้อง (จากทะเบียนผู้ป่วย) แสดงแบบอ่านอย่างเดียว — แก้ได้ที่หน้าลงทะเบียนผู้ป่วย
function PatientSummary({ patient, maskIdentity }: { patient: Patient | undefined; maskIdentity: boolean }) {
  if (!patient) {
    return <p className="text-sm text-destructive">ไม่พบข้อมูลคนไข้ในทะเบียน (อาจถูกลบไปแล้ว)</p>
  }
  const measures = [
    patient.weight !== null ? `น้ำหนัก ${patient.weight} kg` : null,
    patient.height !== null ? `ส่วนสูง ${patient.height} cm` : null,
  ].filter(Boolean)
  return (
    <div className="flex flex-col gap-1 rounded-lg border bg-muted/30 p-3 text-sm">
      <p className="font-medium">
        <span className="font-mono text-muted-foreground">HN {patientDisplayId(patient.hn, maskIdentity)}</span> ·{' '}
        {patientDisplayName(patient, maskIdentity)}
      </p>
      <p className="text-muted-foreground">
        NAP NO {patientDisplayId(patient.napNo, maskIdentity)} · {SEX_LABEL[patient.sex]}
        {patient.age !== null && ` · ${patient.age} ปี`}
        {measures.length > 0 && ` · ${measures.join(' · ')}`}
      </p>
      <div className="flex flex-wrap items-center gap-1 text-muted-foreground">
        โรคประจำตัว:
        {patient.conditions.length === 0
          ? ' —'
          : patient.conditions.map((condition) => (
              <Badge key={condition.id} variant="outline" title={condition.name}>
                {condition.abbreviation}
              </Badge>
            ))}
      </div>
    </div>
  )
}

interface LabVisitFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  visit: LabVisit | null
  patient: Patient | undefined
  // true = โหมดซ่อนข้อมูลระบุตัวของหน้ารายชื่อนัดหมาย (ปุ่มรูปตา) — ซ่อนชื่อ, HN, NAP NO ใน dialog นี้ด้วย
  maskIdentity: boolean
  onSubmit: (input: LabVisitInput) => Promise<void>
}

// แก้ข้อมูลของคนไข้หนึ่งคนในนัดหมาย: ระยะเวลากินยา (ปี/เดือน), ผล VL, สูตรยา, ลำดับ
export function LabVisitFormDialog({
  open,
  onOpenChange,
  visit,
  patient,
  maskIdentity,
  onSubmit,
}: LabVisitFormDialogProps) {
  const form = useForm<VisitFormValues>({
    resolver: zodResolver(visitFormSchema),
    defaultValues: {
      takeMedYearRange: '',
      takeMedMonthRange: '',
      vlResult: '',
      drugRecipe: '',
      sortOrder: '',
    },
  })

  useEffect(() => {
    if (open && visit) form.reset(valuesFromVisit(visit))
  }, [open, visit, form])

  const handleSubmit = form.handleSubmit(async (values) => {
    try {
      await onSubmit(toInput(values))
      onOpenChange(false)
    } catch (err) {
      form.setError('root', {
        message: err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง',
      })
    }
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>แก้ไขข้อมูลคนไข้ในนัดหมาย</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <FieldGroup>
            <PatientSummary patient={patient} maskIdentity={maskIdentity} />

            <div className="grid grid-cols-2 gap-4">
              <TextField
                control={form.control}
                name="takeMedYearRange"
                label="ระยะเวลากินยา (ปี)"
                type="number"
                inputMode="numeric"
              />
              <TextField
                control={form.control}
                name="takeMedMonthRange"
                label="ระยะเวลากินยา (เดือน)"
                type="number"
                inputMode="numeric"
                description={`0–${MAX_MONTHS} (ส่วนที่เกินปี)`}
              />
            </div>
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <TextField
                control={form.control}
                name="vlResult"
                label="ผล VL"
                placeholder="เช่น <20"
                maxLength={MAX_VL_RESULT}
              />
              <TextField
                control={form.control}
                name="sortOrder"
                label="ลำดับ"
                type="number"
                inputMode="numeric"
                description="เว้นว่าง = คงลำดับเดิม"
              />
            </div>
            <TextField
              control={form.control}
              name="drugRecipe"
              label="สูตรยา"
              placeholder="เช่น TDF+3TC+DTG"
              maxLength={MAX_DRUG_RECIPE}
            />

            {form.formState.errors.root && (
              <Field data-invalid>
                <FieldError>{form.formState.errors.root.message}</FieldError>
              </Field>
            )}
          </FieldGroup>
          <DialogFooter className="mt-4">
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              ยกเลิก
            </Button>
            <Button type="submit" disabled={form.formState.isSubmitting}>
              {form.formState.isSubmitting ? 'กำลังบันทึก...' : 'บันทึก'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
