import { api, seg } from '@/lib/api'

import type { MasterApi } from './types'

// สร้าง api ของ master หนึ่งตัวจาก path — endpoint ทุกตัวหน้าตาเดียวกัน (backend/src/<master>/routes.rs):
// GET/POST /x, PATCH/DELETE /x/{id} โดย POST คืนแค่ 201 ไม่มี body (refetch list เองหลังสร้าง)
// ค่าที่คืนเป็น object ของ arrow function (ไม่ใช้ this) เพื่อให้ reference ของ list คงที่
export function createMasterApi<T>(path: string): MasterApi<T> {
  return {
    list: () => api.get<T[]>(path),
    create: (input) => api.post<void>(path, input),
    update: (id, input) => api.patch<void>(`${path}/${seg(id)}`, input),
    remove: (id) => api.delete<void>(`${path}/${seg(id)}`),
  }
}
