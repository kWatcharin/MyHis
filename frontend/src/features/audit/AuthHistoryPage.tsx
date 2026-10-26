import { Search } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'

import { ListState } from '@/components/layout/ListState'
import { ListToolbar } from '@/components/layout/ListToolbar'
import { PageHeader } from '@/components/layout/PageHeader'
import { TablePagination } from '@/components/layout/TablePagination'
import { usePagination } from '@/components/layout/usePagination'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Field, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { ThaiDateInput } from '@/components/ui/thai-date-input'
import { NativeSelect } from '@/components/ui/native-select'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { formatThaiDateTime } from '@/lib/date'
import { textIncludes } from '@/lib/search'

import type { AuthEvent, AuthEventKind, AuthEventsResult } from './auditApi'
import { auditApi } from './auditApi'
import { defaultRange, rangeError } from './dateRange'
import { describeUserAgent } from './userAgent'

const EVENT_LABELS: Record<AuthEventKind, string> = {
  login_succeeded: 'เข้าสู่ระบบ',
  login_failed: 'เข้าสู่ระบบไม่สำเร็จ',
  logout: 'ออกจากระบบ',
  logout_all: 'ออกจากทุกอุปกรณ์',
}

// เข้าสำเร็จ = เขียว, ล้มเหลว = แดง (ความหมายสีเดียวกับป้าย/ปุ่มอื่นทั้งระบบ) ออกจากระบบ = ป้ายเส้น
const EVENT_BADGE: Record<AuthEventKind, 'success' | 'danger' | 'outline'> = {
  login_succeeded: 'success',
  login_failed: 'danger',
  logout: 'outline',
  logout_all: 'outline',
}

interface Filters {
  from: string
  to: string
  user: string
  ip: string
  // '' = ทุกเหตุการณ์, logout รวม logout_all
  event: '' | 'login_succeeded' | 'login_failed' | 'logout'
}

function defaultFilters(): Filters {
  return { ...defaultRange(), user: '', ip: '', event: '' }
}

function matches(event: AuthEvent, filters: Filters): boolean {
  const userText = event.user ? `${event.user.displayName} ${event.user.username}` : 'ไม่พบชื่อผู้ใช้'
  const kind = event.event === 'logout_all' ? 'logout' : event.event
  return (
    textIncludes(userText, filters.user) &&
    textIncludes(event.ip ?? '', filters.ip) &&
    (filters.event === '' || kind === filters.event)
  )
}

// หน้า "ประวัติการเข้าออก" (เพจย่อยของ "ประวัติ", สิทธิ์ audit.view): login/logout ของผู้ใช้ทุกคน พร้อม IP และอุปกรณ์
// จาก request header — เลือกช่วงวัน (โหลดใหม่จาก backend) แล้วกรองผู้ใช้/IP/เหตุการณ์ฝั่ง client
// login ด้วยชื่อที่ไม่มีในระบบแสดงแค่ "ไม่พบชื่อผู้ใช้" (backend ไม่ส่งชื่อที่พิมพ์มา อาจเป็นรหัสผ่านพิมพ์ผิดช่อง)
export function AuthHistoryPage() {
  const [draft, setDraft] = useState<Filters>(defaultFilters)
  const [applied, setApplied] = useState<Filters>(draft)
  // เพิ่มทุกครั้งที่กดค้นหา — โหลดใหม่แม้ช่วงวันเดิม (ดูเหตุการณ์ล่าสุด)
  const [reloadKey, setReloadKey] = useState(0)
  const [result, setResult] = useState<AuthEventsResult | null>(null)
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading')
  const [formError, setFormError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    auditApi
      .authEvents(applied.from, applied.to)
      .then((data) => {
        if (cancelled) return
        setResult(data)
        setStatus('ready')
      })
      .catch(() => {
        if (!cancelled) setStatus('error')
      })
    return () => {
      cancelled = true
    }
  }, [applied.from, applied.to, reloadKey])

  const visible = (result?.events ?? []).filter((event) => matches(event, applied))
  const pagination = usePagination(visible)

  const apply = (next: Filters) => {
    const error = rangeError(next.from, next.to)
    setFormError(error)
    if (error) return
    setStatus('loading')
    setApplied(next)
    setReloadKey((key) => key + 1)
    pagination.reset()
  }

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault()
    apply(draft)
  }

  const handleClear = () => {
    const cleared = defaultFilters()
    setDraft(cleared)
    apply(cleared)
  }

  const set = (field: keyof Filters) => (value: string) => setDraft({ ...draft, [field]: value })

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        fallbackTitle="ประวัติการเข้าออก"
        description="การเข้า/ออกระบบของผู้ใช้ทุกคน พร้อม IP และอุปกรณ์ — วันที่และเวลาตามเวลาไทย"
      />

      <Card>
        <CardContent>
          <form onSubmit={handleSubmit} aria-label="ค้นหาประวัติการเข้าออก" className="flex flex-col gap-4">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-5">
              <Field>
                <FieldLabel htmlFor="auth-history-from">จากวันที่</FieldLabel>
                <ThaiDateInput id="auth-history-from" value={draft.from} onChange={set('from')} />
              </Field>
              <Field>
                <FieldLabel htmlFor="auth-history-to">ถึงวันที่</FieldLabel>
                <ThaiDateInput id="auth-history-to" value={draft.to} onChange={set('to')} />
              </Field>
              <Field>
                <FieldLabel htmlFor="auth-history-user">ผู้ใช้</FieldLabel>
                <Input
                  id="auth-history-user"
                  value={draft.user}
                  placeholder="ชื่อ หรือ username"
                  onChange={(e) => set('user')(e.target.value)}
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="auth-history-ip">IP</FieldLabel>
                <Input
                  id="auth-history-ip"
                  value={draft.ip}
                  className="font-mono"
                  onChange={(e) => set('ip')(e.target.value)}
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="auth-history-event">เหตุการณ์</FieldLabel>
                <NativeSelect
                  id="auth-history-event"
                  value={draft.event}
                  onChange={(e) => set('event')(e.target.value)}
                >
                  <option value="">ทั้งหมด</option>
                  <option value="login_succeeded">{EVENT_LABELS.login_succeeded}</option>
                  <option value="login_failed">{EVENT_LABELS.login_failed}</option>
                  <option value="logout">{EVENT_LABELS.logout}</option>
                </NativeSelect>
              </Field>
            </div>
            {formError && <p className="text-center text-sm text-destructive">{formError}</p>}
            <div className="flex justify-center gap-2">
              <Button type="submit">
                <Search />
                ค้นหา
              </Button>
              <Button type="button" variant="outline" onClick={handleClear}>
                ล้างข้อมูล
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>

      <ListToolbar
        ready={status === 'ready'}
        total={result?.events.length ?? 0}
        shown={visible.length}
        filtered={applied.user.trim() !== '' || applied.ip.trim() !== '' || applied.event !== ''}
      />

      {status === 'ready' && result?.truncated && (
        <p className="text-sm text-warning-ink">
          ช่วงวันที่นี้มีเกิน 5,000 รายการ แสดงเฉพาะส่วนที่ใหม่สุด — เลือกช่วงวันที่ให้แคบลง
        </p>
      )}

      <Card>
        <CardContent className="flex flex-col gap-3 px-0 sm:px-2">
          {status === 'loading' && <ListState kind="loading" />}
          {status === 'error' && (
            <ListState kind="error">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</ListState>
          )}
          {status === 'ready' && visible.length === 0 && (
            <ListState kind="noMatch">ไม่มีประวัติในช่วงนี้ที่ตรงกับเงื่อนไข</ListState>
          )}
          {status === 'ready' && visible.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>เวลา</TableHead>
                  <TableHead>ผู้ใช้</TableHead>
                  <TableHead>เหตุการณ์</TableHead>
                  <TableHead>IP</TableHead>
                  <TableHead>อุปกรณ์</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((event, index) => (
                  <TableRow key={`${event.occurredAt}-${event.event}-${index}`}>
                    <TableCell className="whitespace-nowrap">{formatThaiDateTime(event.occurredAt)}</TableCell>
                    <TableCell>
                      {event.user ? (
                        <>
                          <div className="font-medium">{event.user.displayName}</div>
                          <div className="font-mono text-xs text-muted-foreground">{event.user.username}</div>
                        </>
                      ) : (
                        <span className="text-muted-foreground">ไม่พบชื่อผู้ใช้</span>
                      )}
                    </TableCell>
                    <TableCell>
                      <Badge variant={EVENT_BADGE[event.event]}>{EVENT_LABELS[event.event]}</Badge>
                    </TableCell>
                    <TableCell className="font-mono text-sm">{event.ip ?? '—'}</TableCell>
                    <TableCell className="text-muted-foreground" title={event.userAgent ?? undefined}>
                      {describeUserAgent(event.userAgent)}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          {status === 'ready' && visible.length > 0 && <TablePagination {...pagination.props} />}
        </CardContent>
      </Card>
    </div>
  )
}
