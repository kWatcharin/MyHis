// mock data สำหรับหน้า settings ที่ยังไม่ต่อ API จริงรอบนี้ (RolesSettingsPage,
// PagesSettingsPage) — หน้าตา field ตรงกับ RoleResponse/PageResponse ของ backend เป๊ะ ๆ
// (backend/src/users/dto.rs) เพื่อให้สลับไปดึงข้อมูลจริงทีหลังได้โดยไม่ต้องแก้ component

export interface MockRole {
  id: string
  name: string
  description: string | null
  isSystem: boolean
  permissions: string[]
}

export interface MockPage {
  id: string
  label: string
  icon: string | null
  route: string
  sortOrder: number
  permissions: string[]
}

// ตรงกับ role ที่ seed จริงใน backend/migrations/20260923131951_init.sql
export const mockRoles: MockRole[] = [
  {
    id: '11111111-1111-4111-8111-111111111111',
    name: 'admin',
    description: 'ผู้ดูแลระบบ จัดการผู้ใช้/role/page ได้ทั้งหมด',
    isSystem: true,
    permissions: ['user.manage', 'shift.read', 'shift.write', 'audit.read'],
  },
  {
    id: '22222222-2222-4222-8222-222222222222',
    name: 'nurse',
    description: 'พยาบาลประจำเวร',
    isSystem: true,
    permissions: ['shift.read', 'shift.write'],
  },
]

// ตรงกับ page ที่ seed จริงใน backend/migrations/20260923144933_pages.sql
export const mockPages: MockPage[] = [
  {
    id: '33333333-3333-4333-8333-333333333333',
    label: 'Dashboard',
    icon: 'home',
    route: '/dashboard',
    sortOrder: 0,
    permissions: [],
  },
  {
    id: '44444444-4444-4444-8444-444444444444',
    label: 'การตั้งค่าเพจ',
    icon: 'layout-list',
    route: '/settings/pages',
    sortOrder: 10,
    permissions: ['user.manage'],
  },
  {
    id: '55555555-5555-4555-8555-555555555555',
    label: 'การตั้งค่า role',
    icon: 'shield-check',
    route: '/settings/roles',
    sortOrder: 20,
    permissions: ['user.manage'],
  },
]

// mock ตัวเลขจำนวนคนต่อเวรในสัปดาห์ — สำหรับกราฟหน้า dashboard เท่านั้น ไม่ใช่ข้อมูลจริง
// (ยังไม่มีฟีเจอร์ตารางเวรจริงใน Milestone นี้)
export const mockShiftChartData = [
  { day: 'จ', morning: 4, afternoon: 3, night: 2 },
  { day: 'อ', morning: 5, afternoon: 3, night: 2 },
  { day: 'พ', morning: 4, afternoon: 4, night: 3 },
  { day: 'พฤ', morning: 6, afternoon: 3, night: 2 },
  { day: 'ศ', morning: 5, afternoon: 4, night: 3 },
  { day: 'ส', morning: 3, afternoon: 2, night: 2 },
  { day: 'อา', morning: 3, afternoon: 2, night: 1 },
]
