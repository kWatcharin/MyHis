import { api } from '@/lib/api'

// ตรงกับ PageResponse ของ backend เป๊ะ ๆ (backend/src/users/dto.rs)
export interface Page {
  id: string
  label: string
  icon: string | null
  route: string
  sortOrder: number
  permissions: string[]
  groupLabel: string | null
}

export interface CreatePageInput {
  label: string
  icon?: string
  route: string
  sortOrder?: number
  permissions: string[]
  groupLabel?: string
}

export interface UpdatePageInput {
  label?: string
  icon?: string
  route?: string
  sortOrder?: number
  permissions?: string[]
  groupLabel?: string
}

export const pagesApi = {
  list: () => api.get<Page[]>('/pages'),
  // POST /pages คืนแค่ 201 ไม่มี body เหมือน roles — refetch list เองหลังสร้าง
  create: (input: CreatePageInput) => api.post<void>('/pages', input),
  update: (id: string, input: UpdatePageInput) => api.patch<void>(`/pages/${id}`, input),
  remove: (id: string) => api.delete<void>(`/pages/${id}`),
}
