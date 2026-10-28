import { api, seg } from '@/lib/api'

// ตรงกับ DTO ของ backend เป๊ะ ๆ (backend/src/stock/dto.rs) — id ทุกตัว = uuid, วันที่ = "YYYY-MM-DD", เวลา = ISO (UTC)
// จำนวนเป็นหน่วยฐานของรายการ ยกเว้น quantity ของบรรทัด (หน่วยที่กรอก) — baseQuantity = quantity × unitFactor

export type ReceiptType = 'receive' | 'opening'

export const RECEIPT_TYPE_LABELS: Record<ReceiptType, string> = {
  receive: 'รับเข้า',
  opening: 'ยอดยกมา',
}

export interface StockReceipt {
  id: string
  // เช่น RC2570-0001 (รันต่อปีงบ)
  docNo: string
  budgetYearId: string
  budgetYear: string
  receiptType: ReceiptType
  receivedOn: string
  source: string | null
  refNo: string | null
  note: string | null
  lineCount: number
  // ผู้รับ (ผู้บันทึกใบ)
  createdByName: string
  createdAt: string
  // null = ยังไม่ยกเลิก
  cancelledByName: string | null
  cancelledAt: string | null
  cancelReason: string | null
}

export interface StockReceiptLine {
  itemId: string
  itemName: string
  lotNo: string | null
  expiryDate: string | null
  unitName: string
  // หน่วยฐานต่อ 1 หน่วยที่กรอก ณ วันรับ
  unitFactor: number
  quantity: number
  baseQuantity: number
  baseUnitName: string
}

export interface StockReceiptDetail extends StockReceipt {
  lines: StockReceiptLine[]
}

export interface StockReceiptLineInput {
  itemId: string
  // หน่วยฐานหรือหน่วยแปลงของรายการ
  unitId: string
  quantity: number
  // รายการที่ไม่ติดตามล็อต/วันหมดอายุ = null
  lotNo: string | null
  expiryDate: string | null
}

export interface StockReceiptInput {
  receiptType: ReceiptType
  budgetYearId: string
  receivedOn: string
  source: string | null
  refNo: string | null
  note: string | null
  lines: StockReceiptLineInput[]
}

// แก้ได้แค่ข้อความหัวใบ — ส่งครบสามช่อง (null = ล้าง)
export interface StockReceiptHeaderInput {
  source: string | null
  refNo: string | null
  note: string | null
}

export interface ReceiptUnitOption {
  id: string
  name: string
  factor: number
}

export interface ReceiptItemOption {
  id: string
  name: string
  categoryName: string
  baseUnitId: string
  baseUnitName: string
  trackLot: boolean
  trackExpiry: boolean
  // หน่วยแปลง เรียงตาม factor
  conversions: ReceiptUnitOption[]
}

export interface ReceiptBudgetYearOption {
  id: string
  year: string
  startDate: string
  endDate: string
}

// เฉพาะรายการ/ปีงบที่ใช้งานอยู่ + แหล่งที่มาที่เคยกรอก (ใช้ล่าสุดก่อน)
export interface ReceiptOptions {
  items: ReceiptItemOption[]
  budgetYears: ReceiptBudgetYearOption[]
  sources: string[]
}

export interface StockLot {
  lotNo: string
  expiryDate: string | null
  qtyOnHand: number
}

export const stockApi = {
  listReceipts: () => api.get<StockReceipt[]>('/stock/receipts'),
  getReceipt: (id: string) => api.get<StockReceiptDetail>(`/stock/receipts/${seg(id)}`),
  // ตัวเลือกของฟอร์มใบรับ — ต้องมี stock_receipt.view (ไม่ต้องมีสิทธิ์หน้าตั้งค่ารายการ/ปีงบ)
  receiptOptions: () => api.get<ReceiptOptions>('/stock/receipts/options'),
  createReceipt: (input: StockReceiptInput) =>
    api.post<{ id: string; docNo: string }>('/stock/receipts', input),
  // PUT/ยกเลิก คืน 204 ไม่มี body — refetch list เองหลังบันทึก
  updateReceipt: (id: string, input: StockReceiptHeaderInput) =>
    api.put<void>(`/stock/receipts/${seg(id)}`, input),
  cancelReceipt: (id: string, reason: string) =>
    api.post<void>(`/stock/receipts/${seg(id)}/cancel`, { reason }),
  // ล็อตที่มีเลขของรายการ (มีของก่อน แล้วหมดอายุก่อน) — เติมช่อง lot/วันหมดอายุในฟอร์ม
  lots: (itemId: string) => api.get<StockLot[]>(`/stock/lots?item=${encodeURIComponent(itemId)}`),
}
