// แปลรายละเอียดการเปลี่ยนแปลงของประวัติการใช้งาน (Activity.details) เป็นสิ่งที่ modal "ดู" แสดง — ฟังก์ชันล้วน
// ชื่อช่อง (key) ตรงกับ snapshot ที่ backend เก็บ (repository ของแต่ละโมดูล) ค่าเป็นค่าสำหรับแสดงผลแล้ว (FK เป็นชื่อ)
import type { MenuItem } from '@/features/auth/AuthContext'
import { ACTION_LABELS, parsePermission, resourceLabel } from '@/features/admin/permissions'
import { formatThaiDate } from '@/lib/date'

import { describeAction } from './activities'
import type { Activity } from './auditApi'

// ชื่อช่องภาษาไทย — ลำดับใน object นี้คือลำดับที่แสดงในตาราง (ช่องที่ไม่รู้จักต่อท้ายด้วยชื่อดิบ)
const FIELD_LABELS: Record<string, string> = {
  // บัญชีผู้ใช้
  username: 'ชื่อผู้ใช้',
  display_name: 'ชื่อที่แสดง',
  // ทั่วไป / master
  name: 'ชื่อ',
  label: 'ชื่อเพจ',
  year: 'ปีงบประมาณ (พ.ศ.)',
  national_name: 'สัญชาติ',
  nation: 'เชื้อชาติ',
  abbreviation: 'ชื่อย่อ',
  description: 'คำอธิบาย',
  start_date: 'วันเริ่ม',
  end_date: 'วันสิ้นสุด',
  // เพจ
  icon: 'ไอคอน',
  route: 'ที่อยู่หน้า (route)',
  main_page: 'เพจหลัก',
  // นัดหมายแล็ป
  budget_year: 'ปีงบประมาณ',
  patient_group: 'กลุ่มคนไข้',
  cd4_first_date: 'วันนัด CD4 ครั้งที่ 1',
  cd4_second_date: 'วันนัด CD4 ครั้งที่ 2',
  vl_date: 'วันนัด VL',
  // ผู้ป่วย (แสดงแค่ชื่อช่อง)
  hn: 'HN',
  nap_no: 'NAP NO',
  prename: 'คำนำหน้า',
  first_name: 'ชื่อ',
  last_name: 'นามสกุล',
  position: 'ตำแหน่ง',
  birth_date: 'วันเกิด',
  age: 'อายุ',
  weight: 'น้ำหนัก',
  height: 'ส่วนสูง',
  sex: 'เพศ',
  is_thai: 'คนไทย',
  nationality: 'สัญชาติ',
  diseases: 'โรคประจำตัว',
  // คนไข้ในนัดหมาย (แสดงแค่ชื่อช่อง)
  take_med_year_range: 'ระยะเวลากินยา (ปี)',
  take_med_month_range: 'ระยะเวลากินยา (เดือน)',
  vl_result: 'ผล VL',
  drug_recipe: 'สูตรยา',
  // ท้ายตาราง
  roles: 'role',
  permissions: 'สิทธิ์',
  sort_order: 'ลำดับ',
  is_active: 'สถานะ',
}

const FIELD_ORDER = Object.keys(FIELD_LABELS)
const DATE_FIELDS = new Set(['start_date', 'end_date', 'cd4_first_date', 'cd4_second_date', 'vl_date', 'birth_date'])

export function fieldLabel(field: string): string {
  return FIELD_LABELS[field] ?? field
}

function orderedFields(fields: string[]): string[] {
  const rank = (field: string) => {
    const index = FIELD_ORDER.indexOf(field)
    return index === -1 ? FIELD_ORDER.length : index
  }
  return [...fields].sort((a, b) => rank(a) - rank(b) || a.localeCompare(b))
}

// "patient.view" → "ผู้ป่วย: ดู" (ชื่อเพจตามเมนู) — คีย์ที่ไม่รู้จักแสดงดิบ
function permissionLabel(key: string, menu: MenuItem[]): string {
  const parsed = parsePermission(key)
  return parsed ? `${resourceLabel(parsed.resource, menu)}: ${ACTION_LABELS[parsed.action]}` : key
}

function asStrings(value: unknown): string[] {
  return Array.isArray(value) ? value.map(String) : []
}

export function formatValue(field: string, value: unknown, menu: MenuItem[]): string {
  if (value === null || value === undefined || value === '') return '—'
  if (field === 'is_active') return value ? 'ใช้งาน' : 'ปิดใช้งาน'
  if (typeof value === 'boolean') return value ? 'ใช่' : 'ไม่ใช่'
  if (DATE_FIELDS.has(field) && typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value)) {
    return formatThaiDate(value)
  }
  if (field === 'permissions') {
    const keys = asStrings(value)
    return keys.length === 0 ? '— (ไม่มีสิทธิ์)' : keys.map((key) => permissionLabel(key, menu)).join(', ')
  }
  if (Array.isArray(value)) return value.length === 0 ? '—' : value.map(String).join(', ')
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}

export type DetailView =
  | { kind: 'created'; rows: { field: string; label: string; value: string }[] }
  | {
      kind: 'updated'
      rows: { field: string; label: string; before: string; after: string }[]
      // สิทธิ์ของ role/เพจ — แสดงเป็นที่เพิ่ม/เอาออก แทนรายการยาวสองชุด
      permissions: { added: string[]; removed: string[] } | null
    }
  | { kind: 'fields'; labels: string[] }
  | { kind: 'deleted' }
  | { kind: 'note'; text: string }
  | { kind: 'none' }

// การกระทำที่ตั้งใจไม่เก็บรายละเอียด — ข้อความแทน (ไม่ใช่ "บันทึกก่อนเริ่มเก็บ")
const NOTES: Record<string, string> = {
  user_activated: 'เปิดใช้งานบัญชีนี้',
  user_deactivated: 'ปิดใช้งานบัญชีนี้ (ผู้ใช้ถูกออกจากระบบทุกอุปกรณ์)',
  user_password_reset: 'รีเซ็ตรหัสผ่านและออกลิงก์ตั้งรหัสใหม่ — ไม่แสดงรายละเอียดเกี่ยวกับรหัสผ่าน',
  user_password_set: 'ผู้ใช้ตั้งรหัสผ่านใหม่ผ่านลิงก์ — ไม่แสดงรายละเอียดเกี่ยวกับรหัสผ่าน',
  password_changed: 'เปลี่ยนรหัสผ่านของตัวเอง — ไม่แสดงรายละเอียดเกี่ยวกับรหัสผ่าน',
  patient_created: 'เพิ่มผู้ป่วย 1 ราย — ไม่แสดงข้อมูลผู้ป่วย',
  lab_visit_created: 'เพิ่มคนไข้ 1 รายเข้านัดหมาย — ไม่แสดงข้อมูลผู้ป่วย',
}

export function buildDetailView(activity: Activity, menu: MenuItem[]): DetailView {
  const details = activity.details
  if (describeAction(activity.action).kind === 'delete') return { kind: 'deleted' }
  if (details?.changed) {
    return { kind: 'fields', labels: orderedFields(details.changed).map(fieldLabel) }
  }
  if (details?.before && details.after) {
    const before = details.before
    const after = details.after
    const fields = orderedFields(Object.keys({ ...before, ...after }))
    const plain = fields.filter((field) => field !== 'permissions')
    const permissions = fields.includes('permissions')
      ? {
          added: asStrings(after.permissions)
            .filter((key) => !asStrings(before.permissions).includes(key))
            .map((key) => permissionLabel(key, menu)),
          removed: asStrings(before.permissions)
            .filter((key) => !asStrings(after.permissions).includes(key))
            .map((key) => permissionLabel(key, menu)),
        }
      : null
    return {
      kind: 'updated',
      rows: plain.map((field) => ({
        field,
        label: fieldLabel(field),
        before: formatValue(field, before[field], menu),
        after: formatValue(field, after[field], menu),
      })),
      permissions,
    }
  }
  if (details?.after) {
    const after = details.after
    return {
      kind: 'created',
      rows: orderedFields(Object.keys(after)).map((field) => ({
        field,
        label: fieldLabel(field),
        value: formatValue(field, after[field], menu),
      })),
    }
  }
  const note = NOTES[activity.action]
  if (note) return { kind: 'note', text: note }
  return { kind: 'none' }
}
