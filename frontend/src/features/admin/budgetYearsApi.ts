import { api } from '@/lib/api'

// ตรงกับ BudgetYearResponse ของ backend เป๊ะ ๆ (backend/src/budget_year/dto.rs)
// id = uuid, วันที่เป็น "YYYY-MM-DD" (ไม่มี timezone — เป็นวันที่ล้วน ๆ)
export interface BudgetYear {
  id: string
  year: string // พ.ศ. 4 หลัก เช่น "2570"
  startDate: string
  endDate: string
  sortOrder: number
  isActive: boolean
}

export interface CreateBudgetYearInput {
  year: string
  startDate: string
  endDate: string
  sortOrder?: number
  isActive?: boolean
}

export type UpdateBudgetYearInput = Partial<CreateBudgetYearInput>

export const budgetYearsApi = {
  list: () => api.get<BudgetYear[]>('/budget-years'),
  // POST คืนแค่ 201 ไม่มี body เหมือน roles/pages — refetch list เองหลังสร้าง
  create: (input: CreateBudgetYearInput) => api.post<void>('/budget-years', input),
  update: (id: string, input: UpdateBudgetYearInput) =>
    api.patch<void>(`/budget-years/${id}`, input),
  remove: (id: string) => api.delete<void>(`/budget-years/${id}`),
}
