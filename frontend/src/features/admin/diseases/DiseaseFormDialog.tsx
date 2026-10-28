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

import type { Disease } from './diseasesApi'

// ตรงกับ varchar(100) ของ disease.name / abbreviation — backend ตรวจซ้ำอีกชั้น (trim แล้วไม่ว่าง)
const MAX_LENGTH = 100

const diseaseFormSchema = z.object({
  name: z
    .string()
    .trim()
    .min(1, 'กรอกชื่อโรค')
    .max(MAX_LENGTH, `ชื่อโรคยาวได้ไม่เกิน ${MAX_LENGTH} ตัวอักษร`),
  abbreviation: z
    .string()
    .trim()
    .min(1, 'กรอกคำย่อ')
    .max(MAX_LENGTH, `คำย่อยาวได้ไม่เกิน ${MAX_LENGTH} ตัวอักษร`),
  sortOrder: z.number().int(),
  isActive: z.boolean(),
})

export type DiseaseFormValues = z.infer<typeof diseaseFormSchema>

const EMPTY_VALUES: DiseaseFormValues = {
  name: '',
  abbreviation: '',
  sortOrder: 0,
  isActive: true,
}

interface DiseaseFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: 'create' | 'edit'
  initialValue?: Disease
  onSubmit: (values: DiseaseFormValues) => Promise<void>
}

// ใช้ร่วมกันทั้งสร้าง/แก้ไขโรค (โครงเดียวกับ BudgetYearFormDialog)
export function DiseaseFormDialog({
  open,
  onOpenChange,
  mode,
  initialValue,
  onSubmit,
}: DiseaseFormDialogProps) {
  const form = useForm<DiseaseFormValues>({
    resolver: zodResolver(diseaseFormSchema),
    defaultValues: EMPTY_VALUES,
  })

  useEffect(() => {
    if (!open) return
    form.reset(
      mode === 'edit' && initialValue
        ? {
            name: initialValue.name,
            abbreviation: initialValue.abbreviation,
            sortOrder: initialValue.sortOrder,
            isActive: initialValue.isActive,
          }
        : EMPTY_VALUES,
    )
  }, [open, mode, initialValue, form])

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
          <DialogTitle>{mode === 'create' ? 'เพิ่มโรค' : 'แก้ไขโรค'}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <FieldGroup>
            <Controller
              name="name"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="disease-name">ชื่อโรค</FieldLabel>
                  <Input
                    {...field}
                    id="disease-name"
                    maxLength={MAX_LENGTH}
                    aria-invalid={fieldState.invalid}
                  />
                  <FieldDescription>ชื่อโรคซ้ำกันไม่ได้ (ไม่สนตัวพิมพ์เล็ก/ใหญ่)</FieldDescription>
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <Controller
              name="abbreviation"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="disease-abbreviation">คำย่อ</FieldLabel>
                  <Input
                    {...field}
                    id="disease-abbreviation"
                    maxLength={MAX_LENGTH}
                    aria-invalid={fieldState.invalid}
                  />
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <Controller
              name="sortOrder"
              control={form.control}
              render={({ field: { onChange, value, ...field } }) => (
                <Field>
                  <FieldLabel htmlFor="disease-sort-order">ลำดับการแสดงผล</FieldLabel>
                  <Input
                    {...field}
                    id="disease-sort-order"
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
                  <FieldDescription>ปิดไว้ถ้าไม่ต้องการให้โรคนี้ถูกเลือกใช้ต่อ</FieldDescription>
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
