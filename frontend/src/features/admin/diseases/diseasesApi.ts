import { api, seg } from '@/lib/api'

// ตรงกับ DiseaseResponse ของ backend เป๊ะ ๆ (backend/src/disease/dto.rs) — id = uuid
export interface Disease {
  id: string
  name: string
  abbreviation: string
  sortOrder: number
  isActive: boolean
}

export interface CreateDiseaseInput {
  name: string
  abbreviation: string
  sortOrder?: number
  isActive?: boolean
}

export type UpdateDiseaseInput = Partial<CreateDiseaseInput>

export const diseasesApi = {
  list: () => api.get<Disease[]>('/diseases'),
  // POST คืนแค่ 201 ไม่มี body เหมือน roles/pages — refetch list เองหลังสร้าง
  create: (input: CreateDiseaseInput) => api.post<void>('/diseases', input),
  update: (id: string, input: UpdateDiseaseInput) =>
    api.patch<void>(`/diseases/${seg(id)}`, input),
  remove: (id: string) => api.delete<void>(`/diseases/${seg(id)}`),
}
