import { useEffect, useMemo } from 'react'
import { Controller, useForm, useWatch } from 'react-hook-form'
import type { Control } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import * as z from 'zod'

import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from '@/components/ui/combobox'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { NativeSelect } from '@/components/ui/native-select'
import { todayIso } from '@/lib/date'
import { textIncludes } from '@/lib/search'

import type { Patient, PatientInput, PatientOptions } from './patientsApi'

// ขีดจำกัดตรงกับ backend (backend/src/patient/service.rs) — backend ตรวจซ้ำอีกชั้นเสมอ
const MAX_ID = 50
const MAX_NAME = 100
const MAX_AGE = 150
const MAX_WEIGHT = 500
const MAX_HEIGHT = 300

// ค่าในฟอร์มเป็นสตริงทั้งหมด (ช่องตัวเลขที่ไม่บังคับต้องแยก "ว่าง" ออกจาก 0 ได้ และไม่ติด NaN) แล้วแปลงเป็น
// PatientInput ตอนส่ง (toInput)
const patientFormSchema = z
  .object({
    hn: z.string().trim().min(1, 'กรอก HN').max(MAX_ID, `HN ยาวได้ไม่เกิน ${MAX_ID} ตัวอักษร`),
    napNo: z
      .string()
      .trim()
      .min(1, 'กรอกเลขประจำตัวคนไข้')
      .max(MAX_ID, `เลขประจำตัวคนไข้ยาวได้ไม่เกิน ${MAX_ID} ตัวอักษร`),
    prenameId: z.string().min(1, 'เลือกคำนำหน้า'),
    firstName: z.string().trim().min(1, 'กรอกชื่อ').max(MAX_NAME, `ชื่อยาวได้ไม่เกิน ${MAX_NAME} ตัวอักษร`),
    lastName: z.string().trim().min(1, 'กรอกสกุล').max(MAX_NAME, `สกุลยาวได้ไม่เกิน ${MAX_NAME} ตัวอักษร`),
    sex: z.enum(['1', '2']),
    birthDate: z.string(),
    age: z.string(),
    weight: z.string(),
    height: z.string(),
    nationalityId: z.string(),
    isThai: z.boolean(),
    diseaseIds: z.array(z.string()),
    sortOrder: z.string(),
    isActive: z.boolean(),
  })
  .superRefine((v, ctx) => {
    const issue = (path: string, message: string) => ctx.addIssue({ code: 'custom', path: [path], message })

    if (v.birthDate && v.birthDate > todayIso()) issue('birthDate', 'วันเกิดต้องไม่เป็นอนาคต')
    if (!v.birthDate && v.age !== '') {
      const age = Number(v.age)
      if (!Number.isInteger(age) || age < 0 || age > MAX_AGE) {
        issue('age', `อายุต้องเป็นจำนวนเต็ม 0–${MAX_AGE}`)
      }
    }
    const measure = (path: 'weight' | 'height', max: number, label: string) => {
      if (v[path] === '') return
      const value = Number(v[path])
      if (!Number.isFinite(value) || value <= 0 || value > max) {
        issue(path, `${label}ต้องมากกว่า 0 และไม่เกิน ${max}`)
      }
    }
    measure('weight', MAX_WEIGHT, 'น้ำหนัก')
    measure('height', MAX_HEIGHT, 'ส่วนสูง')
    if (v.sortOrder !== '' && !Number.isInteger(Number(v.sortOrder))) {
      issue('sortOrder', 'ลำดับต้องเป็นจำนวนเต็ม')
    }
  })

export type PatientFormValues = z.infer<typeof patientFormSchema>

// อายุเต็มปีจากวันเกิด (YYYY-MM-DD) ณ วันนี้ — แสดงให้ดูเฉย ๆ ตอนกรอก backend คำนวณซ้ำเองเสมอ
function ageFromBirthDate(birthDate: string): number | null {
  const [y, m, d] = birthDate.split('-').map(Number)
  if (!y || !m || !d) return null
  const [ty, tm, td] = todayIso().split('-').map(Number)
  let age = ty - y
  if (tm < m || (tm === m && td < d)) age -= 1
  return age >= 0 ? age : null
}

const EMPTY_VALUES: PatientFormValues = {
  hn: '',
  napNo: '',
  prenameId: '',
  firstName: '',
  lastName: '',
  sex: '1',
  birthDate: '',
  age: '',
  weight: '',
  height: '',
  nationalityId: '',
  isThai: true,
  diseaseIds: [],
  sortOrder: '',
  isActive: true,
}

function valuesFromPatient(p: Patient): PatientFormValues {
  return {
    hn: p.hn,
    napNo: p.napNo,
    prenameId: p.prenameId,
    firstName: p.firstName,
    lastName: p.lastName,
    sex: String(p.sex) as '1' | '2',
    birthDate: p.birthDate ?? '',
    age: p.age === null ? '' : String(p.age),
    weight: p.weight === null ? '' : String(p.weight),
    height: p.height === null ? '' : String(p.height),
    nationalityId: p.nationalityId ?? '',
    isThai: p.isThai,
    diseaseIds: p.conditions.map((c) => c.diseaseId),
    sortOrder: String(p.sortOrder),
    isActive: p.isActive,
  }
}

function toInput(v: PatientFormValues, diseaseOrder: string[]): PatientInput {
  const optionalNumber = (value: string) => (value === '' ? null : Number(value))
  return {
    hn: v.hn.trim(),
    napNo: v.napNo.trim(),
    prenameId: v.prenameId,
    firstName: v.firstName.trim(),
    lastName: v.lastName.trim(),
    birthDate: v.birthDate || null,
    // มีวันเกิด → อายุคำนวณโดย backend (ไม่ส่ง); ไม่มี → ใช้ที่กรอก
    age: v.birthDate ? null : optionalNumber(v.age),
    weight: optionalNumber(v.weight),
    height: optionalNumber(v.height),
    sex: Number(v.sex),
    isThai: v.isThai,
    // คนไทย = ไม่ต้องระบุสัญชาติ (ช่องเลือกสัญชาติซ่อนอยู่) — ส่ง null; ไม่ใช่คนไทยจึงส่งสัญชาติที่เลือก (ว่างได้)
    nationalityId: v.isThai ? null : v.nationalityId || null,
    sortOrder: optionalNumber(v.sortOrder),
    isActive: v.isActive,
    // เรียงตามลำดับรายการในฟอร์ม (คงที่ ไม่ขึ้นกับลำดับที่ติ๊ก)
    diseaseIds: diseaseOrder.filter((id) => v.diseaseIds.includes(id)),
  }
}

function SectionTitle({ children }: { children: string }) {
  return <h3 className="border-b pb-1 text-sm font-medium text-muted-foreground">{children}</h3>
}

interface TextFieldProps {
  control: Control<PatientFormValues>
  name: 'hn' | 'napNo' | 'firstName' | 'lastName' | 'weight' | 'height' | 'sortOrder' | 'age' | 'birthDate'
  label: string
  type?: string
  inputMode?: 'text' | 'numeric' | 'decimal'
  maxLength?: number
  max?: string
  disabled?: boolean
  description?: string
  placeholder?: string
}

// ช่องกรอกข้อความ/ตัวเลข/วันที่หนึ่งช่องพร้อม label และข้อความ error
function TextField({ control, name, label, description, ...inputProps }: TextFieldProps) {
  const id = `patient-${name}`
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

interface PatientFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: 'create' | 'edit'
  initialValue?: Patient
  // ตัวเลือกของฟอร์ม (null = ยังโหลดไม่เสร็จ)
  options: PatientOptions | null
  onSubmit: (input: PatientInput) => Promise<void>
}

// ฟอร์มลงทะเบียน/แก้ไขผู้ป่วย — รวมการเลือกโรคประจำตัว (เลือกได้หลายโรคจากโรคที่ใช้งานอยู่) ข้อมูลในฟอร์ม
// อยู่แค่ใน state ของ React ระหว่างกรอก ไม่เก็บลง storage/URL
export function PatientFormDialog({
  open,
  onOpenChange,
  mode,
  initialValue,
  options,
  onSubmit,
}: PatientFormDialogProps) {
  const form = useForm<PatientFormValues>({
    resolver: zodResolver(patientFormSchema),
    defaultValues: EMPTY_VALUES,
  })

  useEffect(() => {
    if (!open) return
    form.reset(mode === 'edit' && initialValue ? valuesFromPatient(initialValue) : EMPTY_VALUES)
  }, [open, mode, initialValue, form])

  const birthDate = useWatch({ control: form.control, name: 'birthDate' })
  const isThai = useWatch({ control: form.control, name: 'isThai' })
  const computedAge = birthDate ? ageFromBirthDate(birthDate) : null

  // โรคที่เลือกได้ = โรคที่ใช้งานอยู่ + โรคที่ผู้ป่วยผูกอยู่แล้วแต่ถูกปิดใช้งานภายหลัง (ต้องให้เห็นและถอดได้
  // ไม่งั้นบันทึกแล้วโรคนั้นหายไปเงียบ ๆ) — ผูกโรคที่ปิดอยู่เพิ่มใหม่ไม่ได้ (backend ปฏิเสธ)
  const diseaseChoices = useMemo(() => {
    const active = (options?.diseases ?? []).map((d) => ({ ...d, inactive: false }))
    const extra = (initialValue?.conditions ?? [])
      .filter((c) => !active.some((d) => d.id === c.diseaseId))
      .map((c) => ({ id: c.diseaseId, name: c.name, abbreviation: c.abbreviation, inactive: true }))
    return [...active, ...extra]
  }, [options, initialValue])
  const diseaseOrder = useMemo(() => diseaseChoices.map((d) => d.id), [diseaseChoices])

  // สัญชาติที่ผู้ป่วยใช้อยู่แต่ถูกปิดใช้งานภายหลัง ยังต้องแสดงเพื่อไม่ให้ค่าหาย
  const nationalityChoices = useMemo(() => {
    const active = options?.nationalities ?? []
    if (
      initialValue?.nationalityId &&
      !active.some((n) => n.id === initialValue.nationalityId)
    ) {
      return [
        ...active,
        {
          id: initialValue.nationalityId,
          nationalName: `${initialValue.nationalityName ?? 'ไม่ทราบ'} (ปิดใช้งาน)`,
          nation: '',
        },
      ]
    }
    return active
  }, [options, initialValue])

  const handleSubmit = form.handleSubmit(async (values) => {
    try {
      await onSubmit(toInput(values, diseaseOrder))
      onOpenChange(false)
    } catch (err) {
      form.setError('root', {
        message: err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง',
      })
    }
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{mode === 'create' ? 'ลงทะเบียนผู้ป่วย' : 'แก้ไขข้อมูลผู้ป่วย'}</DialogTitle>
        </DialogHeader>
        {options === null ? (
          <p className="py-6 text-sm text-muted-foreground">กำลังโหลดตัวเลือก...</p>
        ) : (
          <form onSubmit={handleSubmit}>
            <FieldGroup>
              <SectionTitle>ทะเบียน</SectionTitle>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <TextField control={form.control} name="hn" label="HN" maxLength={MAX_ID} description="ซ้ำกันไม่ได้" />
                <TextField control={form.control} name="napNo" label="เลขประจำตัวคนไข้" maxLength={MAX_ID} />
              </div>

              <SectionTitle>ชื่อ</SectionTitle>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                <Controller
                  name="prenameId"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="patient-prenameId">คำนำหน้า</FieldLabel>
                      <NativeSelect {...field} id="patient-prenameId" aria-invalid={fieldState.invalid}>
                        <option value="">— เลือก —</option>
                        {options.prenames.map((p) => (
                          <option key={p.id} value={p.id}>
                            {p.name}
                          </option>
                        ))}
                      </NativeSelect>
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
                <TextField control={form.control} name="firstName" label="ชื่อ" maxLength={MAX_NAME} />
                <TextField control={form.control} name="lastName" label="สกุล" maxLength={MAX_NAME} />
              </div>

              <SectionTitle>ข้อมูลทั่วไป</SectionTitle>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                <Controller
                  name="sex"
                  control={form.control}
                  render={({ field }) => (
                    <Field>
                      <FieldLabel htmlFor="patient-sex">เพศ</FieldLabel>
                      <NativeSelect {...field} id="patient-sex">
                        <option value="1">ชาย</option>
                        <option value="2">หญิง</option>
                      </NativeSelect>
                    </Field>
                  )}
                />
                <TextField control={form.control} name="birthDate" label="วันเกิด" type="date" max={todayIso()} />
                {birthDate ? (
                  <Field>
                    <FieldLabel htmlFor="patient-age-computed">อายุ (ปี)</FieldLabel>
                    <Input id="patient-age-computed" value={computedAge ?? ''} disabled readOnly />
                    <FieldDescription>คำนวณจากวันเกิด</FieldDescription>
                  </Field>
                ) : (
                  <TextField control={form.control} name="age" label="อายุ (ปี)" type="number" inputMode="numeric" />
                )}
                <TextField control={form.control} name="weight" label="น้ำหนัก (kg)" type="number" inputMode="decimal" />
                <TextField control={form.control} name="height" label="ส่วนสูง (cm)" type="number" inputMode="decimal" />
              </div>
              <Controller
                name="isThai"
                control={form.control}
                render={({ field }) => (
                  <label className="flex items-center gap-2 text-sm font-normal">
                    <Checkbox
                      checked={field.value}
                      onCheckedChange={(checked) => field.onChange(checked === true)}
                    />
                    คนไทย
                  </label>
                )}
              />
              {!isThai && (
                <Controller
                  name="nationalityId"
                  control={form.control}
                  render={({ field }) => (
                    <Field className="sm:max-w-md">
                      <FieldLabel htmlFor="patient-nationalityId">สัญชาติ</FieldLabel>
                      {/* รายการสัญชาติยาวเกือบสองร้อย — เลือกด้วย combobox ที่พิมพ์ค้นหา (ชื่อสัญชาติหรือชื่อประเทศ)
                          ได้ และกด × เพื่อล้างค่าที่เลือก (ไม่ระบุ) */}
                      <Combobox
                        items={nationalityChoices}
                        value={nationalityChoices.find((n) => n.id === field.value) ?? null}
                        onValueChange={(item) => field.onChange(item?.id ?? '')}
                        itemToStringLabel={(n) => n.nationalName}
                        isItemEqualToValue={(a, b) => a.id === b.id}
                        filter={(n, query) => textIncludes(n.nationalName, query) || textIncludes(n.nation, query)}
                      >
                        <ComboboxInput
                          id="patient-nationalityId"
                          placeholder="พิมพ์เพื่อค้นหาสัญชาติ"
                          showClear
                        />
                        <ComboboxContent>
                          <ComboboxEmpty>ไม่พบสัญชาติที่ค้นหา</ComboboxEmpty>
                          <ComboboxList>
                            {(n) => (
                              <ComboboxItem key={n.id} value={n}>
                                {n.nationalName}
                                {n.nation && n.nation !== n.nationalName && (
                                  <span className="text-muted-foreground"> · {n.nation}</span>
                                )}
                              </ComboboxItem>
                            )}
                          </ComboboxList>
                        </ComboboxContent>
                      </Combobox>
                      <FieldDescription>ไม่ระบุได้ — กด × ในช่องเพื่อล้างค่าที่เลือก</FieldDescription>
                    </Field>
                  )}
                />
              )}

              <SectionTitle>โรคประจำตัว</SectionTitle>
              <Controller
                name="diseaseIds"
                control={form.control}
                render={({ field }) => (
                  <Field>
                    {diseaseChoices.length === 0 ? (
                      <FieldDescription>
                        ยังไม่มีโรคให้เลือก — เพิ่มได้ที่ การตั้งค่า &gt; ตั้งค่าโรค
                      </FieldDescription>
                    ) : (
                      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                        {diseaseChoices.map((disease) => (
                          <label key={disease.id} className="flex items-center gap-2 text-sm font-normal">
                            <Checkbox
                              checked={field.value.includes(disease.id)}
                              onCheckedChange={(checked) =>
                                field.onChange(
                                  checked === true
                                    ? [...field.value, disease.id]
                                    : field.value.filter((id) => id !== disease.id),
                                )
                              }
                            />
                            <span>
                              {disease.name}
                              <span className="text-muted-foreground"> ({disease.abbreviation})</span>
                              {disease.inactive && (
                                <span className="text-muted-foreground"> · ปิดใช้งาน</span>
                              )}
                            </span>
                          </label>
                        ))}
                      </div>
                    )}
                  </Field>
                )}
              />

              <SectionTitle>อื่น ๆ</SectionTitle>
              <div className="grid grid-cols-1 items-end gap-4 sm:grid-cols-2">
                <TextField
                  control={form.control}
                  name="sortOrder"
                  label="ลำดับการแสดงผล"
                  type="number"
                  inputMode="numeric"
                  description={mode === 'create' ? 'เว้นว่าง = ต่อท้ายสุด' : 'เว้นว่าง = คงลำดับเดิม'}
                />
                <Controller
                  name="isActive"
                  control={form.control}
                  render={({ field }) => (
                    <label className="flex items-center gap-2 pb-2 text-sm font-normal">
                      <Checkbox
                        checked={field.value}
                        onCheckedChange={(checked) => field.onChange(checked === true)}
                      />
                      ใช้งานอยู่
                    </label>
                  )}
                />
              </div>

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
        )}
      </DialogContent>
    </Dialog>
  )
}
