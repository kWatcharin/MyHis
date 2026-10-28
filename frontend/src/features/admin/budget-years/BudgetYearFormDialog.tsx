import { useEffect } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import * as z from 'zod'

import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
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
import { ThaiDateInput } from '@/components/ui/thai-date-input'

import type { BudgetYear } from './budgetYearsApi'

const budgetYearFormSchema = z
  .object({
    year: z.string().regex(/^\d{4}$/, 'กรอกปีงบประมาณ พ.ศ. 4 หลัก เช่น 2570'),
    startDate: z.string().min(1, 'เลือกวันที่เริ่มต้น'),
    endDate: z.string().min(1, 'เลือกวันที่สิ้นสุด'),
    sortOrder: z.number().int(),
    isActive: z.boolean(),
  })
  .refine((v) => !v.startDate || !v.endDate || v.endDate >= v.startDate, {
    path: ['endDate'],
    message: 'วันที่สิ้นสุดต้องไม่ก่อนวันที่เริ่มต้น',
  })

export type BudgetYearFormValues = z.infer<typeof budgetYearFormSchema>

const EMPTY_VALUES: BudgetYearFormValues = {
  year: '',
  startDate: '',
  endDate: '',
  sortOrder: 0,
  isActive: true,
}

// ช่วงมาตรฐานของปีงบประมาณไทย: ปีงบประมาณ พ.ศ. Y เริ่ม 1 ต.ค. ของ ค.ศ. (Y − 543 − 1) และสิ้นสุด
// 30 ก.ย. ของ ค.ศ. (Y − 543) เช่น 2570 → 2026-10-01 ถึง 2027-09-30 — ใช้เติมให้ตอนสร้างเท่านั้น
// (ผู้ใช้แก้ได้) คืน null ถ้า year ยังไม่ครบ 4 หลัก
function defaultRangeForYear(year: string): { startDate: string; endDate: string } | null {
  if (!/^\d{4}$/.test(year)) return null
  const gregorian = Number(year) - 543
  return { startDate: `${gregorian - 1}-10-01`, endDate: `${gregorian}-09-30` }
}

interface BudgetYearFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: 'create' | 'edit'
  initialValue?: BudgetYear
  onSubmit: (values: BudgetYearFormValues) => Promise<void>
}

// ใช้ร่วมกันทั้งสร้าง/แก้ไขปีงบประมาณ — วันที่ใช้ ThaiDateInput (แสดง พ.ศ.) ค่าเป็น "YYYY-MM-DD"
// ตรงกับที่ backend รับ/ส่ง จึงเทียบ string ตรง ๆ ได้
export function BudgetYearFormDialog({
  open,
  onOpenChange,
  mode,
  initialValue,
  onSubmit,
}: BudgetYearFormDialogProps) {
  const form = useForm<BudgetYearFormValues>({
    resolver: zodResolver(budgetYearFormSchema),
    defaultValues: EMPTY_VALUES,
  })

  useEffect(() => {
    if (!open) return
    form.reset(
      mode === 'edit' && initialValue
        ? {
            year: initialValue.year,
            startDate: initialValue.startDate,
            endDate: initialValue.endDate,
            sortOrder: initialValue.sortOrder,
            isActive: initialValue.isActive,
          }
        : EMPTY_VALUES,
    )
  }, [open, mode, initialValue, form])

  // ตอนสร้าง: พอกรอกปีครบ 4 หลักและวันที่ยังว่างทั้งคู่ ให้เติมช่วงมาตรฐานให้ (ไม่ทับที่ผู้ใช้กรอกเอง)
  const fillDefaultRange = (year: string) => {
    if (mode !== 'create') return
    const range = defaultRangeForYear(year)
    if (!range) return
    if (form.getValues('startDate') || form.getValues('endDate')) return
    form.setValue('startDate', range.startDate, { shouldValidate: true })
    form.setValue('endDate', range.endDate, { shouldValidate: true })
  }

  const handleSubmit = form.handleSubmit(async (values) => {
    try {
      await onSubmit(values)
      onOpenChange(false)
    } catch (err) {
      form.setError('root', {
        message: err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง',
      })
    }
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{mode === 'create' ? 'เพิ่มปีงบประมาณ' : 'แก้ไขปีงบประมาณ'}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <FieldGroup>
            <Controller
              name="year"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="budget-year-year">ปีงบประมาณ (พ.ศ.)</FieldLabel>
                  <Input
                    {...field}
                    id="budget-year-year"
                    inputMode="numeric"
                    maxLength={4}
                    placeholder="2570"
                    aria-invalid={fieldState.invalid}
                    onChange={(e) => {
                      field.onChange(e)
                      fillDefaultRange(e.target.value)
                    }}
                  />
                  <FieldDescription>
                    ตอนเพิ่มใหม่ ระบบเติมช่วงวันที่มาตรฐาน (1 ต.ค. – 30 ก.ย.) ให้เมื่อกรอกปีครบ 4 หลัก
                    แก้ได้
                  </FieldDescription>
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
              <Controller
                name="startDate"
                control={form.control}
                render={({ field, fieldState }) => (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor="budget-year-start">วันที่เริ่มต้น</FieldLabel>
                    <ThaiDateInput {...field} id="budget-year-start" aria-invalid={fieldState.invalid} />
                    {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                  </Field>
                )}
              />
              <Controller
                name="endDate"
                control={form.control}
                render={({ field, fieldState }) => (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor="budget-year-end">วันที่สิ้นสุด</FieldLabel>
                    <ThaiDateInput {...field} id="budget-year-end" aria-invalid={fieldState.invalid} />
                    {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                  </Field>
                )}
              />
            </div>
            <Controller
              name="sortOrder"
              control={form.control}
              render={({ field: { onChange, value, ...field } }) => (
                <Field>
                  <FieldLabel htmlFor="budget-year-sort-order">ลำดับการแสดงผล</FieldLabel>
                  <Input
                    {...field}
                    id="budget-year-sort-order"
                    type="number"
                    value={value}
                    onChange={(e) => onChange(e.target.valueAsNumber || 0)}
                  />
                </Field>
              )}
            />
            <Controller
              name="isActive"
              control={form.control}
              render={({ field }) => (
                <Field>
                  <label className="flex items-center gap-2 text-sm font-normal">
                    <Checkbox
                      checked={field.value}
                      onCheckedChange={(checked) => field.onChange(checked === true)}
                    />
                    ใช้งานอยู่
                  </label>
                  <FieldDescription>ปิดไว้ถ้าไม่ต้องการให้ปีนี้ถูกเลือกใช้ต่อ</FieldDescription>
                </Field>
              )}
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
