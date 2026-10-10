import { api } from '@/lib/api'

// ตรงกับ LookupResponse ของ backend (backend/src/registration/dto.rs) — endpoint สาธารณะ ไม่ต้อง login
// token อยู่ใน body ของ POST เสมอ (ไม่ใส่ใน URL/query ที่ไปติด log ของ server)
export interface RegistrationInfo {
  username: string
  displayName: string
  // register = ลงทะเบียนครั้งแรก (กรอกข้อมูลตัว + ตั้งรหัส), reset = ตั้งรหัสใหม่อย่างเดียว
  mode: 'register' | 'reset'
  expiresAt: string
  prenameId: string | null
  firstName: string | null
  lastName: string | null
  position: string | null
  prenames: { id: string; name: string }[]
}

export interface CompleteRegistrationInput {
  token: string
  newPassword: string
  prenameId?: string
  firstName?: string
  lastName?: string
  position?: string | null
}

export const registrationApi = {
  lookup: (token: string) => api.post<RegistrationInfo>('/registration/lookup', { token }),
  complete: (input: CompleteRegistrationInput) =>
    api.post<void>('/registration/complete', input),
}
