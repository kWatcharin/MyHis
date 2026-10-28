import { Ban, Eye } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'

import { AddButton, EditButton } from '@/components/layout/ActionButtons'
import { ListState } from '@/components/layout/ListState'
import { ListToolbar } from '@/components/layout/ListToolbar'
import { PageHeader } from '@/components/layout/PageHeader'
import { SearchForm } from '@/components/layout/SearchForm'
import { TablePagination } from '@/components/layout/TablePagination'
import { usePagination } from '@/components/layout/usePagination'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { useAdminList } from '@/features/admin/useAdminList'
import { usePagePermissions } from '@/features/role-preview/usePagePermissions'
import { formatThaiDate } from '@/lib/date'
import { notify } from '@/lib/notify'
import { hasCriteria, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'

import type {
  ReceiptOptions,
  StockReceipt,
  StockReceiptDetail,
  StockReceiptHeaderInput,
  StockReceiptInput,
} from '../stockApi'
import { RECEIPT_TYPE_LABELS, stockApi } from '../stockApi'
import { CancelReceiptDialog } from './CancelReceiptDialog'
import { receiptErrorMessage } from './receiptErrors'
import { StockReceiptDetailDialog } from './StockReceiptDetailDialog'
import { StockReceiptFormDialog } from './StockReceiptFormDialog'
import { StockReceiptHeaderDialog } from './StockReceiptHeaderDialog'

const TYPE_OPTIONS = [
  { value: 'receive', label: RECEIPT_TYPE_LABELS.receive },
  { value: 'opening', label: RECEIPT_TYPE_LABELS.opening },
]

const STATUS_OPTIONS = [
  { value: 'active', label: 'ปกติ' },
  { value: 'cancelled', label: 'ยกเลิกแล้ว' },
]

// หน้ารับเข้า (กลุ่มเมนู "คลัง" — backend/src/stock) ใบรับ + ยอดยกมา เรียงวันที่รับใหม่→เก่าจาก backend
// บันทึกแล้วลงคลังทันที: แก้ได้แค่ข้อความหัวใบ, ผิดต้องยกเลิกทั้งใบ (สิทธิ์ "ลบ" ของหน้านี้ = ยกเลิก)
// mutation คืนแค่ status (เพิ่มคืนเลขที่ใบ) จึง refetch() ทั้ง list หลังทุกครั้ง
export function StockReceiptsPage() {
  const { canCreate, canUpdate, canDelete } = usePagePermissions('stock_receipt')
  const { items: receipts, status, refetch } = useAdminList(stockApi.listReceipts)
  const [options, setOptions] = useState<ReceiptOptions | null>(null)
  const [isCreating, setIsCreating] = useState(false)
  const [viewing, setViewing] = useState<StockReceipt | null>(null)
  const [detail, setDetail] = useState<StockReceiptDetail | null>(null)
  const [detailFailed, setDetailFailed] = useState(false)
  const [editing, setEditing] = useState<StockReceipt | null>(null)
  const [cancelling, setCancelling] = useState<StockReceipt | null>(null)
  // เงื่อนไขค้นหาที่ใช้กรองจริง (เปลี่ยนเมื่อกดปุ่ม "ค้นหา") — ช่องที่กรอกต้องตรงทุกช่อง
  const [criteria, setCriteria] = useState<SearchValues>({})

  // ตัวเลือกช่องค้นหา "ปีงบประมาณ" = ปีที่มีใบรับอยู่จริง (ใหม่ก่อน ตามที่ backend เรียงมา)
  const yearOptions = useMemo(() => {
    const years = new Map<string, string>()
    for (const receipt of receipts) {
      if (!years.has(receipt.budgetYearId)) years.set(receipt.budgetYearId, receipt.budgetYear)
    }
    return [...years].map(([value, label]) => ({ value, label }))
  }, [receipts])

  const visibleReceipts = receipts.filter(
    (receipt) =>
      (textIncludes(receipt.docNo, criteria.docNo ?? '') ||
        textIncludes(receipt.refNo, criteria.docNo ?? '')) &&
      textIncludes(receipt.source, criteria.source ?? '') &&
      (!criteria.budgetYear || receipt.budgetYearId === criteria.budgetYear) &&
      (!criteria.type || receipt.receiptType === criteria.type) &&
      (!criteria.status || (criteria.status === 'cancelled') === (receipt.cancelledAt !== null)),
  )
  const pagination = usePagination(visibleReceipts)
  const rowOffset = (pagination.props.page - 1) * pagination.props.pageSize

  // ตัวเลือกของฟอร์ม โหลดใหม่ทุกครั้งที่เปิดฟอร์ม (เผื่อมีการแก้รายการ/ปีงบระหว่างที่หน้านี้เปิดค้าง)
  const openCreate = useCallback(async () => {
    setOptions(null)
    setIsCreating(true)
    try {
      setOptions(await stockApi.receiptOptions())
    } catch {
      notify.error('โหลดตัวเลือกรายการ/ปีงบประมาณไม่สำเร็จ ลองรีเฟรชหน้า')
    }
  }, [])

  const openDetail = async (receipt: StockReceipt) => {
    setViewing(receipt)
    setDetail(null)
    setDetailFailed(false)
    try {
      setDetail(await stockApi.getReceipt(receipt.id))
    } catch {
      setDetailFailed(true)
    }
  }

  const handleCreate = async (input: StockReceiptInput) => {
    let docNo: string
    try {
      docNo = (await stockApi.createReceipt(input)).docNo
    } catch (err) {
      throw new Error(receiptErrorMessage(err))
    }
    await refetch()
    notify.created(`บันทึกรับเข้าแล้ว — เลขที่ ${docNo}`)
  }

  const handleUpdate = async (input: StockReceiptHeaderInput) => {
    if (!editing) return
    try {
      await stockApi.updateReceipt(editing.id, input)
    } catch (err) {
      throw new Error(receiptErrorMessage(err))
    }
    await refetch()
    notify.updated(`แก้ไขใบรับ ${editing.docNo} แล้ว`)
  }

  const handleCancel = async (reason: string) => {
    if (!cancelling) return
    try {
      await stockApi.cancelReceipt(cancelling.id, reason)
    } catch (err) {
      throw new Error(receiptErrorMessage(err))
    }
    await refetch()
    notify.deleted(`ยกเลิกใบรับ ${cancelling.docNo} แล้ว`)
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        fallbackTitle="รับเข้า"
        description="บันทึกของที่รับเข้าคลังและยอดยกมา — บันทึกแล้วลงคลังทันที ถ้าจำนวน/ล็อตผิดให้ยกเลิกทั้งใบแล้วทำใหม่"
      />

      <SearchForm
        id="stock-receipt-search"
        ariaLabel="ค้นหาใบรับ"
        fields={[
          { name: 'docNo', label: 'เลขที่ใบรับ / เลขอ้างอิง', placeholder: 'เช่น RC2570-0001' },
          { name: 'source', label: 'แหล่งที่มา' },
          { name: 'budgetYear', label: 'ปีงบประมาณ', type: 'select', options: yearOptions, allLabel: 'ทุกปี' },
          { name: 'type', label: 'ประเภท', type: 'select', options: TYPE_OPTIONS, allLabel: 'ทุกประเภท' },
          { name: 'status', label: 'สถานะ', type: 'select', options: STATUS_OPTIONS, allLabel: 'ทุกสถานะ' },
        ]}
        onSearch={(values) => {
          setCriteria(values)
          pagination.reset()
        }}
      />

      <ListToolbar
        ready={status === 'ready'}
        total={receipts.length}
        shown={visibleReceipts.length}
        filtered={hasCriteria(criteria)}
      >
        {canCreate && <AddButton onClick={() => void openCreate()}>เพิ่มใบรับ</AddButton>}
      </ListToolbar>

      <Card>
        <CardContent className="flex flex-col gap-3 px-0 sm:px-2">
          {status === 'loading' && <ListState kind="loading" />}
          {status === 'error' && <ListState kind="error">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</ListState>}
          {status === 'ready' && receipts.length === 0 && (
            <ListState kind="empty">
              ยังไม่มีใบรับ กด &quot;+ เพิ่มใบรับ&quot; เพื่อเริ่มต้น (เริ่มใช้ระบบ: บันทึกประเภท &quot;ยอดยกมา&quot;)
            </ListState>
          )}
          {status === 'ready' && receipts.length > 0 && visibleReceipts.length === 0 && (
            <ListState kind="noMatch">
              ไม่พบใบรับที่ตรงกับเงื่อนไข — กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด
            </ListState>
          )}
          {status === 'ready' && visibleReceipts.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>ลำดับ</TableHead>
                  <TableHead>เลขที่</TableHead>
                  <TableHead>วันที่รับ</TableHead>
                  <TableHead>ประเภท</TableHead>
                  <TableHead>แหล่งที่มา</TableHead>
                  <TableHead>เลขอ้างอิง</TableHead>
                  <TableHead className="text-right">บรรทัด</TableHead>
                  <TableHead>ผู้รับ</TableHead>
                  <TableHead>สถานะ</TableHead>
                  <TableHead className="text-right">จัดการ</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((receipt, index) => {
                  const cancelled = receipt.cancelledAt !== null
                  return (
                    <TableRow key={receipt.id}>
                      <TableCell className="text-muted-foreground">{rowOffset + index + 1}</TableCell>
                      <TableCell className="font-medium whitespace-nowrap">{receipt.docNo}</TableCell>
                      <TableCell className="whitespace-nowrap">{formatThaiDate(receipt.receivedOn)}</TableCell>
                      <TableCell>
                        <Badge variant={receipt.receiptType === 'opening' ? 'info' : 'outline'}>
                          {RECEIPT_TYPE_LABELS[receipt.receiptType]}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-muted-foreground">{receipt.source ?? '—'}</TableCell>
                      <TableCell className="text-muted-foreground">{receipt.refNo ?? '—'}</TableCell>
                      <TableCell className="text-right text-muted-foreground">{receipt.lineCount}</TableCell>
                      <TableCell className="text-muted-foreground">{receipt.createdByName}</TableCell>
                      <TableCell>
                        <Badge variant={cancelled ? 'danger' : 'success'}>
                          {cancelled ? 'ยกเลิกแล้ว' : 'ปกติ'}
                        </Badge>
                      </TableCell>
                      <TableCell className="text-right whitespace-nowrap">
                        <Button variant="outline" size="sm" onClick={() => void openDetail(receipt)}>
                          <Eye />
                          ดู
                        </Button>
                        {canUpdate && !cancelled && <EditButton onClick={() => setEditing(receipt)} />}
                        {canDelete && !cancelled && (
                          <Button variant="danger" size="sm" onClick={() => setCancelling(receipt)}>
                            <Ban />
                            ยกเลิก
                          </Button>
                        )}
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          )}
          {status === 'ready' && visibleReceipts.length > 0 && <TablePagination {...pagination.props} />}
        </CardContent>
      </Card>

      <StockReceiptFormDialog
        open={isCreating}
        onOpenChange={setIsCreating}
        options={options}
        onSubmit={handleCreate}
      />
      <StockReceiptDetailDialog
        receipt={viewing}
        detail={detail}
        failed={detailFailed}
        onOpenChange={(open) => {
          if (!open) setViewing(null)
        }}
      />
      <StockReceiptHeaderDialog
        receipt={editing}
        onOpenChange={(open) => {
          if (!open) setEditing(null)
        }}
        onSubmit={handleUpdate}
      />
      <CancelReceiptDialog
        receipt={cancelling}
        onOpenChange={(open) => {
          if (!open) setCancelling(null)
        }}
        onConfirm={handleCancel}
      />
    </div>
  )
}
