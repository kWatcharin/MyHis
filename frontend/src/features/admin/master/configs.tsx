import { sortOrderColumn, statusColumn, textColumn } from './columns'
import { createMasterApi } from './mastersApi'
import type { MasterConfig } from './types'

// config ของหน้าตั้งค่า master 4 ตัว (คำนำหน้า, สัญชาติ, กลุ่มคนไข้, แลป) — ชนิดข้อมูลตรงกับ
// *Response ของ backend เป๊ะ ๆ (backend/src/<master>/dto.rs) id = uuid, ฟิลด์เป็น camelCase
// ชื่อ error ใน errorMessages ต้องตรงกับข้อความที่ backend ส่ง (backend/src/<master>/service.rs)

// ---- คำนำหน้า (prename) — ไม่มี is_active ตาม data dict, abbreviation ไม่บังคับ ----
export interface Prename {
  id: string
  name: string
  abbreviation: string | null
  sortOrder: number
}

export const prenameConfig: MasterConfig<Prename> = {
  resource: 'prename',
  title: 'ตั้งค่าคำนำหน้า',
  description: 'คำนำหน้าชื่อที่ใช้บันทึกข้อมูลคนไข้ เช่น นาย นาง นางสาว',
  entityLabel: 'คำนำหน้า',
  api: createMasterApi<Prename>('/prenames'),
  columns: [
    sortOrderColumn(),
    textColumn('คำนำหน้า', (item) => item.name, 'font-medium'),
    textColumn('คำย่อ', (item) => item.abbreviation),
  ],
  fields: [
    {
      kind: 'text',
      name: 'name',
      label: 'คำนำหน้าเต็ม',
      required: true,
      description: 'ชื่อซ้ำกันไม่ได้ (ไม่สนตัวพิมพ์เล็ก/ใหญ่)',
    },
    { kind: 'text', name: 'abbreviation', label: 'คำย่อ (ไม่บังคับ)' },
    { kind: 'number', name: 'sortOrder', label: 'ลำดับการแสดงผล' },
  ],
  searchFields: [
    { name: 'name', label: 'คำนำหน้า', value: (item) => item.name },
    { name: 'abbreviation', label: 'คำย่อ', value: (item) => item.abbreviation },
  ],
  itemLabel: (item) => item.name,
  errorMessages: {
    'prename name already taken': 'คำนำหน้านี้มีอยู่แล้ว',
    'prename is still used': 'ลบไม่ได้ ยังมีคนไข้หรือผู้ใช้งานใช้คำนำหน้านี้อยู่',
  },
  deleteNote: 'ถ้ายังมีคนไข้หรือผู้ใช้งานใช้คำนำหน้านี้อยู่ ระบบจะไม่ให้ลบ',
}

// ---- สัญชาติ (nationality) ----
export interface Nationality {
  id: string
  nationalName: string
  nation: string
  sortOrder: number
  isActive: boolean
}

export const nationalityConfig: MasterConfig<Nationality> = {
  resource: 'nationality',
  title: 'ตั้งค่าสัญชาติ',
  description: 'เชื้อชาติ/สัญชาติและประเทศ ที่ใช้บันทึกข้อมูลคนไข้',
  entityLabel: 'สัญชาติ',
  api: createMasterApi<Nationality>('/nationalities'),
  columns: [
    sortOrderColumn(),
    textColumn('เชื้อชาติ/สัญชาติ', (item) => item.nationalName, 'font-medium'),
    textColumn('ประเทศ', (item) => item.nation),
    statusColumn(),
  ],
  fields: [
    {
      kind: 'text',
      name: 'nationalName',
      label: 'เชื้อชาติ/สัญชาติ',
      required: true,
      description: 'ชื่อซ้ำกันไม่ได้ (ไม่สนตัวพิมพ์เล็ก/ใหญ่)',
    },
    { kind: 'text', name: 'nation', label: 'ประเทศ', required: true },
    { kind: 'number', name: 'sortOrder', label: 'ลำดับการแสดงผล' },
    {
      kind: 'checkbox',
      name: 'isActive',
      label: 'ใช้งานอยู่',
      description: 'ปิดไว้ถ้าไม่ต้องการให้ถูกเลือกใช้ต่อ',
    },
  ],
  searchFields: [
    { name: 'nationalName', label: 'เชื้อชาติ/สัญชาติ', value: (item) => item.nationalName },
    { name: 'nation', label: 'ประเทศ', value: (item) => item.nation },
  ],
  statusOf: (item) => item.isActive,
  itemLabel: (item) => item.nationalName,
  errorMessages: {
    'nationality name already taken': 'เชื้อชาติ/สัญชาตินี้มีอยู่แล้ว',
    'nationality is still used': 'ลบไม่ได้ ยังมีคนไข้ใช้สัญชาตินี้อยู่',
  },
  deleteNote: 'ถ้ายังมีคนไข้ใช้สัญชาตินี้อยู่ ระบบจะไม่ให้ลบ',
}

// ---- กลุ่มคนไข้ (patient_group) ----
export interface PatientGroup {
  id: string
  name: string
  sortOrder: number
  isActive: boolean
}

export const patientGroupConfig: MasterConfig<PatientGroup> = {
  resource: 'patient_group',
  title: 'ตั้งค่ากลุ่มคนไข้',
  description: 'กลุ่มคนไข้ที่ใช้จัดนัดหมายและกลุ่มที่มา',
  entityLabel: 'กลุ่มคนไข้',
  api: createMasterApi<PatientGroup>('/patient-groups'),
  columns: [
    sortOrderColumn(),
    textColumn('กลุ่มคนไข้', (item) => item.name, 'font-medium'),
    statusColumn(),
  ],
  fields: [
    {
      kind: 'text',
      name: 'name',
      label: 'ชื่อกลุ่มคนไข้',
      required: true,
      description: 'ชื่อซ้ำกันไม่ได้ (ไม่สนตัวพิมพ์เล็ก/ใหญ่)',
    },
    { kind: 'number', name: 'sortOrder', label: 'ลำดับการแสดงผล' },
    {
      kind: 'checkbox',
      name: 'isActive',
      label: 'ใช้งานอยู่',
      description: 'ปิดไว้ถ้าไม่ต้องการให้กลุ่มนี้ถูกเลือกใช้ต่อ',
    },
  ],
  searchFields: [{ name: 'name', label: 'ชื่อกลุ่มคนไข้', value: (item) => item.name }],
  statusOf: (item) => item.isActive,
  itemLabel: (item) => item.name,
  errorMessages: {
    'patient group name already taken': 'ชื่อกลุ่มคนไข้นี้มีอยู่แล้ว',
    'patient group is still used': 'ลบไม่ได้ ยังมีนัดหมายหรือกลุ่มที่มาใช้กลุ่มคนไข้นี้อยู่',
  },
  deleteNote: 'ถ้ายังมีนัดหมายหรือกลุ่มที่มาใช้กลุ่มนี้อยู่ ระบบจะไม่ให้ลบ',
}

// ---- แลป (lab) — abbreviation ไม่บังคับ, ยังไม่มีตารางอ้างอิง (ลบได้เสมอ) ----
export interface Lab {
  id: string
  name: string
  abbreviation: string | null
  isActive: boolean
  sortOrder: number
}

export const labConfig: MasterConfig<Lab> = {
  resource: 'lab',
  title: 'ตั้งค่าแลป',
  description: 'รายการแลปที่ใช้ตรวจ เช่น CD4, Viral load',
  entityLabel: 'แลป',
  api: createMasterApi<Lab>('/labs'),
  columns: [
    sortOrderColumn(),
    textColumn('ชื่อแลป', (item) => item.name, 'font-medium'),
    textColumn('คำย่อ', (item) => item.abbreviation),
    statusColumn(),
  ],
  fields: [
    {
      kind: 'text',
      name: 'name',
      label: 'ชื่อเต็ม',
      required: true,
      description: 'ชื่อซ้ำกันไม่ได้ (ไม่สนตัวพิมพ์เล็ก/ใหญ่)',
    },
    { kind: 'text', name: 'abbreviation', label: 'คำย่อ (ไม่บังคับ)' },
    { kind: 'number', name: 'sortOrder', label: 'ลำดับการแสดงผล' },
    {
      kind: 'checkbox',
      name: 'isActive',
      label: 'ใช้งานอยู่',
      description: 'ปิดไว้ถ้าไม่ต้องการให้แลปนี้ถูกเลือกใช้ต่อ',
    },
  ],
  searchFields: [
    { name: 'name', label: 'ชื่อแลป', value: (item) => item.name },
    { name: 'abbreviation', label: 'คำย่อ', value: (item) => item.abbreviation },
  ],
  statusOf: (item) => item.isActive,
  itemLabel: (item) => item.name,
  errorMessages: {
    'lab name already taken': 'ชื่อแลปนี้มีอยู่แล้ว',
  },
  deleteNote: 'การลบนี้ย้อนกลับไม่ได้',
}
