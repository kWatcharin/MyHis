// แปลแถวประวัติการใช้งาน (GET /audit/activities) เป็นข้อความไทย — ฟังก์ชันล้วน ไม่แตะเครือข่าย
// action ตั้งชื่อ `<ชนิด>_created/_updated/_deleted/_exported` (backend เขียนเองในแต่ละโมดูล) + action พิเศษของผู้ใช้งาน
// ชื่อหน้ามาจากการตั้งค่าเพจ (เมนู) ผ่าน PERMISSION_RESOURCES — targetType ตรงกับคีย์ resource ยกเว้นที่แมปไว้ข้างล่าง
import type { MenuItem } from '@/features/auth/AuthContext'
import { PERMISSION_RESOURCES, resourceLabel } from '@/features/admin/permissions'
import { LAB_DOCUMENTS } from '@/features/lab-visits/documents'

import type { Activity } from './auditApi'

export type ActivityKind = 'create' | 'update' | 'delete' | 'export' | 'other'

export const KIND_LABELS: Record<ActivityKind, string> = {
  create: 'เพิ่ม',
  update: 'แก้ไข',
  delete: 'ลบ',
  export: 'ส่งออก',
  other: 'อื่น ๆ',
}

// สีป้ายการกระทำ (ตารางประวัติ + กล่องรายละเอียด) — ความหมายเดียวกับปุ่ม: เพิ่ม = เขียว, แก้ไข = ส้ม, ลบ = แดง
export const KIND_BADGE: Record<ActivityKind, 'success' | 'warning' | 'danger' | 'info' | 'outline'> = {
  create: 'success',
  update: 'warning',
  delete: 'danger',
  export: 'info',
  other: 'outline',
}

const SUFFIX_KINDS: [string, ActivityKind][] = [
  ['_created', 'create'],
  ['_updated', 'update'],
  ['_deleted', 'delete'],
  ['_exported', 'export'],
]

const SPECIAL_ACTIONS: Record<string, { label: string; kind: ActivityKind }> = {
  user_activated: { label: 'เปิดใช้งาน', kind: 'update' },
  user_deactivated: { label: 'ปิดใช้งาน', kind: 'update' },
  user_roles_replaced: { label: 'เปลี่ยน role', kind: 'update' },
  user_password_reset: { label: 'รีเซ็ตรหัสผ่าน (ออกลิงก์ใหม่)', kind: 'update' },
  user_registered: { label: 'ลงทะเบียน', kind: 'other' },
  user_password_set: { label: 'ตั้งรหัสผ่านใหม่', kind: 'other' },
  password_changed: { label: 'เปลี่ยนรหัสผ่านของตัวเอง', kind: 'other' },
  // หน้า SQL — รายการสรุปจาก backend เช่น "12 แถว · patient" (ไม่มีตัว SQL)
  sql_executed: { label: 'รัน SQL', kind: 'other' },
}

// ชื่อการกระทำ + ประเภท (action ที่ไม่รู้จัก = แสดงชื่อดิบ ประเภท "อื่น ๆ")
export function describeAction(action: string): { label: string; kind: ActivityKind } {
  const special = SPECIAL_ACTIONS[action]
  if (special) return special
  for (const [suffix, kind] of SUFFIX_KINDS) {
    if (action.endsWith(suffix)) return { label: KIND_LABELS[kind], kind }
  }
  return { label: action, kind: 'other' }
}

// หน้าที่ไม่ใช่เพจในเมนู
const ACCOUNT_PAGE = 'account'
const REGISTRATION_PAGE = 'registration'
const OTHER_PAGE = 'other'
const EXTRA_PAGE_LABELS: Record<string, string> = {
  [ACCOUNT_PAGE]: 'บัญชีของฉัน',
  [REGISTRATION_PAGE]: 'ลงทะเบียนผ่านลิงก์',
  [OTHER_PAGE]: 'อื่น ๆ',
}
// targetType ที่ไม่ตรงกับคีย์ resource
const RESOURCE_OF_TARGET: Record<string, string> = { lab_visit_group: 'lab_visit' }
// เพจที่ไม่มีการเพิ่ม/แก้ไข/ลบ (ไม่ต้องอยู่ในตัวกรอง)
const READ_ONLY_RESOURCES = new Set(['shift', 'audit'])

// คีย์หน้าของแถว (ใช้กรอง) — resource key หรือ account/registration/other
export function activityPage(activity: Activity): string {
  if (activity.action === 'password_changed') return ACCOUNT_PAGE
  if (activity.action === 'user_registered' || activity.action === 'user_password_set') return REGISTRATION_PAGE
  const type = activity.targetType ?? ''
  const key = RESOURCE_OF_TARGET[type] ?? type
  return PERMISSION_RESOURCES.some((resource) => resource.key === key) ? key : OTHER_PAGE
}

export function pageLabel(key: string, menu: MenuItem[]): string {
  const resource = PERMISSION_RESOURCES.find((r) => r.key === key)
  return resource ? resourceLabel(resource, menu) : (EXTRA_PAGE_LABELS[key] ?? key)
}

// ตัวเลือกของตัวกรอง "หน้า" ตามลำดับเดียวกับฟอร์มสิทธิ์
export function pageOptions(menu: MenuItem[]): { key: string; label: string }[] {
  const keys = [
    ...PERMISSION_RESOURCES.filter((r) => !READ_ONLY_RESOURCES.has(r.key)).map((r) => r.key),
    ACCOUNT_PAGE,
    REGISTRATION_PAGE,
  ]
  return keys.map((key) => ({ key, label: pageLabel(key, menu) }))
}

function exportDescription(activity: Activity): string {
  const kind = activity.exportDocument
  const title = kind === 'cd4' || kind === 'roster' ? LAB_DOCUMENTS[kind].title : 'เอกสาร'
  const format = activity.exportFormat === 'excel' ? 'Excel' : activity.exportFormat === 'pdf' ? 'PDF' : null
  return format ? `${title} (${format})` : title
}

// คอลัมน์ "รายการ" — แถวผู้ป่วยไม่มีตัวตน (backend ไม่ส่งชื่อรายการของผู้ป่วยอยู่แล้ว)
export function describeTarget(activity: Activity): string {
  if (activity.targetType === 'patient') return 'ผู้ป่วย 1 ราย'
  if (activity.targetType === 'lab_visit') {
    return activity.targetLabel ? `คนไข้ 1 ราย · ${activity.targetLabel}` : 'คนไข้ 1 ราย'
  }
  if (activity.action === 'lab_visit_group_exported') {
    return [activity.targetLabel, exportDescription(activity)].filter(Boolean).join(' · ')
  }
  return activity.targetLabel ?? '—'
}
