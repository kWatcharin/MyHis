import { useState } from 'react'

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
import { Field, FieldDescription, FieldError, FieldLabel } from '@/components/ui/field'
import { Textarea } from '@/components/ui/textarea'

import type { StockReceipt } from '../stockApi'

// ตรงกับ stock_receipt.cancel_reason varchar(500)
const MAX_REASON_LENGTH = 500

interface CancelReceiptDialogProps {
  // ใบที่จะยกเลิก (null = ปิด)
  receipt: StockReceipt | null
  onOpenChange: (open: boolean) => void
  // throw Error(ข้อความไทย) เมื่อไม่สำเร็จ — แสดงในกล่องนี้
  onConfirm: (reason: string) => Promise<void>
}

// ยืนยันยกเลิกใบรับ (สิทธิ์ stock_receipt.delete) — ต้องมีเหตุผล ระบบลงรายการกลับยอดทุกบรรทัด ใบเดิมยังอยู่ในรายการ
export function CancelReceiptDialog({ receipt, onOpenChange, onConfirm }: CancelReceiptDialogProps) {
  const [reason, setReason] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [isSaving, setIsSaving] = useState(false)

  const close = (open: boolean) => {
    if (open) return
    setReason('')
    setError(null)
    onOpenChange(false)
  }

  const handleConfirm = async () => {
    if (reason.trim() === '') {
      setError('กรอกเหตุผลที่ยกเลิก')
      return
    }
    setIsSaving(true)
    try {
      await onConfirm(reason.trim())
      close(false)
    } catch (err) {
      setError(err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง')
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <AlertDialog open={receipt !== null} onOpenChange={close}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>ยกเลิกใบรับ {receipt?.docNo}</AlertDialogTitle>
          <AlertDialogDescription>
            ระบบจะลงรายการกลับยอดของทุกบรรทัดในใบนี้ออกจากคลัง (ใบเดิมยังอยู่ในรายการพร้อมสถานะยกเลิก)
            ยกเลิกแล้วทำย้อนไม่ได้ — ถ้าของถูกเบิกไปแล้วบางส่วนจะยกเลิกไม่ได้
          </AlertDialogDescription>
        </AlertDialogHeader>
        <Field data-invalid={error !== null}>
          <FieldLabel htmlFor="receipt-cancel-reason">เหตุผลที่ยกเลิก</FieldLabel>
          <Textarea
            id="receipt-cancel-reason"
            rows={3}
            value={reason}
            maxLength={MAX_REASON_LENGTH}
            onChange={(e) => {
              setReason(e.target.value)
              setError(null)
            }}
            aria-invalid={error !== null}
          />
          <FieldDescription>เช่น กรอกจำนวนผิด, รับผิดรายการ</FieldDescription>
          {error && <FieldError>{error}</FieldError>}
        </Field>
        <AlertDialogFooter>
          <AlertDialogCancel>ปิด</AlertDialogCancel>
          <AlertDialogAction
            variant="danger"
            disabled={isSaving}
            onClick={() => void handleConfirm()}
          >
            {isSaving ? 'กำลังยกเลิก...' : 'ยกเลิกใบรับ'}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
