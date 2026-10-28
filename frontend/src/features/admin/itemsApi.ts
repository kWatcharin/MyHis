import { api, seg } from '@/lib/api'

// ตรงกับ DTO ของ backend เป๊ะ ๆ (backend/src/item/dto.rs) — id ทุกตัว = uuid
// จำนวนทุกช่องเป็นหน่วยฐาน (หน่วยเล็กสุดที่เบิกจริง) — factor = จำนวนหน่วยฐานต่อ 1 หน่วยนั้น

export interface ItemConversion {
  unitId: string
  unitName: string
  factor: number
}

export interface Item {
  id: string
  name: string
  categoryId: string
  categoryName: string
  baseUnitId: string
  baseUnitName: string
  trackLot: boolean
  trackExpiry: boolean
  patientLinkable: boolean
  reorderPoint: number | null
  isActive: boolean
  // เรียงตาม factor น้อยไปมาก
  conversions: ItemConversion[]
}

// ใช้ทั้งเพิ่ม (POST) และแทนที่ทั้งชุด (PUT) — หน่วยแปลงชุดนี้แทนที่ชุดเดิมทั้งหมด
export interface ItemInput {
  name: string
  categoryId: string
  baseUnitId: string
  trackLot: boolean
  trackExpiry: boolean
  patientLinkable: boolean
  // null = ไม่ตั้งจุดสั่งซื้อ
  reorderPoint: number | null
  isActive: boolean
  conversions: { unitId: string; factor: number }[]
}

// หมวด/หน่วยที่ใช้งานอยู่ — ค่าที่รายการผูกอยู่แล้วแม้ถูกปิดใช้งานภายหลังมากับแถวของรายการเอง
export interface ItemOptions {
  categories: {
    id: string
    name: string
    defaultTrackLot: boolean
    defaultTrackExpiry: boolean
    defaultPatientLinkable: boolean
  }[]
  units: { id: string; name: string }[]
}

export const itemsApi = {
  list: () => api.get<Item[]>('/items'),
  // ตัวเลือกของฟอร์ม — ต้องมี item.view (ไม่ต้องมีสิทธิ์หน้าหมวด/หน่วยนับ)
  options: () => api.get<ItemOptions>('/items/options'),
  // POST คืนแค่ 201, PUT/DELETE คืน 204 ไม่มี body — refetch list เองหลังบันทึก
  create: (input: ItemInput) => api.post<void>('/items', input),
  update: (id: string, input: ItemInput) => api.put<void>(`/items/${seg(id)}`, input),
  remove: (id: string) => api.delete<void>(`/items/${seg(id)}`),
}
