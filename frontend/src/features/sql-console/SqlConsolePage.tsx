import { CircleAlert, History, Play, ShieldAlert } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { ListState } from '@/components/layout/ListState'
import { PageHeader } from '@/components/layout/PageHeader'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Checkbox } from '@/components/ui/checkbox'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'

import { ResultGrid } from './ResultGrid'
import { SchemaPanel } from './SchemaPanel'
import type { SchemaState } from './SchemaPanel'
import { SqlEditor } from './SqlEditor'
import type { SqlEditorHandle } from './SqlEditor'
import type { BlockedStatement, QueryFailure, QueryResult } from './sqlConsoleApi'
import { blockedStatement, consoleErrorMessage, queryFailure, sqlConsoleApi } from './sqlConsoleApi'

type RunState =
  | { status: 'idle' }
  | { status: 'running' }
  | { status: 'done'; result: QueryResult }
  | { status: 'failed'; failure: QueryFailure }
  | { status: 'blocked'; blocked: BlockedStatement }
  | { status: 'error'; message: string }

// สีป้ายผลในประวัติ: สำเร็จ = เส้นขอบ, Postgres ปฏิเสธ = แดง, หน้า SQL ไม่รัน (คำสั่งแก้ข้อมูล/หลายคำสั่ง) = ส้ม
type HistoryTone = 'outline' | 'danger' | 'warning'

interface HistoryEntry {
  id: number
  sql: string
  at: Date
  outcome: string
  tone: HistoryTone
}

// คำสั่งที่หน้า SQL รันได้ (backend ตรวจคำแรก — sql_text::is_read_keyword)
const READ_COMMANDS = ['SELECT', 'WITH', 'VALUES', 'TABLE', 'EXPLAIN', 'SHOW']

// ประวัติในหน้านี้เก็บได้กี่รายการ (หน่วยความจำเท่านั้น — ปิดหน้าแล้วหาย)
const HISTORY_LIMIT = 15
// ค่าตั้งต้นก่อนโหลดขีดจำกัดจริงจาก backend
const DEFAULT_LIMITS = { maxRows: 500, statementTimeoutMs: 5000 }

const TIME_FORMAT = new Intl.DateTimeFormat('th-TH', {
  hour: '2-digit',
  minute: '2-digit',
  second: '2-digit',
})

// หน้า SQL (/data/script, สิทธิ์ sql.view): เขียน SQL พร้อม auto complete แล้วรันแบบอ่านอย่างเดียว ถอดรหัสคอลัมน์ผู้ป่วย
// ในผลลัพธ์ได้เมื่อมี patient.view และติ๊กเอง (ค่าเริ่มต้นไม่ถอด) — ผลลัพธ์/SQL/ประวัติอยู่ใน state ของหน้าเท่านั้น
// ไม่ลง localStorage, URL หรือ console ทุกการรันถูกบันทึกในประวัติการใช้งานฝั่ง backend
export function SqlConsolePage() {
  const { hasEffectivePermission } = useRolePreview()
  const canDecrypt = hasEffectivePermission('patient.view')
  const editor = useRef<SqlEditorHandle>(null)
  const running = useRef(false)
  const nextHistoryId = useRef(1)

  const [schemaState, setSchemaState] = useState<SchemaState>({ status: 'loading' })
  const [limits, setLimits] = useState(DEFAULT_LIMITS)
  const [decrypt, setDecrypt] = useState(false)
  const [run, setRun] = useState<RunState>({ status: 'idle' })
  const [history, setHistory] = useState<HistoryEntry[]>([])

  useEffect(() => {
    let cancelled = false
    sqlConsoleApi
      .schema()
      .then((schema) => {
        if (cancelled) return
        setSchemaState({ status: 'ready', tables: schema.tables })
        setLimits({ maxRows: schema.maxRows, statementTimeoutMs: schema.statementTimeoutMs })
      })
      .catch((err: unknown) => {
        if (!cancelled) setSchemaState({ status: 'error', message: consoleErrorMessage(err) })
      })
    return () => {
      cancelled = true
    }
  }, [])

  const completionSchema = useMemo(
    () =>
      schemaState.status === 'ready'
        ? Object.fromEntries(schemaState.tables.map((t) => [t.name, t.columns.map((c) => c.name)]))
        : {},
    [schemaState],
  )

  const remember = useCallback((sql: string, at: Date, outcome: string, tone: HistoryTone) => {
    const id = nextHistoryId.current
    nextHistoryId.current += 1
    setHistory((previous) => [{ id, sql, at, outcome, tone }, ...previous].slice(0, HISTORY_LIMIT))
  }, [])

  const handleRun = useCallback(async () => {
    if (running.current) return
    const target = editor.current?.runnable()
    if (!target || !target.text.trim()) {
      setRun({ status: 'error', message: 'พิมพ์คำสั่ง SQL ก่อน' })
      return
    }
    running.current = true
    setRun({ status: 'running' })
    const startedAt = new Date()
    try {
      const result = await sqlConsoleApi.run(target.text, canDecrypt && decrypt)
      setRun({ status: 'done', result })
      remember(target.text, startedAt, `${result.rowCount.toLocaleString('th-TH')} แถว`, 'outline')
    } catch (err) {
      const blocked = blockedStatement(err)
      const failure = queryFailure(err)
      if (blocked) {
        setRun({ status: 'blocked', blocked })
        const outcome =
          blocked.reason === 'multiple_statements'
            ? 'หลายคำสั่ง'
            : `ไม่อนุญาต${blocked.keyword ? ` (${blocked.keyword})` : ''}`
        remember(target.text, startedAt, outcome, 'warning')
      } else if (failure) {
        setRun({ status: 'failed', failure })
        remember(target.text, startedAt, failure.code ? `ล้มเหลว (${failure.code})` : 'ล้มเหลว', 'danger')
        // ชี้ตำแหน่งที่ Postgres บอก (นับจากต้นข้อความที่รัน — รันเฉพาะส่วนที่เลือกก็บวกจุดเริ่มของส่วนนั้น)
        if (failure.position !== null) editor.current?.focusAt(target.offset + failure.position - 1)
      } else {
        setRun({ status: 'error', message: consoleErrorMessage(err) })
      }
    } finally {
      running.current = false
    }
  }, [canDecrypt, decrypt, remember])

  const timeoutSeconds = limits.statementTimeoutMs / 1000

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        fallbackTitle="SQL"
        description="รันคำสั่ง SQL แบบอ่านอย่างเดียว — ทุกครั้งถูกบันทึกในประวัติการใช้งาน"
      />

      <div className="grid items-start gap-4 lg:grid-cols-[16rem_minmax(0,1fr)]">
        <SchemaPanel state={schemaState} onPick={(text) => editor.current?.insert(text)} />

        <div className="flex min-w-0 flex-col gap-4">
          <Card className="gap-0 py-0">
            <div className="border-b">
              <SqlEditor
                ref={editor}
                initialValue=""
                schema={completionSchema}
                onRun={() => void handleRun()}
                ariaLabel="คำสั่ง SQL"
              />
            </div>
            <div className="flex flex-wrap items-center gap-x-4 gap-y-2 px-3 py-2.5">
              <Button onClick={() => void handleRun()} disabled={run.status === 'running'}>
                <Play />
                {run.status === 'running' ? 'กำลังรัน...' : 'รัน'}
              </Button>
              <span className="hidden text-xs text-muted-foreground sm:inline">
                <kbd className="rounded border bg-muted px-1.5 py-0.5 font-mono text-[11px]">Ctrl</kbd> +{' '}
                <kbd className="rounded border bg-muted px-1.5 py-0.5 font-mono text-[11px]">Enter</kbd>{' '}
                เลือกข้อความไว้ = รันเฉพาะส่วนนั้น
              </span>
              {canDecrypt && (
                <label className="flex items-center gap-2 text-sm">
                  <Checkbox checked={decrypt} onCheckedChange={(checked) => setDecrypt(checked === true)} />
                  ถอดรหัสข้อมูลผู้ป่วย
                </label>
              )}
              <p className="text-xs text-muted-foreground tabular-nums sm:ml-auto">
                อ่านอย่างเดียว · สูงสุด {limits.maxRows.toLocaleString('th-TH')} แถว · {timeoutSeconds} วินาที
              </p>
            </div>
          </Card>

          <ResultPanel run={run} maxRows={limits.maxRows} />

          {history.length > 0 && (
            <Card className="gap-0 py-0">
              <h2 className="flex items-center gap-2 border-b px-3 py-2.5 text-sm font-semibold">
                <History aria-hidden className="size-4 text-muted-foreground" />
                ประวัติในหน้านี้
                <span className="text-xs font-normal text-muted-foreground">(กดเพื่อนำกลับมาแก้ ปิดหน้าแล้วหาย)</span>
              </h2>
              <ul className="divide-y">
                {history.map((entry) => (
                  <li key={entry.id}>
                    <button
                      type="button"
                      onClick={() => editor.current?.setText(entry.sql)}
                      className="flex w-full items-center gap-3 px-3 py-2 text-left hover:bg-muted/60 focus-visible:bg-muted/60 focus-visible:outline-none"
                    >
                      <span className="shrink-0 text-xs text-muted-foreground tabular-nums">
                        {TIME_FORMAT.format(entry.at)}
                      </span>
                      <span className="min-w-0 flex-1 truncate font-mono text-[12.5px]">
                        {entry.sql.replace(/\s+/g, ' ')}
                      </span>
                      <Badge variant={entry.tone}>{entry.outcome}</Badge>
                    </button>
                  </li>
                ))}
              </ul>
            </Card>
          )}
        </div>
      </div>
    </div>
  )
}

function ResultPanel({ run, maxRows }: { run: RunState; maxRows: number }) {
  if (run.status === 'idle') {
    return (
      <Card className="py-0">
        <ListState kind="empty">ยังไม่ได้รันคำสั่ง — เขียน SQL แล้วกด &quot;รัน&quot;</ListState>
      </Card>
    )
  }
  if (run.status === 'running') {
    return (
      <Card className="py-0">
        <ListState kind="loading" />
      </Card>
    )
  }
  if (run.status === 'error') {
    return (
      <Card className="py-0">
        <ListState kind="error">{run.message}</ListState>
      </Card>
    )
  }
  if (run.status === 'blocked') {
    return <BlockedNotice blocked={run.blocked} />
  }
  if (run.status === 'failed') {
    const { failure } = run
    return (
      <Card role="alert" className="gap-2 px-4 py-3 ring-danger/40">
        <p className="flex items-center gap-2 text-sm font-semibold text-danger-ink">
          <CircleAlert aria-hidden className="size-4" />
          คำสั่งไม่สำเร็จ
          {failure.code && <Badge variant="danger">{failure.code}</Badge>}
        </p>
        <pre className="font-mono text-[13px] leading-relaxed whitespace-pre-wrap text-foreground">
          {failure.message}
        </pre>
      </Card>
    )
  }

  const { result } = run
  return (
    <div className="flex flex-col gap-2">
      <p className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted-foreground tabular-nums">
        <span className="font-medium text-foreground">{result.rowCount.toLocaleString('th-TH')} แถว</span>
        <span>{result.durationMs.toLocaleString('th-TH')} ms</span>
        {result.truncated && (
          <span className="text-warning-ink">
            แสดงเฉพาะ {maxRows.toLocaleString('th-TH')} แถวแรก — เพิ่มเงื่อนไข where ให้แคบลง
          </span>
        )}
        {result.decryptErrors > 0 && (
          <span className="text-danger-ink">ถอดรหัสไม่ได้ {result.decryptErrors} ช่อง (แสดงเป็น NULL)</span>
        )}
      </p>
      {result.columns.length === 0 ? (
        <Card className="py-0">
          <ListState kind="empty">คำสั่งนี้ไม่คืนคอลัมน์</ListState>
        </Card>
      ) : (
        <ResultGrid result={result} />
      )}
    </div>
  )
}

// คำเตือนเมื่อหน้า SQL ไม่รันคำสั่ง — บอกว่าทำไม ไม่มีอะไรถูกเปลี่ยน และต้องทำอย่างไรต่อ (แทน syntax error ของ Postgres)
function BlockedNotice({ blocked }: { blocked: BlockedStatement }) {
  const severalStatements = blocked.reason === 'multiple_statements'
  return (
    <Card role="alert" className="gap-2 px-4 py-3 ring-warning/50">
      <p className="flex flex-wrap items-center gap-2 text-sm font-semibold text-warning-ink">
        <ShieldAlert aria-hidden className="size-4" />
        {severalStatements ? 'รันได้ทีละคำสั่ง' : 'หน้านี้อ่านข้อมูลได้อย่างเดียว'}
        {blocked.keyword && <Badge variant="warning">{blocked.keyword}</Badge>}
      </p>
      {severalStatements ? (
        <p className="text-sm">
          พบเครื่องหมาย <Kbd>;</Kbd> คั่นมากกว่าหนึ่งคำสั่ง จึงยังไม่ได้รัน — ลบคำสั่งอื่นออก
          หรือลากเลือกเฉพาะคำสั่งที่ต้องการแล้วกด &quot;รัน&quot;
        </p>
      ) : (
        <>
          <p className="text-sm">
            {blocked.keyword
              ? `คำสั่ง ${blocked.keyword} ไม่ใช่การอ่านข้อมูล จึงไม่ได้รัน — ไม่มีข้อมูลใดถูกเปลี่ยน`
              : 'คำสั่งนี้มีส่วนที่แก้ข้อมูล (เช่น INSERT / UPDATE / DELETE ใน WITH) จึงไม่ได้รัน — ไม่มีข้อมูลใดถูกเปลี่ยน'}
          </p>
          <p className="flex flex-wrap items-center gap-1.5 text-sm text-muted-foreground">
            ใช้ได้เฉพาะ
            {READ_COMMANDS.map((command) => (
              <Kbd key={command}>{command}</Kbd>
            ))}
            — เพิ่ม แก้ไข หรือลบข้อมูล ให้ทำที่หน้าของข้อมูลนั้น
          </p>
        </>
      )}
    </Card>
  )
}

function Kbd({ children }: { children: string }) {
  return <kbd className="rounded border bg-muted px-1.5 py-0.5 font-mono text-[11px] text-foreground">{children}</kbd>
}
