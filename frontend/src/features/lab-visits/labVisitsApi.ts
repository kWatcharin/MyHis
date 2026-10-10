import { api } from '@/lib/api'

// ตรงกับ DTO ของ backend เป๊ะ ๆ (backend/src/lab_visit/dto.rs) — id ทุกตัว = uuid, วันที่เป็น "YYYY-MM-DD"
// API นี้ไม่มีข้อมูลระบุตัวคนไข้ (มีแค่ patientId) — ชื่อ/HN มาจาก GET /patients แล้ว join ฝั่งหน้าเว็บ

export interface LabVisitGroup {
  id: string
  budgetYearId: string
  budgetYear: string // ปีงบประมาณ พ.ศ. เช่น "2569"
  patientGroupId: string
  patientGroupName: string
  cd4FirstDate: string
  cd4SecondDate: string
  vlDate: string
  sortOrder: number
  patientCount: number
}

export interface LabVisit {
  id: string // uuid ของแถว lab_visit
  patientId: string
  takeMedYearRange: number | null
  takeMedMonthRange: number | null
  vlResult: string | null
  drugRecipe: string | null
  sortOrder: number
}

export interface LabVisitGroupDetail extends LabVisitGroup {
  visits: LabVisit[]
}

export interface LabVisitGroupInput {
  budgetYearId: string
  patientGroupId: string
  cd4FirstDate: string
  cd4SecondDate: string
  vlDate: string
  // สร้าง: null = ต่อท้ายลำดับสุดท้าย, แก้ไข: null = คงลำดับเดิม
  sortOrder: number | null
}

// แทนที่ทั้งชุด: null = ล้างค่า (ยกเว้น sortOrder null = คงลำดับเดิม)
export interface LabVisitInput {
  takeMedYearRange: number | null
  takeMedMonthRange: number | null
  vlResult: string | null
  drugRecipe: string | null
  sortOrder: number | null
}

export interface LabVisitOptions {
  // ทุกแถวที่ยังไม่ถูกลบ — ฟอร์มให้เลือกเฉพาะที่ isActive แต่ยังโชว์ค่าปัจจุบันของนัดหมายที่แก้อยู่
  budgetYears: { id: string; year: string; startDate: string; endDate: string; isActive: boolean }[]
  patientGroups: { id: string; name: string; isActive: boolean }[]
}

export const labVisitsApi = {
  list: () => api.get<LabVisitGroup[]>('/lab-visit-groups'),
  // ตัวเลือกปีงบประมาณ/กลุ่มคนไข้ — ต้องมี patient.read (ไม่ต้องมี user.manage เหมือน /budget-years)
  options: () => api.get<LabVisitOptions>('/lab-visit-groups/options'),
  get: (id: string) => api.get<LabVisitGroupDetail>(`/lab-visit-groups/${id}`),
  // POST คืน uuid ของนัดหมายใหม่ (ไว้พาไปหน้ารายชื่อต่อ); PUT/DELETE คืน 204 ไม่มี body
  create: (input: LabVisitGroupInput) => api.post<{ id: string }>('/lab-visit-groups', input),
  update: (id: string, input: LabVisitGroupInput) => api.put<void>(`/lab-visit-groups/${id}`, input),
  remove: (id: string) => api.delete<void>(`/lab-visit-groups/${id}`),
  // เพิ่มคนไข้หลายคนต่อท้ายตามลำดับที่ส่ง — คนใดคนหนึ่งไม่ผ่าน = ไม่เพิ่มสักคน
  addVisits: (groupId: string, patientIds: string[]) =>
    api.post<void>(`/lab-visit-groups/${groupId}/visits`, { patientIds }),
  updateVisit: (id: string, input: LabVisitInput) => api.put<void>(`/lab-visits/${id}`, input),
  removeVisit: (id: string) => api.delete<void>(`/lab-visits/${id}`),
}
