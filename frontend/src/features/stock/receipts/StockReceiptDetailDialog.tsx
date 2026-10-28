import type { ReactNode } from 'react'

import { Badge } from '@/components/ui/badge'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { formatThaiDate, formatThaiDateTime } from '@/lib/date'

import type { StockReceipt, StockReceiptDetail, StockReceiptLine } from '../stockApi'
import { RECEIPT_TYPE_LABELS } from '../stockApi'

const n = (value: number) => value.toLocaleString('th-TH')

// "2 กล่อง × 100 = 200 เม็ด" — factor ที่ใช้ตอนรับ (แก้หน่วยแปลงของรายการภายหลังไม่กระทบใบเดิม)
function lineQuantity(line: StockReceiptLine): string {
  if (line.unitFactor === 1) return `${n(line.quantity)} ${line.unitName}`
  return `${n(line.quantity)} ${line.unitName} × ${n(line.unitFactor)} = ${n(line.baseQuantity)} ${line.baseUnitName}`
}

function InfoRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-xs text-muted-foreground">{label}</dt>
      <dd className="text-sm">{children}</dd>
    </div>
  )
}

interface StockReceiptDetailDialogProps {
  // แถวจากรายการ (แสดงหัวใบได้ทันที) + รายละเอียดที่โหลดตามมา (null = กำลังโหลด/โหลดไม่สำเร็จ)
  receipt: StockReceipt | null
  detail: StockReceiptDetail | null
  failed: boolean
  onOpenChange: (open: boolean) => void
}

// ดูใบรับ: หัวใบ + สถานะยกเลิก + บรรทัดพร้อมสูตรแปลงเป็นหน่วยฐาน
export function StockReceiptDetailDialog({
  receipt,
  detail,
  failed,
  onOpenChange,
}: StockReceiptDetailDialogProps) {
  const shown = detail ?? receipt
  return (
    <Dialog open={receipt !== null} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>ใบรับ {shown?.docNo}</DialogTitle>
        </DialogHeader>
        {shown && (
          <div className="flex flex-col gap-4">
            {shown.cancelledAt && (
              <div className="rounded-lg border border-danger/40 bg-danger/10 px-3 py-2 text-sm">
                <p className="font-medium text-danger-ink">
                  ยกเลิกแล้ว — ยอดของใบนี้ถูกกลับรายการออกจากคลัง
                </p>
                <p className="text-muted-foreground">
                  โดย {shown.cancelledByName ?? '—'} · {formatThaiDateTime(shown.cancelledAt)}
                </p>
                <p>เหตุผล: {shown.cancelReason}</p>
              </div>
            )}
            <dl className="grid grid-cols-2 gap-3 sm:grid-cols-3">
              <InfoRow label="ประเภท">
                <Badge variant={shown.receiptType === 'opening' ? 'info' : 'outline'}>
                  {RECEIPT_TYPE_LABELS[shown.receiptType]}
                </Badge>
              </InfoRow>
              <InfoRow label="วันที่รับ">{formatThaiDate(shown.receivedOn)}</InfoRow>
              <InfoRow label="ปีงบประมาณ">{shown.budgetYear}</InfoRow>
              <InfoRow label="แหล่งที่มา">{shown.source ?? '—'}</InfoRow>
              <InfoRow label="เลขที่เอกสารอ้างอิง">{shown.refNo ?? '—'}</InfoRow>
              <InfoRow label="ผู้รับ">
                {shown.createdByName}
                <span className="block text-xs text-muted-foreground">
                  บันทึก {formatThaiDateTime(shown.createdAt)}
                </span>
              </InfoRow>
            </dl>
            {shown.note && (
              <dl>
                <InfoRow label="หมายเหตุ">
                  <span className="whitespace-pre-wrap">{shown.note}</span>
                </InfoRow>
              </dl>
            )}

            {detail === null ? (
              <p className="py-4 text-sm text-muted-foreground">
                {failed ? 'โหลดรายการในใบไม่สำเร็จ ลองปิดแล้วเปิดใหม่' : 'กำลังโหลดรายการ...'}
              </p>
            ) : (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>ลำดับ</TableHead>
                    <TableHead>รายการ</TableHead>
                    <TableHead>เลขล็อต</TableHead>
                    <TableHead>วันหมดอายุ</TableHead>
                    <TableHead className="text-right">จำนวน (หน่วยฐาน)</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {detail.lines.map((line, index) => (
                    <TableRow key={`${line.itemId}-${line.lotNo ?? ''}`}>
                      <TableCell className="text-muted-foreground">{index + 1}</TableCell>
                      <TableCell className="font-medium">{line.itemName}</TableCell>
                      <TableCell className="text-muted-foreground">{line.lotNo ?? '—'}</TableCell>
                      <TableCell className="text-muted-foreground">
                        {line.expiryDate ? formatThaiDate(line.expiryDate) : '—'}
                      </TableCell>
                      <TableCell className="text-right whitespace-nowrap">{lineQuantity(line)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
