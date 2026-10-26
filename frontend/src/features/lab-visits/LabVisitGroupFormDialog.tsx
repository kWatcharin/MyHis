import { useEffect, useMemo, useRef } from 'react'
import { Controller, useForm, useWatch } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import * as z from 'zod'

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
import { ThaiDateInput } from '@/components/ui/thai-date-input'
import { NativeSelect } from '@/components/ui/native-select'
import { addMonthsClamped, formatThaiDate } from '@/lib/date'

import type { LabVisitGroup, LabVisitGroupInput, LabVisitOptions } from './labVisitsApi'

// ค่าเริ่มต้นของวันนัด CD4 ครั้งที่ 2 = ครั้งที่ 1 + 6 เดือน (แก้ได้ — backend บังคับแค่ว่าต้องหลังครั้งที่ 1)
const CD4_INTERVAL_MONTHS = 6

const groupFormSchema = z
  .object({
    budgetYearId: z.string().min(1, 'เลือกปีงบประมาณ'),
    patientGroupId: z.string().min(1, 'เลือกกลุ่มคนไข้'),
    cd4FirstDate: z.string().min(1, 'กรอกวันนัด CD4 ครั้งที่ 1'),
    cd4SecondDate: z.string().min(1, 'กรอกวันนัด CD4 ครั้งที่ 2'),
    vlDate: z.string().min(1, 'กรอกวันนัดตรวจ VL'),
    sortOrder: z.string(),
  })
  .superRefine((v, ctx) => {
    // วันที่รูป YYYY-MM-DD เทียบแบบสตริงได้ตรงตามลำดับเวลา
    if (v.cd4FirstDate && v.cd4SecondDate && v.cd4SecondDate <= v.cd4FirstDate) {
      ctx.addIssue({
        code: 'custom',
        path: ['cd4SecondDate'],
        message: 'วันนัด CD4 ครั้งที่ 2 ต้องหลังครั้งที่ 1',
      })
    }
    if (v.sortOrder !== '' && !Number.isInteger(Number(v.sortOrder))) {
      ctx.addIssue({ code: 'custom', path: ['sortOrder'], message: 'ลำดับต้องเป็นจำนวนเต็ม' })
    }
  })

type GroupFormValues = z.infer<typeof groupFormSchema>

const EMPTY_VALUES: GroupFormValues = {
  budgetYearId: '',
  patientGroupId: '',
  cd4FirstDate: '',
  cd4SecondDate: '',
  vlDate: '',
  sortOrder: '',
}

function valuesFromGroup(group: LabVisitGroup): GroupFormValues {
  return {
    budgetYearId: group.budgetYearId,
    patientGroupId: group.patientGroupId,
    cd4FirstDate: group.cd4FirstDate,
    cd4SecondDate: group.cd4SecondDate,
    vlDate: group.vlDate,
    sortOrder: String(group.sortOrder),
  }
}

function toInput(v: GroupFormValues): LabVisitGroupInput {
  return {
    budgetYearId: v.budgetYearId,
    patientGroupId: v.patientGroupId,
    cd4FirstDate: v.cd4FirstDate,
    cd4SecondDate: v.cd4SecondDate,
    vlDate: v.vlDate,
    sortOrder: v.sortOrder === '' ? null : Number(v.sortOrder),
  }
}

interface LabVisitGroupFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: 'create' | 'edit'
  initialValue?: LabVisitGroup
  // ตัวเลือกปีงบประมาณ/กลุ่มคนไข้ (null = ยังโหลดไม่เสร็จ)
  options: LabVisitOptions | null
  onSubmit: (input: LabVisitGroupInput) => Promise<void>
}

// ฟอร์มสร้าง/แก้ไขหัวนัดหมาย: ปีงบประมาณ + กลุ่มคนไข้ + วันนัด CD4 สองครั้ง + วันนัด VL
export function LabVisitGroupFormDialog({
  open,
  onOpenChange,
  mode,
  initialValue,
  options,
  onSubmit,
}: LabVisitGroupFormDialogProps) {
  const form = useForm<GroupFormValues>({
    resolver: zodResolver(groupFormSchema),
    defaultValues: EMPTY_VALUES,
  })
  // ผู้ใช้แก้ช่อง CD4 ครั้งที่ 2 เองแล้ว (หรือเป็นค่าที่บันทึกไว้เดิมตอนแก้ไข) → เปลี่ยนครั้งที่ 1 แล้วไม่ทับให้
  const secondEditedRef = useRef(false)

  useEffect(() => {
    if (!open) return
    form.reset(mode === 'edit' && initialValue ? valuesFromGroup(initialValue) : EMPTY_VALUES)
    secondEditedRef.current = mode === 'edit'
  }, [open, mode, initialValue, form])

  const [budgetYearId, cd4FirstDate, cd4SecondDate] = useWatch({
    control: form.control,
    name: ['budgetYearId', 'cd4FirstDate', 'cd4SecondDate'],
  })
  const suggestedSecond = cd4FirstDate ? addMonthsClamped(cd4FirstDate, CD4_INTERVAL_MONTHS) : null

  // ปีงบ/กลุ่มที่เลือกได้ = ที่ใช้งานอยู่ + ค่าปัจจุบันของนัดหมายที่แก้อยู่ (แม้ถูกปิดใช้งานภายหลัง ไม่งั้นค่าหายเงียบ ๆ)
  const budgetYears = useMemo(
    () =>
      (options?.budgetYears ?? []).filter(
        (year) => year.isActive || year.id === initialValue?.budgetYearId,
      ),
    [options, initialValue],
  )
  const patientGroups = useMemo(
    () =>
      (options?.patientGroups ?? []).filter(
        (group) => group.isActive || group.id === initialValue?.patientGroupId,
      ),
    [options, initialValue],
  )
  const selectedYear = budgetYears.find((year) => year.id === budgetYearId)

  const setDate = (name: 'cd4SecondDate' | 'vlDate', value: string) =>
    form.setValue(name, value, { shouldDirty: true, shouldValidate: form.formState.isSubmitted })

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
          <DialogTitle>{mode === 'create' ? 'สร้างนัดหมายแล็ป' : 'แก้ไขนัดหมายแล็ป'}</DialogTitle>
        </DialogHeader>
        {options === null ? (
          <p className="py-6 text-sm text-muted-foreground">กำลังโหลดตัวเลือก...</p>
        ) : (
          <form onSubmit={handleSubmit}>
            <FieldGroup>
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <Controller
                  name="budgetYearId"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="lab-group-budgetYearId">ปีงบประมาณ</FieldLabel>
                      <NativeSelect {...field} id="lab-group-budgetYearId" aria-invalid={fieldState.invalid}>
                        <option value="">— เลือก —</option>
                        {budgetYears.map((year) => (
                          <option key={year.id} value={year.id}>
                            {year.year}
                            {year.isActive ? '' : ' (ปิดใช้งาน)'}
                          </option>
                        ))}
                      </NativeSelect>
                      {selectedYear && (
                        <FieldDescription>
                          {formatThaiDate(selectedYear.startDate)} – {formatThaiDate(selectedYear.endDate)}
                        </FieldDescription>
                      )}
                      {budgetYears.length === 0 && (
                        <FieldDescription>
                          ยังไม่มีปีงบประมาณ — เพิ่มได้ที่ การตั้งค่า &gt; ตั้งค่าปีงบประมาณ
                        </FieldDescription>
                      )}
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
                <Controller
                  name="patientGroupId"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="lab-group-patientGroupId">กลุ่มคนไข้</FieldLabel>
                      <NativeSelect {...field} id="lab-group-patientGroupId" aria-invalid={fieldState.invalid}>
                        <option value="">— เลือก —</option>
                        {patientGroups.map((group) => (
                          <option key={group.id} value={group.id}>
                            {group.name}
                            {group.isActive ? '' : ' (ปิดใช้งาน)'}
                          </option>
                        ))}
                      </NativeSelect>
                      {patientGroups.length === 0 && (
                        <FieldDescription>
                          ยังไม่มีกลุ่มคนไข้ — เพิ่มได้ที่ การตั้งค่า &gt; ตั้งค่ากลุ่มคนไข้
                        </FieldDescription>
                      )}
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
              </div>

              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <Controller
                  name="cd4FirstDate"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="lab-group-cd4FirstDate">วันนัดตรวจ CD4 ครั้งที่ 1</FieldLabel>
                      <ThaiDateInput
                        {...field}
                        id="lab-group-cd4FirstDate"
                        aria-invalid={fieldState.invalid}
                        onChange={(value) => {
                          field.onChange(value)
                          // ยังไม่ได้แก้ครั้งที่ 2 เอง → ตั้งเป็นครั้งที่ 1 + 6 เดือนให้อัตโนมัติ
                          if (!secondEditedRef.current) {
                            const next = value ? addMonthsClamped(value, CD4_INTERVAL_MONTHS) : null
                            form.setValue('cd4SecondDate', next ?? '', {
                              shouldValidate: form.formState.isSubmitted,
                            })
                          }
                        }}
                      />
                      {field.value && <FieldDescription>{formatThaiDate(field.value)}</FieldDescription>}
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
                <Controller
                  name="cd4SecondDate"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="lab-group-cd4SecondDate">วันนัดตรวจ CD4 ครั้งที่ 2</FieldLabel>
                      <ThaiDateInput
                        {...field}
                        id="lab-group-cd4SecondDate"
                        aria-invalid={fieldState.invalid}
                        onChange={(value) => {
                          secondEditedRef.current = true
                          field.onChange(value)
                        }}
                      />
                      {/* สูตรของค่าเริ่มต้นแสดงคู่กับผลเสมอ ให้ตรวจซ้ำด้วยตาได้ (CLAUDE.md) */}
                      <FieldDescription>
                        ค่าเริ่มต้น = CD4 ครั้งที่ 1 + {CD4_INTERVAL_MONTHS} เดือน
                        {suggestedSecond && ` → ${formatThaiDate(suggestedSecond)}`}
                        {suggestedSecond && suggestedSecond !== cd4SecondDate && (
                          <>
                            {' '}
                            <button
                              type="button"
                              className="font-medium text-primary underline underline-offset-2"
                              onClick={() => {
                                secondEditedRef.current = false
                                setDate('cd4SecondDate', suggestedSecond)
                              }}
                            >
                              ใช้ค่านี้
                            </button>
                          </>
                        )}
                      </FieldDescription>
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
              </div>

              <div className="grid grid-cols-1 items-start gap-4 sm:grid-cols-2">
                <Controller
                  name="vlDate"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="lab-group-vlDate">วันนัดตรวจ VL</FieldLabel>
                      <ThaiDateInput {...field} id="lab-group-vlDate" aria-invalid={fieldState.invalid} />
                      {field.value && <FieldDescription>{formatThaiDate(field.value)}</FieldDescription>}
                      <div className="flex flex-wrap gap-2">
                        <Button
                          type="button"
                          variant="outline"
                          size="xs"
                          disabled={!cd4FirstDate}
                          onClick={() => setDate('vlDate', cd4FirstDate)}
                        >
                          ใช้วัน CD4 ครั้งที่ 1
                        </Button>
                        <Button
                          type="button"
                          variant="outline"
                          size="xs"
                          disabled={!cd4SecondDate}
                          onClick={() => setDate('vlDate', cd4SecondDate)}
                        >
                          ใช้วัน CD4 ครั้งที่ 2
                        </Button>
                      </div>
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
                <Controller
                  name="sortOrder"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="lab-group-sortOrder">ลำดับการแสดงผล</FieldLabel>
                      <Input
                        {...field}
                        id="lab-group-sortOrder"
                        type="number"
                        inputMode="numeric"
                        aria-invalid={fieldState.invalid}
                      />
                      <FieldDescription>
                        {mode === 'create' ? 'เว้นว่าง = ต่อท้ายสุด' : 'เว้นว่าง = คงลำดับเดิม'}
                      </FieldDescription>
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
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
