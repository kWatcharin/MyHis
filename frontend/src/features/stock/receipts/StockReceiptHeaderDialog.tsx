import { useEffect } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import * as z from 'zod'

import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Field, FieldError, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'

import type { StockReceipt, StockReceiptHeaderInput } from '../stockApi'

// ตรงกับความยาวคอลัมน์ใน migration stock_receipts
const MAX_SOURCE_LENGTH = 200
const MAX_REF_NO_LENGTH = 50
const MAX_NOTE_LENGTH = 500

const headerSchema = z.object({
  source: z.string().max(MAX_SOURCE_LENGTH, `ยาวได้ไม่เกิน ${MAX_SOURCE_LENGTH} ตัวอักษร`),
  refNo: z.string().max(MAX_REF_NO_LENGTH, `ยาวได้ไม่เกิน ${MAX_REF_NO_LENGTH} ตัวอักษร`),
  note: z.string().max(MAX_NOTE_LENGTH, `ยาวได้ไม่เกิน ${MAX_NOTE_LENGTH} ตัวอักษร`),
})

type HeaderFormValues = z.infer<typeof headerSchema>

const blankToNull = (value: string) => (value.trim() === '' ? null : value.trim())

interface StockReceiptHeaderDialogProps {
  // ใบที่กำลังแก้ (null = ปิด)
  receipt: StockReceipt | null
  onOpenChange: (open: boolean) => void
  onSubmit: (input: StockReceiptHeaderInput) => Promise<void>
}

// แก้ข้อความหัวใบรับ (แหล่งที่มา / เลขที่อ้างอิง / หมายเหตุ) — บรรทัด/วันที่/ประเภทแก้ไม่ได้ (ผิด = ยกเลิกแล้วทำใหม่)
export function StockReceiptHeaderDialog({ receipt, onOpenChange, onSubmit }: StockReceiptHeaderDialogProps) {
  const form = useForm<HeaderFormValues>({
    resolver: zodResolver(headerSchema),
    defaultValues: { source: '', refNo: '', note: '' },
  })

  useEffect(() => {
    if (!receipt) return
    form.reset({ source: receipt.source ?? '', refNo: receipt.refNo ?? '', note: receipt.note ?? '' })
  }, [receipt, form])

  const handleSubmit = form.handleSubmit(async (values) => {
    try {
      await onSubmit({
        source: blankToNull(values.source),
        refNo: blankToNull(values.refNo),
        note: blankToNull(values.note),
      })
      onOpenChange(false)
    } catch (err) {
      form.setError('root', {
        message: err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง',
      })
    }
  })

  return (
    <Dialog open={receipt !== null} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>แก้ไขใบรับ {receipt?.docNo}</DialogTitle>
          <DialogDescription>
            แก้ได้เฉพาะข้อความหัวใบ ถ้าจำนวน/ล็อต/วันที่ผิด ให้ยกเลิกใบนี้แล้วบันทึกใบใหม่
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <FieldGroup>
            <Controller
              name="source"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="receipt-header-source">แหล่งที่มา</FieldLabel>
                  <Input {...field} id="receipt-header-source" maxLength={MAX_SOURCE_LENGTH} aria-invalid={fieldState.invalid} />
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <Controller
              name="refNo"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="receipt-header-refNo">เลขที่เอกสารอ้างอิง</FieldLabel>
                  <Input {...field} id="receipt-header-refNo" maxLength={MAX_REF_NO_LENGTH} aria-invalid={fieldState.invalid} />
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <Controller
              name="note"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="receipt-header-note">หมายเหตุ</FieldLabel>
                  <Textarea
                    {...field}
                    id="receipt-header-note"
                    rows={3}
                    maxLength={MAX_NOTE_LENGTH}
                    aria-invalid={fieldState.invalid}
                  />
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
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
