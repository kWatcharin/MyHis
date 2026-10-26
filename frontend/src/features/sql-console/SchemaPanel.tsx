import { ChevronRight, Lock, PanelLeftClose, Table2 } from 'lucide-react'

import { ListState } from '@/components/layout/ListState'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'

import type { SchemaTable } from './sqlConsoleApi'

export type SchemaState =
  | { status: 'loading' }
  | { status: 'error'; message: string }
  | { status: 'ready'; tables: SchemaTable[] }

// id ของแผงและปุ่มซ่อน — ปุ่ม "แสดงตาราง" ในหน้าอ้าง aria-controls และย้ายโฟกัสกลับมาที่ปุ่มซ่อนตอนเปิด
export const SCHEMA_PANEL_ID = 'sql-schema-panel'
export const SCHEMA_HIDE_BUTTON_ID = 'sql-schema-hide'

interface SchemaPanelProps {
  state: SchemaState
  // แทรกชื่อตาราง/คอลัมน์ลงตำแหน่งเคอร์เซอร์ใน editor
  onPick: (text: string) => void
  // false = ซ่อน (ยังอยู่ใน DOM ตารางที่กางไว้จึงไม่พับเองตอนเปิดกลับ)
  open: boolean
  onHide: () => void
}

// รายการตาราง → คอลัมน์ ที่ role ของหน้า SQL อ่านได้ (คอลัมน์ลับไม่มีอยู่ในรายการ) กดชื่อเพื่อแทรกลง editor
export function SchemaPanel({ state, onPick, open, onHide }: SchemaPanelProps) {
  return (
    <Card
      id={SCHEMA_PANEL_ID}
      hidden={!open}
      className="gap-0 py-0 lg:sticky lg:top-4 lg:max-h-[calc(100svh-6rem)]"
    >
      <div className="flex items-center gap-2 border-b py-1.5 pr-1.5 pl-3">
        <h2 className="text-sm font-semibold">ตาราง</h2>
        {state.status === 'ready' && (
          <span className="text-xs text-muted-foreground tabular-nums">{state.tables.length}</span>
        )}
        <Button
          id={SCHEMA_HIDE_BUTTON_ID}
          variant="ghost"
          size="sm"
          className="ml-auto text-muted-foreground"
          onClick={onHide}
          aria-controls={SCHEMA_PANEL_ID}
          aria-expanded={open}
        >
          <PanelLeftClose />
          ซ่อน
        </Button>
      </div>
      <div className="max-h-72 overflow-auto py-1 lg:max-h-none lg:min-h-0 lg:flex-1">
        {state.status === 'loading' && <ListState kind="loading" />}
        {state.status === 'error' && <ListState kind="error">{state.message}</ListState>}
        {state.status === 'ready' && (
          <ul className="text-sm">
            {state.tables.map((table) => (
              <li key={table.name} className="relative">
                {/* ปุ่มแทรกวางทับแถวหัวตาราง แต่อยู่นอก <summary> (ปุ่มซ้อนในตัวกด กาง/พับ ใช้กับ screen reader ไม่ดี) */}
                <button
                  type="button"
                  onClick={() => onPick(table.name)}
                  className="absolute top-1 right-2 z-10 rounded px-1.5 py-0.5 text-xs text-muted-foreground hover:bg-accent hover:text-accent-foreground focus-visible:ring-2 focus-visible:ring-ring/50 focus-visible:outline-none"
                  aria-label={`แทรกชื่อตาราง ${table.name}`}
                >
                  แทรก
                </button>
                <details className="group/table">
                  <summary className="flex cursor-pointer list-none items-center gap-1.5 py-1.5 pr-14 pl-3 hover:bg-muted/60 [&::-webkit-details-marker]:hidden">
                    <ChevronRight
                      aria-hidden
                      className="size-3.5 shrink-0 text-muted-foreground transition-transform duration-150 group-open/table:rotate-90"
                    />
                    <Table2 aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
                    <span className="min-w-0 flex-1 truncate font-mono text-[13px]">{table.name}</span>
                  </summary>
                  <ul className="pb-1">
                    {table.columns.map((column) => (
                      <li key={column.name}>
                        <button
                          type="button"
                          onClick={() => onPick(column.name)}
                          className="flex w-full items-center gap-1.5 py-1 pr-3 pl-10 text-left hover:bg-muted/60 focus-visible:bg-muted/60 focus-visible:outline-none"
                          title={column.encrypted ? 'คอลัมน์เข้ารหัส' : undefined}
                        >
                          <span className="min-w-0 flex-1 truncate font-mono text-[12.5px]">{column.name}</span>
                          {column.encrypted && <Lock aria-label="เข้ารหัส" className="size-3 text-info-ink" />}
                          <span className="shrink-0 text-[11px] text-muted-foreground">{column.type}</span>
                        </button>
                      </li>
                    ))}
                  </ul>
                </details>
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className="border-t px-3 py-2.5 text-xs leading-relaxed text-muted-foreground">
        ค้นด้วย HN: <code className="font-mono text-foreground">where hn_idx = :hn('66471')</code>
        <br />
        เลขประจำตัว: <code className="font-mono text-foreground">:nap_no('...')</code> กับ nap_no_idx
      </div>
    </Card>
  )
}
