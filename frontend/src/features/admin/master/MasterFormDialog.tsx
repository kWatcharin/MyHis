import { useEffect, useMemo } from 'react'
import { Controller, useForm } from 'react-hook-form'
import type { FieldValues, Resolver } from 'react-hook-form'
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

import type { MasterField } from './types'

// ตรงกับขีดจำกัด 100 ตัวอักษรที่ backend บังคับกับทุก master (backend/src/<master>/service.rs)
const DEFAULT_MAX_LENGTH = 100

export type MasterFormValues = Record<string, string | number | boolean>

function buildSchema(fields: MasterField[]) {
  const shape: Record<string, z.ZodType> = {}
  for (const field of fields) {
    if (field.kind === 'text') {
      const max = field.maxLength ?? DEFAULT_MAX_LENGTH
      const base = z.string().trim().max(max, `${field.label}ยาวได้ไม่เกิน ${max} ตัวอักษร`)
      shape[field.name] = field.required ? base.min(1, `กรอก${field.label}`) : base
    } else if (field.kind === 'number') {
      shape[field.name] = z.number().int()
    } else {
      shape[field.name] = z.boolean()
    }
  }
  return z.object(shape)
}

function emptyValues(fields: MasterField[]): MasterFormValues {
  const values: MasterFormValues = {}
  for (const field of fields) {
    values[field.name] =
      field.kind === 'text' ? '' : field.kind === 'number' ? 0 : (field.defaultValue ?? true)
  }
  return values
}

// ค่าเริ่มต้นตอนแก้ไข: ดึงจากแถวเดิมตามชื่อฟิลด์ (null → '' สำหรับข้อความที่ไม่บังคับ)
function valuesFromItem(fields: MasterField[], item: object): MasterFormValues {
  const record = item as Record<string, unknown>
  const values = emptyValues(fields)
  for (const field of fields) {
    const raw = record[field.name]
    if (field.kind === 'text') values[field.name] = typeof raw === 'string' ? raw : ''
    else if (field.kind === 'number') values[field.name] = typeof raw === 'number' ? raw : 0
    else values[field.name] = typeof raw === 'boolean' ? raw : true
  }
  return values
}

interface MasterFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: 'create' | 'edit'
  entityLabel: string
  fields: MasterField[]
  // แถวเดิมที่กำลังแก้ (เฉพาะ mode edit)
  initialItem?: object
  onSubmit: (values: MasterFormValues) => Promise<void>
}

// ฟอร์มเพิ่ม/แก้ไข master ที่สร้างจากรายการฟิลด์ใน config (โครงเดียวกับ DiseaseFormDialog เดิม)
export function MasterFormDialog({
  open,
  onOpenChange,
  mode,
  entityLabel,
  fields,
  initialItem,
  onSubmit,
}: MasterFormDialogProps) {
  const schema = useMemo(() => buildSchema(fields), [fields])
  const form = useForm<FieldValues>({
    resolver: zodResolver(schema) as Resolver<FieldValues>,
    defaultValues: emptyValues(fields),
  })

  useEffect(() => {
    if (!open) return
    form.reset(
      mode === 'edit' && initialItem ? valuesFromItem(fields, initialItem) : emptyValues(fields),
    )
  }, [open, mode, initialItem, fields, form])

  const handleSubmit = form.handleSubmit(async (values) => {
    try {
      await onSubmit(values as MasterFormValues)
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
          <DialogTitle>{mode === 'create' ? `เพิ่ม${entityLabel}` : `แก้ไข${entityLabel}`}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <FieldGroup>
            {fields.map((field) => {
              const id = `master-${field.name}`
              if (field.kind === 'text') {
                return (
                  <Controller
                    key={field.name}
                    name={field.name}
                    control={form.control}
                    render={({ field: rhf, fieldState }) => (
                      <Field data-invalid={fieldState.invalid}>
                        <FieldLabel htmlFor={id}>{field.label}</FieldLabel>
                        <Input
                          {...rhf}
                          id={id}
                          maxLength={field.maxLength ?? DEFAULT_MAX_LENGTH}
                          placeholder={field.placeholder}
                          aria-invalid={fieldState.invalid}
                        />
                        {field.description && <FieldDescription>{field.description}</FieldDescription>}
                        {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                      </Field>
                    )}
                  />
                )
              }
              if (field.kind === 'number') {
                return (
                  <Controller
                    key={field.name}
                    name={field.name}
                    control={form.control}
                    render={({ field: { onChange, value, ...rhf } }) => (
                      <Field>
                        <FieldLabel htmlFor={id}>{field.label}</FieldLabel>
                        <Input
                          {...rhf}
                          id={id}
                          type="number"
                          value={value as number}
                          onChange={(e) => onChange(e.target.valueAsNumber || 0)}
                        />
                      </Field>
                    )}
                  />
                )
              }
              return (
                <Controller
                  key={field.name}
                  name={field.name}
                  control={form.control}
                  render={({ field: rhf }) => (
                    <Field>
                      <label className="flex items-center gap-2 text-sm font-normal">
                        <Checkbox
                          checked={rhf.value === true}
                          onCheckedChange={(checked) => rhf.onChange(checked === true)}
                        />
                        {field.label}
                      </label>
                      {field.description && <FieldDescription>{field.description}</FieldDescription>}
                    </Field>
                  )}
                />
              )
            })}
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
