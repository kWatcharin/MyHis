import { Eye, Search } from 'lucide-react'
import { useEffect, useState } from 'react'
import type { FormEvent } from 'react'

import { ListToolbar } from '@/components/layout/ListToolbar'
import { PageHeader } from '@/components/layout/PageHeader'
import { TablePagination } from '@/components/layout/TablePagination'
import { usePagination } from '@/components/layout/usePagination'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Field, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { NativeSelect } from '@/components/ui/native-select'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { useAuth } from '@/features/auth/AuthContext'
import { formatThaiDateTime } from '@/lib/date'
import { textIncludes } from '@/lib/search'

import { ActivityDetailDialog } from './ActivityDetailDialog'
import type { ActivityKind } from './activities'
import { activityPage, describeAction, describeTarget, KIND_LABELS, pageLabel, pageOptions } from './activities'
import type { ActivitiesResult, Activity } from './auditApi'
import { auditApi } from './auditApi'
import { defaultRange, rangeError } from './dateRange'

const KIND_BADGE: Record<ActivityKind, 'default' | 'secondary' | 'destructive' | 'outline'> = {
  create: 'secondary',
  update: 'outline',
  delete: 'destructive',
  export: 'default',
  other: 'outline',
}

interface Filters {
  from: string
  to: string
  user: string
  // '' = ทุกหน้า
  page: string
  // '' = ทุกการกระทำ
  kind: '' | ActivityKind
}

function defaultFilters(): Filters {
  return { ...defaultRange(), user: '', page: '', kind: '' }
}

function matches(activity: Activity, filters: Filters): boolean {
  const userText = activity.user
    ? `${activity.user.displayName} ${activity.user.username}`
    : 'ไม่ทราบผู้ใช้'
  return (
    textIncludes(userText, filters.user) &&
    (filters.page === '' || activityPage(activity) === filters.page) &&
    (filters.kind === '' || describeAction(activity.action).kind === filters.kind)
  )
}

// หน้า "ประวัติการใช้งาน" (เพจย่อยของ "ประวัติ", สิทธิ์ audit.view): ใครเพิ่ม/แก้ไข/ลบ/ส่งออกอะไรในหน้าไหน เมื่อไร
// เลือกช่วงวัน (โหลดใหม่จาก backend) แล้วกรองผู้ใช้/หน้า/การกระทำฝั่ง client — แถวผู้ป่วยแสดงแค่ "ผู้ป่วย 1 ราย"
// (backend ไม่ส่งตัวตนผู้ป่วยในประวัติ) ไม่มี IP/อุปกรณ์ (ดูได้ที่ประวัติการเข้าออก)
export function ActivityHistoryPage() {
  const { menu } = useAuth()
  const [draft, setDraft] = useState<Filters>(defaultFilters)
  const [applied, setApplied] = useState<Filters>(draft)
  // เพิ่มทุกครั้งที่กดค้นหา — โหลดใหม่แม้ช่วงวันเดิม (ดูรายการล่าสุด)
  const [reloadKey, setReloadKey] = useState(0)
  const [result, setResult] = useState<ActivitiesResult | null>(null)
  const [status, setStatus] = useState<'loading' | 'ready' | 'error'>('loading')
  const [formError, setFormError] = useState<string | null>(null)
  // แถวที่เปิดดูรายละเอียด (modal) — null = ปิด
  const [viewing, setViewing] = useState<Activity | null>(null)

  useEffect(() => {
    let cancelled = false
    auditApi
      .activities(applied.from, applied.to)
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

  const visible = (result?.events ?? []).filter((activity) => matches(activity, applied))
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
        fallbackTitle="ประวัติการใช้งาน"
        description="การเพิ่ม/แก้ไข/ลบ/ส่งออกในทุกหน้าของผู้ใช้ทุกคน — วันที่และเวลาตามเวลาไทย"
      />

      <Card>
        <CardContent>
          <form onSubmit={handleSubmit} aria-label="ค้นหาประวัติการใช้งาน" className="flex flex-col gap-4">
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-5">
              <Field>
                <FieldLabel htmlFor="activity-history-from">จากวันที่</FieldLabel>
                <Input
                  id="activity-history-from"
                  type="date"
                  value={draft.from}
                  onChange={(e) => set('from')(e.target.value)}
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="activity-history-to">ถึงวันที่</FieldLabel>
                <Input
                  id="activity-history-to"
                  type="date"
                  value={draft.to}
                  onChange={(e) => set('to')(e.target.value)}
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="activity-history-user">ผู้ใช้</FieldLabel>
                <Input
                  id="activity-history-user"
                  value={draft.user}
                  placeholder="ชื่อ หรือ username"
                  onChange={(e) => set('user')(e.target.value)}
                />
              </Field>
              <Field>
                <FieldLabel htmlFor="activity-history-page">หน้า</FieldLabel>
                <NativeSelect
                  id="activity-history-page"
                  value={draft.page}
                  onChange={(e) => set('page')(e.target.value)}
                >
                  <option value="">ทุกหน้า</option>
                  {pageOptions(menu).map((option) => (
                    <option key={option.key} value={option.key}>
                      {option.label}
                    </option>
                  ))}
                </NativeSelect>
              </Field>
              <Field>
                <FieldLabel htmlFor="activity-history-kind">การกระทำ</FieldLabel>
                <NativeSelect
                  id="activity-history-kind"
                  value={draft.kind}
                  onChange={(e) => set('kind')(e.target.value)}
                >
                  <option value="">ทั้งหมด</option>
                  {(Object.keys(KIND_LABELS) as ActivityKind[]).map((kind) => (
                    <option key={kind} value={kind}>
                      {KIND_LABELS[kind]}
                    </option>
                  ))}
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
        filtered={applied.user.trim() !== '' || applied.page !== '' || applied.kind !== ''}
      />

      {status === 'ready' && result?.truncated && (
        <p className="text-sm text-warning">
          ช่วงวันที่นี้มีเกิน 5,000 รายการ แสดงเฉพาะส่วนที่ใหม่สุด — เลือกช่วงวันที่ให้แคบลง
        </p>
      )}

      <Card>
        <CardContent className="flex flex-col gap-3 px-0 sm:px-2">
          {status === 'loading' && <p className="p-4 text-sm text-muted-foreground">กำลังโหลด...</p>}
          {status === 'error' && (
            <p className="p-4 text-sm text-destructive">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</p>
          )}
          {status === 'ready' && visible.length === 0 && (
            <p className="p-4 text-sm text-muted-foreground">ไม่มีประวัติในช่วงนี้ที่ตรงกับเงื่อนไข</p>
          )}
          {status === 'ready' && visible.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>เวลา</TableHead>
                  <TableHead>ผู้ใช้</TableHead>
                  <TableHead>หน้า</TableHead>
                  <TableHead>การกระทำ</TableHead>
                  <TableHead>รายการ</TableHead>
                  <TableHead>รายละเอียด</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((activity, index) => {
                  const action = describeAction(activity.action)
                  return (
                    <TableRow key={`${activity.occurredAt}-${activity.action}-${index}`}>
                      <TableCell className="whitespace-nowrap">
                        {formatThaiDateTime(activity.occurredAt)}
                      </TableCell>
                      <TableCell>
                        {activity.user ? (
                          <>
                            <div className="font-medium">{activity.user.displayName}</div>
                            <div className="font-mono text-xs text-muted-foreground">
                              {activity.user.username}
                            </div>
                          </>
                        ) : (
                          <span className="text-muted-foreground">ไม่ทราบผู้ใช้</span>
                        )}
                      </TableCell>
                      <TableCell>{pageLabel(activityPage(activity), menu)}</TableCell>
                      <TableCell>
                        <Badge variant={KIND_BADGE[action.kind]}>{action.label}</Badge>
                      </TableCell>
                      <TableCell className="max-w-xs whitespace-normal">{describeTarget(activity)}</TableCell>
                      <TableCell>
                        <Button variant="outline" size="sm" onClick={() => setViewing(activity)}>
                          <Eye />
                          ดู
                        </Button>
                      </TableCell>
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          )}
          {status === 'ready' && visible.length > 0 && <TablePagination {...pagination.props} />}
        </CardContent>
      </Card>

      <ActivityDetailDialog activity={viewing} onClose={() => setViewing(null)} />
    </div>
  )
}
