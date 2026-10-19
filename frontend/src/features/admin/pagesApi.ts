import { api } from '@/lib/api'

// ตรงกับ PageResponse ของ backend เป๊ะ ๆ (backend/src/users/dto.rs) — เรียงตามโครงเมนู (เพจหลักแต่ละอันตามด้วย
// เพจย่อยของมัน) เมนู 2 ชั้น: refId = uuid ของเพจหลัก (null = เป็นเพจหลักเอง), route null = หัวข้อกลุ่ม
export interface Page {
  id: string
  label: string
  icon: string | null
  route: string | null
  sortOrder: number
  permissions: string[]
  refId: string | null
}

// ลำดับไม่ส่งจากหน้าเว็บ (จัดด้วยการลากวาง) — backend วางเพจใหม่/เพจที่ย้ายกลุ่มไว้ท้ายกลุ่มเอง
export interface CreatePageInput {
  label: string
  icon?: string
  // ไม่ส่ง = เพจหลักที่เป็นหัวข้อกลุ่ม (เพจย่อยต้องมี route)
  route?: string
  permissions: string[]
  refId?: string | null
}

export interface UpdatePageInput {
  label?: string
  icon?: string
  // ตั้ง route ใหม่ได้ แต่ล้างเป็น null ไม่ได้ (backend ไม่รองรับ)
  route?: string
  permissions?: string[]
  // ไม่ส่ง = ไม่แตะ, null = ย้ายขึ้นเป็นเพจหลัก
  refId?: string | null
}

export const pagesApi = {
  list: () => api.get<Page[]>('/pages'),
  // POST /pages คืนแค่ 201 ไม่มี body เหมือน roles — refetch list เองหลังสร้าง
  create: (input: CreatePageInput) => api.post<void>('/pages', input),
  update: (id: string, input: UpdatePageInput) => api.patch<void>(`/pages/${id}`, input),
  remove: (id: string) => api.delete<void>(`/pages/${id}`),
  // ลำดับใหม่ของเพจทั้งระดับ (refId null = เพจหลักชั้นบน) — ids ต้องครบชุด ไม่งั้น 409 page order out of date
  reorder: (refId: string | null, ids: string[]) => api.put<void>('/pages/order', { refId, ids }),
}
