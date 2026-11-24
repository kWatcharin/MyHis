import type { ReactNode } from 'react'

import { MedicalBackdrop } from './MedicalBackdrop'

// โครงหน้า "ทางเข้า" (login, register) แบ่งสองฝั่ง — แนวทางทั้งหมดอยู่ใน docs/ui-3d-backdrop.md
// - จอกว้าง (lg+): ซ้าย = แผงเกลียว DNA เต็มความสูง + ชื่อ MyHis ตัวใหญ่ชิดซ้ายล่าง, ขวา = เนื้อหา (การ์ดฟอร์ม) กลางแนวตั้ง
// - จอเล็ก: แผงเกลียวเป็นแถบด้านบน (~30% ของจอ) ฟอร์มอยู่ข้างล่าง — นิ้วโป้งเอื้อมถึงช่องกรอกและปุ่มได้ด้วยมือเดียว
// เกลียวอยู่ในแผงของตัวเอง ไม่มีการ์ดบัง; ไม่มี WebGL/โหลดไม่สำเร็จ = แผงพื้นเรียบสี sidebar ของธีม (ไม่ต้องมี gradient สำรอง)
// ชื่อ/คำโปรยอยู่มุมซ้ายล่าง ซึ่งเกลียวที่พาดจากซ้ายบนไปขวาล่างไม่ผ่าน จึงอ่านชัดเสมอ
export function EntryLayout({ children }: { children: ReactNode }) {
  return (
    <div className="grid min-h-svh grid-rows-[auto_1fr] bg-background lg:grid-cols-[1.15fr_1fr] lg:grid-rows-1">
      <section
        aria-label="my-his"
        className="relative isolate flex h-[30svh] min-h-40 flex-col justify-end overflow-hidden border-b bg-sidebar p-5 lg:h-auto lg:border-r lg:border-b-0 lg:p-12"
      >
        <MedicalBackdrop />
        <div className="pointer-events-none relative z-10 flex flex-col gap-1">
          <p className="text-3xl font-semibold tracking-tight text-sidebar-foreground lg:text-5xl">MyHis</p>
          <p className="text-sm text-muted-foreground lg:text-base">เครื่องมือช่วยงานพยาบาล</p>
        </div>
      </section>
      <main className="flex items-start justify-center px-4 py-6 lg:items-center lg:p-10">{children}</main>
    </div>
  )
}
