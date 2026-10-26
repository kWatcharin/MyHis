import type { ReactNode } from 'react'

// ชนิดฟิลด์ของฟอร์ม master — ใช้สร้างทั้ง zod schema และช่องกรอกใน MasterFormDialog
// ชื่อฟิลด์ (`name`) ต้องตรงกับ key ใน JSON ที่ backend รับ/ส่ง (camelCase เช่น nationalName)
export type MasterField =
  | {
      kind: 'text'
      name: string
      label: string
      // บังคับกรอกไหม (ฟิลด์ที่ไม่บังคับ ส่งสตริงว่างได้ backend เก็บเป็น NULL)
      required?: boolean
      maxLength?: number
      description?: string
      placeholder?: string
    }
  | { kind: 'number'; name: string; label: string }
  | { kind: 'checkbox'; name: string; label: string; description?: string }

export interface MasterColumn<T> {
  header: string
  cell: (item: T) => ReactNode
  className?: string
  // ถ้ามี = หัวคอลัมน์นี้กดเรียงน้อย→มาก / มาก→น้อยได้ (ค่าตัวเลขที่ใช้เรียง)
  sortValue?: (item: T) => number
}

// method syntax (ไม่ใช่ property) โดยตั้งใจ — ให้ api ของแต่ละ master ที่รับ input ชนิดเฉพาะของตัวเอง
// กำหนดให้ช่องนี้ได้ (method parameter เป็น bivariant) โดยไม่ต้องใช้ any
export interface MasterApi<T> {
  list(): Promise<T[]>
  create(input: Record<string, unknown>): Promise<void>
  update(id: string, input: Record<string, unknown>): Promise<void>
  remove(id: string): Promise<void>
}

// config ของหน้าตั้งค่า master หนึ่งหน้า — MasterSettingsPage ทำทุกอย่างที่เหลือ
// (ตาราง, ค้นหา, ฟอร์มเพิ่ม/แก้ไข, ยืนยันลบ, toast, refetch)
export interface MasterConfig<T extends { id: string }> {
  // ส่วนหน้าของคีย์สิทธิ์ของหน้านี้ เช่น "prename" → prename.view/create/update/delete (ตรงกับ backend)
  resource: string
  title: string
  description: string
  // ชื่อสิ่งที่จัดการ ใช้ประกอบข้อความ เช่น "คำนำหน้า" → "เพิ่มคำนำหน้า", "ลบคำนำหน้าแล้ว"
  entityLabel: string
  api: MasterApi<T>
  columns: MasterColumn<T>[]
  fields: MasterField[]
  // ช่องของฟอร์มค้นหา (กรองฝั่ง client, ไม่สนตัวพิมพ์): name = key ของช่อง, value = ค่าของแถวที่ช่องนี้ใช้เทียบ
  // ช่องที่กรอกต้องตรงทุกช่อง (ดู components/layout/SearchForm.tsx)
  searchFields: {
    name: string
    label: string
    placeholder?: string
    value: (item: T) => string | null
  }[]
  // master ที่มี isActive: ใส่แล้วฟอร์มค้นหามีช่อง "สถานะ" ต่อท้าย (คำนำหน้าไม่มีสถานะตาม data dict จึงไม่ใส่)
  statusOf?: (item: T) => boolean
  // ข้อความที่ใช้เรียกแถวในกล่องยืนยันลบ
  itemLabel: (item: T) => string
  // ข้อความ error ที่ backend ส่งมา (field `error`) → ข้อความไทย เช่น
  // 'prename name already taken' → 'ชื่อคำนำหน้านี้มีอยู่แล้ว'
  errorMessages: Record<string, string>
  // อธิบายผลของการลบ (แสดงในกล่องยืนยัน) เช่น "ถ้ายังมีคนไข้ใช้อยู่ ระบบจะไม่ให้ลบ"
  deleteNote: string
}
