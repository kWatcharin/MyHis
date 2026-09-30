import { api } from '@/lib/api'

// ตรงกับ RoleResponse ของ backend เป๊ะ ๆ (backend/src/users/dto.rs)
export interface Role {
  id: string
  name: string
  description: string | null
  isSystem: boolean
  permissions: string[]
}

export interface CreateRoleInput {
  name: string
  description?: string
  permissions: string[]
}

export interface UpdateRoleInput {
  name?: string
  description?: string
  permissions?: string[]
}

export const rolesApi = {
  list: () => api.get<Role[]>('/roles'),
  // POST /roles คืนแค่ 201 ไม่มี body (ดู backend/src/users/handlers.rs) — ผู้เรียกต้อง refetch
  // list เองถึงจะเห็น role ที่เพิ่งสร้าง
  create: (input: CreateRoleInput) => api.post<void>('/roles', input),
  update: (id: string, input: UpdateRoleInput) => api.patch<void>(`/roles/${id}`, input),
  remove: (id: string) => api.delete<void>(`/roles/${id}`),
}
