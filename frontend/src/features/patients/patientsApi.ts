import { api } from '@/lib/api'

// ตรงกับ PatientResponse / PatientRequest ของ backend เป๊ะ ๆ (backend/src/patient/dto.rs)
// id ทุกตัว = uuid; วันที่เป็น "YYYY-MM-DD"; ข้อมูลระบุตัวคนไข้ (hn, napNo, firstName, lastName) ถูกถอดรหัส
// โดย backend แล้ว — อย่าเก็บลง localStorage / URL / console

export interface PatientCondition {
  id: string // uuid ของแถว medical_condition
  diseaseId: string
  name: string
  abbreviation: string
}

export interface Patient {
  id: string
  hn: string
  napNo: string
  prenameId: string
  prenameName: string
  firstName: string
  lastName: string
  birthDate: string | null
  age: number | null
  weight: number | null
  height: number | null
  sex: 1 | 2 // 1 = ชาย, 2 = หญิง
  isThai: boolean
  nationalityId: string | null
  nationalityName: string | null
  sortOrder: number
  isActive: boolean
  conditions: PatientCondition[]
}

export interface PatientInput {
  hn: string
  napNo: string
  prenameId: string
  firstName: string
  lastName: string
  birthDate: string | null
  age: number | null
  weight: number | null
  height: number | null
  sex: number
  isThai: boolean
  nationalityId: string | null
  // สร้าง: null = ต่อท้ายลำดับสุดท้าย, แก้ไข: null = คงลำดับเดิม
  sortOrder: number | null
  isActive: boolean
  // โรคประจำตัว (uuid ของ disease) ตามลำดับ — ชุดนี้แทนที่โรคที่ผูกอยู่เดิมทั้งหมด
  diseaseIds: string[]
}

export interface PatientOptions {
  prenames: { id: string; name: string; abbreviation: string | null }[]
  nationalities: { id: string; nationalName: string; nation: string }[]
  diseases: { id: string; name: string; abbreviation: string }[]
}

export const patientsApi = {
  list: () => api.get<Patient[]>('/patients'),
  // ตัวเลือกของฟอร์ม (คำนำหน้า, สัญชาติ, โรค) — ต้องมี patient.read (ไม่ต้องมี user.manage เหมือน /diseases)
  options: () => api.get<PatientOptions>('/patients/options'),
  // POST คืนแค่ 201 ไม่มี body — refetch list เองหลังสร้าง; PUT แทนที่ทั้งชุด (ฟิลด์ nullable ส่ง null = ล้างค่า)
  create: (input: PatientInput) => api.post<void>('/patients', input),
  update: (id: string, input: PatientInput) => api.put<void>(`/patients/${id}`, input),
  remove: (id: string) => api.delete<void>(`/patients/${id}`),
}
