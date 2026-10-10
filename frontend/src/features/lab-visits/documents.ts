import type { Patient } from '@/features/patients/patientsApi'
import { formatThaiMonthYear, formatThaiShortDate } from '@/lib/date'

import type { LabVisit, LabVisitGroupDetail } from './labVisitsApi'

// ข้อมูลของเอกสารที่ส่งออก (Excel และหน้าพิมพ์/PDF ใช้ชุดเดียวกัน) — แบบตามกระดาษจริงใน
// docs/lab-visit-example-{head,detail}.jpg ไฟล์นี้เป็นฟังก์ชันล้วน ไม่แตะ DOM/เครือข่าย (เทสแบบ headless ได้)
// ผลลัพธ์มีข้อมูลระบุตัวคนไข้ (HN, ชื่อ) — ใช้แค่สร้างเอกสารในเครื่อง ห้าม log/เก็บลง storage

export type LabDocumentKind = 'cd4' | 'roster'

export const LAB_DOCUMENTS: Record<LabDocumentKind, { title: string; description: string }> = {
  cd4: {
    title: 'ใบนัดตรวจ CD4/VL',
    description: 'อายุ ระยะเวลากินยา วันนัด ผล VL สูตรยา โรคร่วม',
  },
  roster: {
    title: 'ใบรายชื่อนัดรับยา',
    description: 'HN ชื่อ NAP NO — น้ำหนัก ความดัน รายการแลป หมายเหตุ เว้นไว้เขียนมือ',
  },
}

export function patientFullName(patient: Patient): string {
  return `${patient.prenameName}${patient.firstName} ${patient.lastName}`
}

export interface JoinedVisit {
  visit: LabVisit
  // undefined = ไม่พบในทะเบียน (เช่น ถูกลบไประหว่างที่หน้านี้เปิดค้างอยู่)
  patient: Patient | undefined
}

// จับคู่รายชื่อในนัดหมาย (มีแค่ patientId) กับข้อมูลคนไข้จาก GET /patients — คงลำดับของ visits
export function joinVisits(visits: LabVisit[], patients: Patient[]): JoinedVisit[] {
  const byId = new Map(patients.map((patient) => [patient.id, patient]))
  return visits.map((visit) => ({ visit, patient: byId.get(visit.patientId) }))
}

const MISSING_PATIENT = '(ไม่พบข้อมูลคนไข้)'

function numberText(value: number | null | undefined): string {
  return value === null || value === undefined ? '' : String(value)
}

export interface Cd4Row {
  // uuid ของแถว lab_visit (ใช้เป็น key ตอนแสดงผล ไม่พิมพ์ลงเอกสาร)
  id: string
  hn: string
  name: string
  age: string
  takeMedYears: string
  takeMedMonths: string
  cd4First: string
  cd4Second: string
  vlDate: string
  vlResult: string
  drugRecipe: string
  comorbidities: string
}

export interface RosterRow {
  id: string
  hn: string
  name: string
  napNo: string
}

export interface LabDocumentData {
  groupName: string
  budgetYear: string
  // เดือน ปี ที่ส่งออก เช่น "ตุลาคม 2569" (หัวกระดาษ "ณ ...")
  asOf: string
  cd4Rows: Cd4Row[]
  rosterRows: RosterRow[]
}

// exportDate = วันที่ส่งออก "YYYY-MM-DD" (ส่งเข้ามา ไม่อ่านนาฬิกาเอง เพื่อให้เทสได้)
export function buildLabDocumentData(
  group: LabVisitGroupDetail,
  patients: Patient[],
  exportDate: string,
): LabDocumentData {
  const joined = joinVisits(group.visits, patients)
  const cd4First = formatThaiShortDate(group.cd4FirstDate)
  const cd4Second = formatThaiShortDate(group.cd4SecondDate)
  const vlDate = formatThaiShortDate(group.vlDate)

  return {
    groupName: group.patientGroupName,
    budgetYear: group.budgetYear,
    asOf: formatThaiMonthYear(exportDate),
    cd4Rows: joined.map(({ visit, patient }) => ({
      id: visit.id,
      hn: patient?.hn ?? '',
      name: patient ? patientFullName(patient) : MISSING_PATIENT,
      age: numberText(patient?.age),
      takeMedYears: numberText(visit.takeMedYearRange),
      takeMedMonths: numberText(visit.takeMedMonthRange),
      // วันนัดเก็บระดับนัดหมาย (ทุกคนในกลุ่ม/ปีเดียวกันใช้วันเดียวกัน) แต่กระดาษมีช่องรายคน จึงพิมพ์ซ้ำทุกแถว
      cd4First,
      cd4Second,
      vlDate,
      vlResult: visit.vlResult ?? '',
      drugRecipe: visit.drugRecipe ?? '',
      comorbidities: patient?.conditions.map((condition) => condition.abbreviation).join(', ') ?? '',
    })),
    rosterRows: joined.map(({ visit, patient }) => ({
      id: visit.id,
      hn: patient?.hn ?? '',
      name: patient ? patientFullName(patient) : MISSING_PATIENT,
      napNo: patient?.napNo ?? '',
    })),
  }
}

export function documentHeading(data: LabDocumentData): string {
  return `กลุ่ม ${data.groupName}`
}

export function cd4Subtitle(data: LabDocumentData): string {
  return `วันนัดตรวจ ซีดีโฟร์/ไวรัลโหลด (CD4/VL)   ปีงบประมาณ ${data.budgetYear} (ณ ${data.asOf})`
}

// บรรทัดใต้หัวใบรายชื่อ — เว้นจุดไว้เขียนวันที่ด้วยมือเหมือนกระดาษเดิม
export const ROSTER_DATE_LINE =
  'วันที่นัดรับยา ...............................      วันที่นัดครั้งต่อไป ...............................'

// หัวตารางใบนัด CD4/VL: หัวที่มี sub กินหลายคอลัมน์ (หัวบน) แล้วแตกเป็นหัวล่าง, ที่ไม่มี sub กินสองแถว
export const CD4_HEADER: { label: string; sub?: string[] }[] = [
  { label: 'HN' },
  { label: 'ชื่อ-นามสกุล' },
  { label: 'อายุ' },
  { label: 'ระยะเวลากินยา', sub: ['ปี', 'เดือน'] },
  { label: 'วันนัดตรวจ CD4', sub: ['ครั้งที่ 1', 'ครั้งที่ 2'] },
  { label: 'นัดตรวจ VL' },
  { label: 'ผล VL' },
  { label: 'สูตรยา' },
  { label: 'โรคร่วม' },
]

// ลำดับค่าในแถวตรงกับคอลัมน์ล่างสุดของ CD4_HEADER (11 คอลัมน์)
export function cd4RowCells(row: Cd4Row): string[] {
  return [
    row.hn,
    row.name,
    row.age,
    row.takeMedYears,
    row.takeMedMonths,
    row.cd4First,
    row.cd4Second,
    row.vlDate,
    row.vlResult,
    row.drugRecipe,
    row.comorbidities,
  ]
}

export const ROSTER_HEADER = ['HN', 'ชื่อ-นามสกุล', 'NAP NO', 'น้ำหนัก', 'ความดัน', 'รายการแลป', 'หมายเหตุ']

// ช่องหลัง NAP NO เว้นว่างไว้เขียนมือ (ยังไม่มีตารางเก็บค่าเหล่านี้)
export function rosterRowCells(row: RosterRow): string[] {
  return [row.hn, row.name, row.napNo, '', '', '', '']
}

// ชื่อไฟล์/ชื่อเอกสาร: ชนิดเอกสาร + กลุ่ม + ปีงบ (ไม่มีข้อมูลระบุตัวคนไข้) ตัดอักขระที่ใช้ในชื่อไฟล์ไม่ได้
export function documentFileName(kind: LabDocumentKind, data: LabDocumentData): string {
  return `${LAB_DOCUMENTS[kind].title} กลุ่ม ${data.groupName} ปีงบ ${data.budgetYear}`
    .replace(/[\\/:*?"<>|]+/g, '-')
    .replace(/\s+/g, ' ')
    .trim()
}
