import { api } from '@/lib/api'

// ตรงกับ UserSummary ของ backend (backend/src/users/dto.rs) — id/role id เป็น uuid ไม่มี id int ภายใน
export interface UserRoleRef {
  id: string
  name: string
}

interface UserResponse {
  id: string
  username: string
  displayName: string
  isActive: boolean
  // บัญชีเดิมที่ยังใช้รหัสผ่านชั่วคราว (ก่อนมีลิงก์ลงทะเบียน) — บัญชีใหม่ไม่มีค่านี้แล้ว
  mustChangePassword: boolean
  // ล็อกชั่วคราวจากการใส่รหัสผิดหลายครั้งถึงเวลานี้ (ISO, UTC) — null หรือเวลาในอดีต = ไม่ล็อก
  lockedUntil: string | null
  prenameId: string | null
  prenameName: string | null
  firstName: string | null
  lastName: string | null
  position: string | null
  // ตั้งรหัสผ่านเองผ่านลิงก์สำเร็จเมื่อ — null = ยังไม่ลงทะเบียน (login ไม่ได้)
  registeredAt: string | null
  // หมดอายุของลิงก์ลงทะเบียน/ตั้งรหัสที่ค้างอยู่ (อาจหมดอายุแล้ว) — null = ไม่มีลิงก์ค้าง
  registrationExpiresAt: string | null
  roles: UserRoleRef[]
  createdAt: string
}

// สถานะของลิงก์ที่ค้างอยู่ ณ ตอนโหลดรายการ: ไม่มี / ยังใช้ได้ / หมดอายุแล้ว
export type LinkState = 'none' | 'pending' | 'expired'

export interface User extends UserResponse {
  // คำนวณตอน fetch ไม่ใช่ตอน render — หน้า refetch ทุกครั้งหลังแก้ไข
  isLocked: boolean
  linkState: LinkState
}

export interface CreateUserInput {
  username: string
  displayName: string
  // สร้างได้ทีละ 1 role (docs/auth-design.md ข้อ 10) — เพิ่ม role อื่นทีหลังจากฟอร์มแก้ไข
  roleId: string
}

// แทนที่ทั้งชุดในคำสั่งเดียว (PUT /users/{id}) — null = ล้างค่า
export interface ReplaceUserInput {
  displayName: string
  prenameId: string | null
  firstName: string | null
  lastName: string | null
  position: string | null
  roleIds: string[]
}

// token ของลิงก์ลงทะเบียน/ตั้งรหัสใหม่ — มีใน response ครั้งเดียว แสดงใน dialog แล้วทิ้ง ห้ามเก็บ/log
export interface RegistrationLink {
  registrationToken: string
  registrationExpiresAt: string
}

export const usersApi = {
  list: async (): Promise<User[]> => {
    const users = await api.get<UserResponse[]>('/users')
    const now = Date.now()
    return users.map((user) => ({
      ...user,
      isLocked: user.lockedUntil !== null && Date.parse(user.lockedUntil) > now,
      linkState:
        user.registrationExpiresAt === null
          ? 'none'
          : Date.parse(user.registrationExpiresAt) > now
            ? 'pending'
            : 'expired',
    }))
  },
  // สร้างบัญชีที่ยังไม่ลงทะเบียน + ลิงก์ลงทะเบียน (ผู้ใช้กรอกข้อมูลและตั้งรหัสผ่านเอง)
  create: (input: CreateUserInput) =>
    api.post<{ user: UserResponse } & RegistrationLink>('/users', input),
  // PATCH คืน 204 ไม่มี body (ใช้เปิด/ปิดใช้งาน) — ผู้เรียกต้อง refetch เอง
  update: (id: string, input: { isActive?: boolean; displayName?: string }) =>
    api.patch<void>(`/users/${id}`, input),
  // ฟอร์มแก้ไข: ชื่อที่แสดง + ข้อมูลตัว + ชุด role ในทรานแซกชันเดียว
  replace: (id: string, input: ReplaceUserInput) => api.put<void>(`/users/${id}`, input),
  // รหัสเดิมใช้ไม่ได้ทันที + ปลดล็อก + ออกจากระบบทุกอุปกรณ์ + ได้ลิงก์ตั้งรหัสใหม่ (แทนลิงก์เดิม)
  resetPassword: (id: string) => api.post<RegistrationLink>(`/users/${id}/reset-password`),
}

// ลิงก์ที่ส่งให้ผู้ใช้ — token อยู่หลัง # (fragment ไม่ถูกส่งไป server จึงไม่ติด log)
export function registrationUrl(token: string): string {
  return `${window.location.origin}/register#${token}`
}

// ชื่อเต็ม "นางสมศรี ใจดี" (ไม่มีข้อมูล = null)
export function fullName(user: Pick<User, 'prenameName' | 'firstName' | 'lastName'>): string | null {
  if (!user.firstName && !user.lastName) return null
  return `${user.prenameName ?? ''}${user.firstName ?? ''} ${user.lastName ?? ''}`.trim()
}
