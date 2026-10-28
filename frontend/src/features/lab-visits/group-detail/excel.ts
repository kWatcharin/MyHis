import type { Alignment, Borders, Workbook, Worksheet } from 'exceljs'

import type { LabDocumentData, LabDocumentKind } from '../documents'
import {
  CD4_HEADER,
  LAB_DOCUMENTS,
  ROSTER_DATE_LINE,
  ROSTER_HEADER,
  cd4RowCells,
  cd4Subtitle,
  documentFileName,
  documentHeading,
  rosterRowCells,
} from '../documents'

// สร้างไฟล์ Excel (.xlsx) ของเอกสารนัดหมายแล็ปด้วย exceljs — หน้าตาตามกระดาษจริง (หัวเรื่อง, หัวตาราง 2 ชั้น,
// เส้นตารางทุกช่อง, A4 แนวตั้ง) exceljs ใหญ่ (~0.9MB) จึง import แบบ dynamic ตอนกดส่งออกเท่านั้น
// buildLabWorkbook รับ constructor ของ Workbook เข้ามา (ไม่ import เอง) เพื่อให้เทสแบบ headless ได้

type WorkbookConstructor = new () => Workbook

const FONT = 'Tahoma'
// รหัสขนาดกระดาษ A4 ของ Excel (PaperSize.A4 = 9 — เป็น const enum ใน d.ts ใช้ตรง ๆ ไม่ได้)
const PAPER_A4 = 9
const THIN_BORDER: Partial<Borders> = {
  top: { style: 'thin' },
  left: { style: 'thin' },
  bottom: { style: 'thin' },
  right: { style: 'thin' },
}
const CENTER: Partial<Alignment> = { horizontal: 'center', vertical: 'middle', wrapText: true }
const LEFT: Partial<Alignment> = { horizontal: 'left', vertical: 'middle', wrapText: true }

// หัวเรื่อง 2 บรรทัดบนสุด merge เต็มความกว้างตาราง
function addTitleRows(sheet: Worksheet, columnCount: number, heading: string, subtitle: string) {
  sheet.mergeCells(1, 1, 1, columnCount)
  const title = sheet.getCell(1, 1)
  title.value = heading
  title.font = { name: FONT, size: 14, bold: true }
  title.alignment = CENTER
  sheet.getRow(1).height = 24

  sheet.mergeCells(2, 1, 2, columnCount)
  const sub = sheet.getCell(2, 1)
  sub.value = subtitle
  sub.font = { name: FONT, size: 11 }
  sub.alignment = CENTER
  sheet.getRow(2).height = 20
}

function styleHeaderCell(sheet: Worksheet, row: number, column: number, value: string) {
  const cell = sheet.getCell(row, column)
  cell.value = value
  cell.font = { name: FONT, size: 10, bold: true }
  cell.alignment = CENTER
  cell.border = THIN_BORDER
}

// เติมแถวข้อมูลเริ่มที่ startRow — คอลัมน์ใน leftColumns ชิดซ้าย (ข้อความยาว) ที่เหลือจัดกลาง
// คอลัมน์ใน numberColumns เขียนเป็นตัวเลขจริง (Excel ไม่ขึ้นเตือน "ตัวเลขเก็บเป็นข้อความ") — HN/NAP NO คงเป็นข้อความ
function addDataRows(
  sheet: Worksheet,
  startRow: number,
  rows: string[][],
  layout: { leftColumns: number[]; numberColumns: number[]; height: number },
) {
  rows.forEach((cells, index) => {
    const rowNumber = startRow + index
    cells.forEach((value, columnIndex) => {
      const column = columnIndex + 1
      const cell = sheet.getCell(rowNumber, column)
      // ช่องว่าง = ไม่มีค่า (ไม่ใช่สตริงว่าง) แต่ยังมีเส้นตาราง
      cell.value =
        value === '' ? null : layout.numberColumns.includes(column) && /^\d+$/.test(value) ? Number(value) : value
      cell.font = { name: FONT, size: 10 }
      cell.alignment = layout.leftColumns.includes(column) ? LEFT : CENTER
      cell.border = THIN_BORDER
    })
    sheet.getRow(rowNumber).height = layout.height
  })
}

function setupPage(sheet: Worksheet, printTitlesRow: string) {
  sheet.pageSetup = {
    paperSize: PAPER_A4,
    orientation: 'portrait',
    fitToPage: true,
    fitToWidth: 1,
    fitToHeight: 0,
    horizontalCentered: true,
    printTitlesRow,
    margins: { left: 0.3, right: 0.3, top: 0.4, bottom: 0.4, header: 0.2, footer: 0.2 },
  }
}

function addCd4Sheet(workbook: Workbook, data: LabDocumentData) {
  const sheet = workbook.addWorksheet('ใบนัดตรวจ CD4-VL')
  // HN, ชื่อ, อายุ, ปี, เดือน, CD4 ครั้งที่ 1, ครั้งที่ 2, นัด VL, ผล VL, สูตรยา, โรคร่วม
  // (exceljs ไม่เขียนความกว้าง 9 เพราะเท่าค่าเริ่มต้นของมัน จึงใช้ 9.5)
  const widths = [10, 26, 6, 6, 6, 11, 11, 11, 9.5, 18, 14]
  widths.forEach((width, index) => {
    sheet.getColumn(index + 1).width = width
  })
  addTitleRows(sheet, widths.length, documentHeading(data), cd4Subtitle(data))

  // หัวตาราง 2 ชั้น (แถว 3–4): หัวที่มี sub merge แนวนอนในแถว 3, ที่ไม่มี merge แนวตั้ง 3–4
  let column = 1
  for (const header of CD4_HEADER) {
    if (header.sub) {
      const last = column + header.sub.length - 1
      sheet.mergeCells(3, column, 3, last)
      styleHeaderCell(sheet, 3, column, header.label)
      header.sub.forEach((label, offset) => styleHeaderCell(sheet, 4, column + offset, label))
      // เส้นขอบของช่องที่ถูก merge ทับ (ไม่งั้นขอบบนของหัวกลุ่มขาดบางโปรแกรม)
      for (let c = column + 1; c <= last; c += 1) sheet.getCell(3, c).border = THIN_BORDER
      column = last + 1
    } else {
      sheet.mergeCells(3, column, 4, column)
      styleHeaderCell(sheet, 3, column, header.label)
      sheet.getCell(4, column).border = THIN_BORDER
      column += 1
    }
  }
  sheet.getRow(3).height = 20
  sheet.getRow(4).height = 20

  // ชื่อ (2), สูตรยา (10), โรคร่วม (11) ชิดซ้าย; อายุ (3), ปี (4), เดือน (5) เป็นตัวเลข
  addDataRows(sheet, 5, data.cd4Rows.map(cd4RowCells), {
    leftColumns: [2, 10, 11],
    numberColumns: [3, 4, 5],
    height: 20,
  })
  setupPage(sheet, '3:4')
}

function addRosterSheet(workbook: Workbook, data: LabDocumentData) {
  const sheet = workbook.addWorksheet('ใบรายชื่อนัดรับยา')
  // HN, ชื่อ, NAP NO, น้ำหนัก, ความดัน, รายการแลป, หมายเหตุ
  const widths = [10, 26, 18, 9.5, 10, 24, 20]
  widths.forEach((width, index) => {
    sheet.getColumn(index + 1).width = width
  })
  addTitleRows(sheet, widths.length, documentHeading(data), ROSTER_DATE_LINE)

  ROSTER_HEADER.forEach((label, index) => styleHeaderCell(sheet, 3, index + 1, label))
  sheet.getRow(3).height = 20

  // แถวสูงกว่าใบนัดเล็กน้อย เผื่อเขียนด้วยมือ
  addDataRows(sheet, 4, data.rosterRows.map(rosterRowCells), {
    leftColumns: [2, 6, 7],
    numberColumns: [],
    height: 24,
  })
  setupPage(sheet, '3:3')
}

export function buildLabWorkbook(
  WorkbookClass: WorkbookConstructor,
  kind: LabDocumentKind,
  data: LabDocumentData,
): Workbook {
  const workbook = new WorkbookClass()
  workbook.creator = 'my-his'
  workbook.title = LAB_DOCUMENTS[kind].title
  if (kind === 'cd4') {
    addCd4Sheet(workbook, data)
  } else {
    addRosterSheet(workbook, data)
  }
  return workbook
}

const XLSX_MIME = 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'

// สร้างไฟล์แล้วให้เบราว์เซอร์ดาวน์โหลด (ไฟล์อยู่ในเครื่องผู้ใช้เท่านั้น ไม่ส่งขึ้น server)
export async function downloadLabExcel(kind: LabDocumentKind, data: LabDocumentData): Promise<void> {
  const exceljs = await import('exceljs')
  // build ของเบราว์เซอร์เป็น UMD — บาง bundler ให้ named export บางตัวให้แค่ default
  const WorkbookClass =
    (exceljs as unknown as { default?: { Workbook: WorkbookConstructor } }).default?.Workbook ??
    exceljs.Workbook
  const buffer = await buildLabWorkbook(WorkbookClass, kind, data).xlsx.writeBuffer()

  const url = URL.createObjectURL(new Blob([buffer], { type: XLSX_MIME }))
  const link = document.createElement('a')
  link.href = url
  link.download = `${documentFileName(kind, data)}.xlsx`
  document.body.append(link)
  link.click()
  link.remove()
  // ปล่อย URL หลังเบราว์เซอร์เริ่มดาวน์โหลดไปแล้ว
  window.setTimeout(() => URL.revokeObjectURL(url), 1000)
}
