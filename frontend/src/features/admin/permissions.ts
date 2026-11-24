// สิทธิ์แยกตามเพจ `<resource>.<action>` ตรงกับตาราง PERMISSIONS ฝั่ง backend (backend/src/domain/mod.rs) เป๊ะ —
// ต้อง sync มือ เพราะ backend ไม่มี endpoint คืนรายชื่อนี้ (ส่งคีย์ที่ไม่รู้จัก = 400 unknown permission)
// ใช้เป็นช่องติ๊กในฟอร์ม role (ตาราง เพจ × ดู/เพิ่ม/แก้ไข/ลบ) และฟอร์มเพจ แทนให้พิมพ์คีย์เอง
import type { MenuItem } from '@/features/auth/AuthContext'

export type PermissionAction = 'view' | 'create' | 'update' | 'delete' | 'export'

export const ACTION_LABELS: Record<PermissionAction, string> = {
  view: 'ดู',
  create: 'เพิ่ม',
  update: 'แก้ไข',
  delete: 'ลบ',
  export: 'ส่งออก',
}

export interface PermissionResource {
  key: string
  // route ของเพจ — ชื่อเพจ/กลุ่มที่แสดงในฟอร์ม role มาจากเมนู (การตั้งค่าเพจ) ที่ route ตรงกัน null = ยังไม่มีหน้า
  route: string | null
  // ชื่อเมื่อหาเพจในเมนูไม่เจอ (เพจถูกลบ/ผู้แก้มองไม่เห็นเพจนั้น)
  fallbackLabel: string
  actions: PermissionAction[]
  // ชื่อ action เฉพาะเพจนี้ (แทน ACTION_LABELS) เช่น "ลบ" ของใบรับคือ "ยกเลิก" — ใช้ผ่าน actionLabel()
  actionLabels?: Partial<Record<PermissionAction, string>>
  // ใช้งานหน้านี้ได้จริงต้องมีสิทธิ์นี้ด้วย (เตือนในฟอร์ม role)
  requires?: { permission: string; message: string }
}

const CRUD: PermissionAction[] = ['view', 'create', 'update', 'delete']

export const PERMISSION_RESOURCES: PermissionResource[] = [
  { key: 'patient', route: '/patients', fallbackLabel: 'ผู้ป่วย', actions: CRUD },
  { key: 'user', route: '/users', fallbackLabel: 'ผู้ใช้งาน', actions: ['view', 'create', 'update'] },
  {
    key: 'lab_visit',
    route: '/lab/appointments',
    fallbackLabel: 'นัดหมายแล็ป',
    actions: [...CRUD, 'export'],
    requires: {
      permission: 'patient.view',
      message: 'หน้านัดหมายแล็ปแสดงชื่อ/HN จากทะเบียนผู้ป่วย ต้องติ๊ก "ดู" ผู้ป่วยด้วยถึงจะใช้งานได้',
    },
  },
  { key: 'page', route: '/settings/pages', fallbackLabel: 'การตั้งค่าเพจ', actions: CRUD },
  { key: 'role', route: '/settings/roles', fallbackLabel: 'การตั้งค่าสิทธิ์', actions: CRUD },
  { key: 'budget_year', route: '/settings/budget-years', fallbackLabel: 'ตั้งค่าปีงบประมาณ', actions: CRUD },
  { key: 'disease', route: '/settings/diseases', fallbackLabel: 'ตั้งค่าโรค', actions: CRUD },
  { key: 'prename', route: '/settings/prenames', fallbackLabel: 'ตั้งค่าคำนำหน้า', actions: CRUD },
  { key: 'nationality', route: '/settings/nationalities', fallbackLabel: 'ตั้งค่าสัญชาติ', actions: CRUD },
  { key: 'patient_group', route: '/settings/patient-groups', fallbackLabel: 'ตั้งค่ากลุ่มคนไข้', actions: CRUD },
  { key: 'lab', route: '/settings/labs', fallbackLabel: 'ตั้งค่าแลป', actions: CRUD },
  // master คลัง — หน่วยแปลงของรายการใช้สิทธิ์ของ item (แก้ในฟอร์มรายการ)
  { key: 'item_category', route: '/settings/item-categories', fallbackLabel: 'ตั้งค่าหมวดคลัง', actions: CRUD },
  { key: 'item_unit', route: '/settings/item-units', fallbackLabel: 'ตั้งค่าหน่วยนับ', actions: CRUD },
  { key: 'item', route: '/settings/items', fallbackLabel: 'ตั้งค่ารายการคลัง', actions: CRUD },
  // ใบรับเข้า (+ ยอดยกมา): แก้ไข = แก้ข้อความหัวใบ, ลบ = ยกเลิกใบ (ลงรายการกลับยอด ไม่ลบแถว)
  {
    key: 'stock_receipt',
    route: '/stock/receipts',
    fallbackLabel: 'รับเข้า',
    actions: CRUD,
    actionLabels: { delete: 'ยกเลิก' },
  },
  { key: 'shift', route: null, fallbackLabel: 'ตารางเวร (ยังไม่มีหน้า)', actions: ['view', 'update'] },
  // audit.view ตัวเดียวคุมทุกหน้าใต้ "ประวัติ" (ประวัติการเข้าออก /audit/auth, ประวัติการใช้งาน)
  { key: 'audit', route: '/audit/auth', fallbackLabel: 'ประวัติการเข้าออก', actions: ['view'] },
  // หน้า SQL: รัน SQL อ่านอย่างเดียว (ทุกครั้งถูกบันทึกในประวัติการใช้งาน)
  {
    key: 'sql',
    route: '/data/script',
    fallbackLabel: 'SQL',
    actions: ['view'],
    requires: {
      permission: 'patient.view',
      message: 'ถอดรหัสชื่อ/HN ผู้ป่วยในผลลัพธ์ได้เมื่อติ๊ก "ดู" ผู้ป่วยด้วย (ไม่ติ๊ก = เห็นเป็นรหัส)',
    },
  },
]

// ชื่อ action ของเพจนี้ (ชื่อเฉพาะเพจก่อน ไม่มีใช้ชื่อกลาง)
export function actionLabel(resource: PermissionResource, action: PermissionAction): string {
  return resource.actionLabels?.[action] ?? ACTION_LABELS[action]
}

export function permissionKey(resource: string, action: PermissionAction): string {
  return `${resource}.${action}`
}

export const ALL_PERMISSIONS: string[] = PERMISSION_RESOURCES.flatMap((resource) =>
  resource.actions.map((action) => permissionKey(resource.key, action)),
)

// สิทธิ์จัดการ role — ต้องเหลือทั้งสองตัวกับบัญชีตัวเองเสมอ (backend ปฏิเสธด้วย
// "cannot remove your own role management permission")
export const ROLE_MANAGEMENT = ['role.view', 'role.update'] as const

export function grantsRoleManagement(permissions: readonly string[]): boolean {
  return ROLE_MANAGEMENT.every((permission) => permissions.includes(permission))
}

export interface LabeledResource {
  resource: PermissionResource
  label: string
}

export interface PermissionGroup {
  label: string
  resources: LabeledResource[]
}

const OTHER_GROUP = 'อื่น ๆ'

// จัดกลุ่มเพจตามเพจหลักในเมนู (ผลจาก GET /menu — ชื่อตรงกับการตั้งค่าเพจ ใครก็เรียกได้ ไม่ต้องมี page.view)
// เรียงตามลำดับเมนู เพจที่หาในเมนูไม่เจอ (ยังไม่มีหน้า/ผู้แก้มองไม่เห็น) อยู่กลุ่ม "อื่น ๆ" ท้ายสุดด้วยชื่อสำรอง
export function groupPermissionResources(menu: MenuItem[]): PermissionGroup[] {
  const groups: PermissionGroup[] = []
  const other: LabeledResource[] = []
  const placed = PERMISSION_RESOURCES.map((resource) => {
    const index = resource.route === null ? -1 : menu.findIndex((item) => item.route === resource.route)
    return { resource, index }
  }).sort((a, b) => a.index - b.index)

  for (const { resource, index } of placed) {
    const item = menu[index]
    if (!item) {
      other.push({ resource, label: resource.fallbackLabel })
      continue
    }
    const main = item.refId ? menu.find((m) => m.id === item.refId) : undefined
    const groupLabel = main?.label ?? 'เมนูหลัก'
    let group = groups.find((g) => g.label === groupLabel)
    if (!group) {
      group = { label: groupLabel, resources: [] }
      groups.push(group)
    }
    group.resources.push({ resource, label: item.label })
  }
  if (other.length > 0) groups.push({ label: OTHER_GROUP, resources: other })
  return groups
}

// ชื่อเพจของ resource จากเมนู (ไม่พบ = ชื่อสำรอง)
export function resourceLabel(resource: PermissionResource, menu: MenuItem[]): string {
  return menu.find((item) => item.route === resource.route)?.label ?? resource.fallbackLabel
}

// แยกคีย์ "patient.view" → resource + action (คีย์ที่ไม่รู้จัก = undefined)
export function parsePermission(
  key: string,
): { resource: PermissionResource; action: PermissionAction } | undefined {
  const dot = key.lastIndexOf('.')
  const resource = PERMISSION_RESOURCES.find((r) => r.key === key.slice(0, dot))
  const action = key.slice(dot + 1) as PermissionAction
  return resource && resource.actions.includes(action) ? { resource, action } : undefined
}
