import { Printer, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useParams, useSearchParams } from 'react-router'

import { Button } from '@/components/ui/button'
import { patientsApi } from '@/features/patients/patientsApi'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'
import { todayIso } from '@/lib/date'
import { isUuid } from '@/lib/uuid'

import type { Cd4Row, LabDocumentData, LabDocumentKind, RosterRow } from '../documents'
import {
  CD4_HEADER,
  ROSTER_DATE_LINE,
  ROSTER_HEADER,
  buildLabDocumentData,
  cd4RowCells,
  cd4Subtitle,
  documentFileName,
  documentHeading,
  rosterRowCells,
} from '../documents'
import { labVisitsApi } from '../labVisitsApi'

// CSS เฉพาะหน้าพิมพ์: กระดาษ A4 แนวตั้ง, ตารางเส้นดำ, หัวตารางซ้ำทุกหน้า, ไม่ตัดแถวกลางหน้า
// ฟอนต์ไทยที่มีในเครื่องส่วนใหญ่ (เบราว์เซอร์จัดสระ/วรรณยุกต์ให้เอง ไม่ต้องฝังฟอนต์)
const PRINT_CSS = `
@page { size: A4 portrait; margin: 10mm; }
@media print { html, body { background: #fff !important; } }
.lab-print { font-family: "Sarabun", "TH Sarabun New", "Leelawadee UI", "Tahoma", sans-serif; }
.lab-print table { border-collapse: collapse; width: 100%; table-layout: fixed; font-size: 12px; }
.lab-print th, .lab-print td { border: 1px solid #000; padding: 3px 4px; vertical-align: middle; overflow-wrap: anywhere; }
.lab-print th { font-weight: 700; text-align: center; }
.lab-print thead { display: table-header-group; }
.lab-print tr { break-inside: avoid; }
.lab-print .roster td { height: 26px; }
`

// คอลัมน์ข้อความยาวชิดซ้าย (ตำแหน่งตาม cd4RowCells / rosterRowCells) ที่เหลือจัดกลาง
const CD4_LEFT = new Set([1, 9, 10])
const ROSTER_LEFT = new Set([1, 5, 6])
const CD4_WIDTHS = ['8%', '22%', '5%', '5%', '5%', '9%', '9%', '9%', '7%', '12%', '9%']
const ROSTER_WIDTHS = ['10%', '26%', '16%', '9%', '10%', '17%', '12%']

function Cd4Table({ rows }: { rows: Cd4Row[] }) {
  return (
    <table>
      <colgroup>
        {CD4_WIDTHS.map((width, index) => (
          <col key={index} style={{ width }} />
        ))}
      </colgroup>
      <thead>
        <tr>
          {CD4_HEADER.map((header) =>
            header.sub ? (
              <th key={header.label} colSpan={header.sub.length}>
                {header.label}
              </th>
            ) : (
              <th key={header.label} rowSpan={2}>
                {header.label}
              </th>
            ),
          )}
        </tr>
        <tr>
          {CD4_HEADER.flatMap((header) =>
            (header.sub ?? []).map((label) => <th key={`${header.label}-${label}`}>{label}</th>),
          )}
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.id}>
            {cd4RowCells(row).map((cell, index) => (
              <td key={index} style={{ textAlign: CD4_LEFT.has(index) ? 'left' : 'center' }}>
                {cell}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  )
}

function RosterTable({ rows }: { rows: RosterRow[] }) {
  return (
    <table className="roster">
      <colgroup>
        {ROSTER_WIDTHS.map((width, index) => (
          <col key={index} style={{ width }} />
        ))}
      </colgroup>
      <thead>
        <tr>
          {ROSTER_HEADER.map((label) => (
            <th key={label}>{label}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((row) => (
          <tr key={row.id}>
            {rosterRowCells(row).map((cell, index) => (
              <td key={index} style={{ textAlign: ROSTER_LEFT.has(index) ? 'left' : 'center' }}>
                {cell}
              </td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  )
}

type PrintState = { status: 'loading' } | { status: 'error' } | { status: 'ready'; data: LabDocumentData }

// หน้าเอกสารพร้อมพิมพ์ (ใช้ส่งออก PDF): /lab/appointments/:id/print?doc=cd4|roster — อยู่นอกโครงหน้าหลัก
// (ไม่มี sidebar) แต่ยังต้อง login + lab_visit.export (route ใน App.tsx) และ patient.view (ชื่อ/HN จากทะเบียน)
// เปิดในแท็บใหม่แล้วเปิดหน้าต่างพิมพ์ให้อัตโนมัติหนึ่งครั้ง ผู้ใช้เลือก "บันทึกเป็น PDF" เอง — URL มีแค่ uuid ของ
// นัดหมาย ข้อมูลคนไข้โหลดจาก API อยู่ในหน้าเท่านั้น
export function LabVisitPrintPage() {
  const { id = '' } = useParams()
  const [searchParams] = useSearchParams()
  const kind: LabDocumentKind = searchParams.get('doc') === 'roster' ? 'roster' : 'cd4'
  const [state, setState] = useState<PrintState>({ status: 'loading' })
  const printedRef = useRef(false)
  const { hasEffectivePermission } = useRolePreview()
  const canViewPatients = hasEffectivePermission('patient.view')
  // id จาก URL ที่ไม่ใช่ uuid (ลิงก์ที่ถูกแก้ให้มี ../ หรือ ?) ไม่ยิง API เลย — effect ข้างล่างยิงทันทีที่เปิดหน้า ไม่ต้องรอผู้ใช้กด
  const validId = isUuid(id)

  // โหลดข้อมูล + บันทึกการส่งออก PDF (ประวัติการใช้งาน, backend ตรวจ lab_visit.export) พร้อมกัน — บันทึกไม่ได้ =
  // ไม่แสดงเอกสาร เปิดหน้านี้ซ้ำ = ส่งออกอีกครั้ง
  useEffect(() => {
    if (!canViewPatients || !validId) return
    let cancelled = false
    Promise.all([labVisitsApi.get(id), patientsApi.list(), labVisitsApi.recordExport(id, kind, 'pdf')])
      .then(([group, patients]) => {
        if (!cancelled) {
          setState({ status: 'ready', data: buildLabDocumentData(group, patients, todayIso()) })
        }
      })
      .catch(() => {
        if (!cancelled) setState({ status: 'error' })
      })
    return () => {
      cancelled = true
    }
  }, [id, kind, canViewPatients, validId])

  const data = state.status === 'ready' ? state.data : null

  // ชื่อแท็บ = ชื่อไฟล์ตั้งต้นตอนบันทึกเป็น PDF (ชนิดเอกสาร + กลุ่ม + ปี — ไม่มีข้อมูลคนไข้)
  useEffect(() => {
    if (!data) return
    const previous = document.title
    document.title = documentFileName(kind, data)
    return () => {
      document.title = previous
    }
  }, [data, kind])

  // เปิดหน้าต่างพิมพ์ให้ครั้งเดียวหลังเอกสารแสดงแล้ว (ตั้ง flag ใน timer เพื่อให้ StrictMode ที่รัน effect ซ้ำยังพิมพ์ได้)
  useEffect(() => {
    if (!data || printedRef.current) return
    const timer = window.setTimeout(() => {
      printedRef.current = true
      window.print()
    }, 500)
    return () => window.clearTimeout(timer)
  }, [data])

  return (
    <div className="lab-print min-h-screen bg-white text-black [color-scheme:light]">
      <style>{PRINT_CSS}</style>
      <div className="sticky top-0 z-10 flex flex-wrap items-center gap-2 border-b border-neutral-300 bg-white px-4 py-3 print:hidden">
        <Button onClick={() => window.print()} disabled={!data}>
          <Printer />
          พิมพ์ / บันทึกเป็น PDF
        </Button>
        <Button variant="outline" onClick={() => window.close()}>
          <X />
          ปิดแท็บนี้
        </Button>
        <p className="text-sm text-neutral-600">
          ในหน้าต่างพิมพ์ เลือกปลายทาง “บันทึกเป็น PDF” (Save as PDF) และกระดาษ A4
        </p>
      </div>

      {!canViewPatients && (
        <p className="p-6 text-sm text-neutral-600">
          เอกสารมีชื่อ/HN จากทะเบียนผู้ป่วย ต้องมีสิทธิ์ &quot;ดู&quot; ผู้ป่วยด้วยถึงจะพิมพ์ได้
        </p>
      )}
      {canViewPatients && !validId && (
        <p className="p-6 text-sm text-neutral-600">ไม่พบนัดหมายนี้ (ลิงก์ไม่ถูกต้อง) ปิดแท็บแล้วเปิดจากหน้านัดหมายใหม่</p>
      )}
      {canViewPatients && validId && state.status === 'loading' && (
        <p className="p-6 text-sm text-neutral-600">กำลังเตรียมเอกสาร...</p>
      )}
      {validId && state.status === 'error' && (
        <p className="p-6 text-sm text-red-700">โหลดข้อมูลหรือบันทึกการส่งออกไม่สำเร็จ (นัดหมายอาจถูกลบไปแล้ว หรือไม่มีสิทธิ์ส่งออก) ปิดแท็บแล้วลองใหม่</p>
      )}
      {data && (
        <main className="mx-auto w-full max-w-[210mm] px-4 py-6 print:max-w-none print:p-0">
          <header className="mb-3 text-center">
            <h1 className="text-lg font-bold">{documentHeading(data)}</h1>
            <p className="mt-1 text-sm whitespace-pre-wrap">
              {kind === 'cd4' ? cd4Subtitle(data) : ROSTER_DATE_LINE}
            </p>
          </header>
          {kind === 'cd4' ? <Cd4Table rows={data.cd4Rows} /> : <RosterTable rows={data.rosterRows} />}
        </main>
      )}
    </div>
  )
}
