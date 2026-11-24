import { Lock, LockOpen, TriangleAlert } from 'lucide-react'

import { cn } from '@/lib/utils'

import type { Cell, QueryResult, ResultColumn } from './sqlConsoleApi'

// ชนิดตัวเลข — ชิดขวา เทียบหลักกันได้
const NUMERIC_TYPES = new Set(['INT2', 'INT4', 'INT8', 'FLOAT4', 'FLOAT8', 'NUMERIC', 'OID'])
// ข้อความยาวกว่านี้ตัดด้วย … (ดูเต็มได้ที่ tooltip ของช่อง)
const LONG_TEXT = 80

function cellText(cell: Cell): string {
  if (cell === null) return 'NULL'
  if (typeof cell === 'object') return JSON.stringify(cell)
  return String(cell)
}

function ColumnHeader({ column }: { column: ResultColumn }) {
  const note = column.decrypted
    ? { icon: LockOpen, className: 'text-info-ink', title: 'ถอดรหัสแล้ว' }
    : column.encrypted
      ? {
          icon: Lock,
          className: 'text-muted-foreground',
          title: 'ข้อมูลเข้ารหัส ติ๊ก "ถอดรหัสข้อมูลผู้ป่วย" เพื่อดูค่าจริง',
        }
      : !column.supported
        ? { icon: TriangleAlert, className: 'text-warning-ink', title: 'ชนิดนี้ยังแสดงไม่ได้ ใส่ ::text ต่อท้ายคอลัมน์' }
        : null
  const Icon = note?.icon
  return (
    <th
      scope="col"
      className="sticky top-0 z-10 border-b bg-muted px-3 py-2 text-left align-bottom font-semibold whitespace-nowrap"
    >
      <span className="flex items-center gap-1.5 text-xs text-foreground" title={note?.title}>
        {Icon && <Icon aria-hidden className={cn('size-3.5', note.className)} />}
        {column.name}
        {note && <span className="sr-only">({note.title})</span>}
      </span>
      <span className="block text-[11px] font-normal text-muted-foreground">{column.type.toLowerCase()}</span>
    </th>
  )
}

// ตารางผลลัพธ์: หัวตารางติดขอบบนขณะเลื่อน, เลขแถว, NULL สีจาง, ช่องที่ถอดรหัสมีพื้นสีอ่อน — ข้อมูลอยู่ใน DOM ของหน้านี้เท่านั้น
export function ResultGrid({ result }: { result: QueryResult }) {
  const numeric = result.columns.map((column) => NUMERIC_TYPES.has(column.type))
  return (
    <div className="max-h-[60vh] overflow-auto rounded-lg border">
      <table className="w-full border-separate border-spacing-0 font-mono text-[13px] tabular-nums">
        <thead>
          <tr>
            <th
              scope="col"
              className="sticky top-0 left-0 z-20 w-px border-b bg-muted px-3 py-2 text-right text-xs font-semibold text-muted-foreground"
            >
              #
            </th>
            {result.columns.map((column, index) => (
              <ColumnHeader key={`${index}-${column.name}`} column={column} />
            ))}
          </tr>
        </thead>
        <tbody>
          {result.rows.map((row, rowIndex) => (
            <tr key={rowIndex} className="hover:bg-muted/40">
              <td className="sticky left-0 border-b bg-card px-3 py-1.5 text-right text-xs text-muted-foreground">
                {rowIndex + 1}
              </td>
              {row.map((cell, index) => {
                const column = result.columns[index]
                const text = cellText(cell)
                return (
                  <td
                    key={index}
                    title={text.length > LONG_TEXT ? text : undefined}
                    className={cn(
                      'max-w-[28rem] truncate border-b px-3 py-1.5 whitespace-nowrap',
                      numeric[index] && 'text-right',
                      column?.decrypted && 'bg-primary/8',
                    )}
                  >
                    {cell === null ? <span className="text-muted-foreground italic">NULL</span> : text}
                  </td>
                )
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
