import { Badge } from '@/components/ui/badge'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import type { MenuItem } from '@/features/auth/AuthContext'
import { useAuth } from '@/features/auth/AuthContext'
import { formatThaiDateTime } from '@/lib/date'

import { activityPage, describeAction, describeTarget, KIND_BADGE, pageLabel } from './activities'
import { buildDetailView } from './activityDetails'
import type { Activity } from '../auditApi'

interface ActivityDetailDialogProps {
  // null = ปิด
  activity: Activity | null
  onClose: () => void
}

function actorText(activity: Activity): string {
  return activity.user ? `${activity.user.displayName} (${activity.user.username})` : 'ไม่ทราบผู้ใช้'
}

// modal "รายละเอียด" ของหนึ่งแถวในประวัติการใช้งาน: เพิ่ม = ค่าที่เพิ่ม, แก้ไข = ค่าเดิม→ใหม่เฉพาะช่องที่เปลี่ยน,
// ลบ = เมื่อไหร่/โดยใคร, ข้อมูลผู้ป่วย = ชื่อช่องที่แก้อย่างเดียว (backend ไม่เก็บค่า)
export function ActivityDetailDialog({ activity, onClose }: ActivityDetailDialogProps) {
  const { menu } = useAuth()
  const open = activity !== null

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose()
      }}
    >
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
        {activity && <DetailBody activity={activity} menu={menu} />}
      </DialogContent>
    </Dialog>
  )
}

function DetailBody({ activity, menu }: { activity: Activity; menu: MenuItem[] }) {
  const menuLabel = pageLabel(activityPage(activity), menu)
  const action = describeAction(activity.action)
  const view = buildDetailView(activity, menu)
  const when = formatThaiDateTime(activity.occurredAt)

  return (
    <>
      <DialogHeader>
        <DialogTitle className="flex flex-wrap items-center gap-2">
          <Badge variant={KIND_BADGE[action.kind]}>{action.label}</Badge>
          <span>{menuLabel}</span>
        </DialogTitle>
        <DialogDescription>{describeTarget(activity)}</DialogDescription>
      </DialogHeader>

      <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
        <dt className="text-muted-foreground">เวลา</dt>
        <dd>{when}</dd>
        <dt className="text-muted-foreground">ผู้ทำ</dt>
        <dd>{actorText(activity)}</dd>
      </dl>

      {view.kind === 'created' && (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>ช่อง</TableHead>
              <TableHead>ค่าที่เพิ่ม</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {view.rows.map((row) => (
              <TableRow key={row.field}>
                <TableCell className="text-muted-foreground">{row.label}</TableCell>
                <TableCell className="whitespace-normal">{row.value}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}

      {view.kind === 'updated' && (
        <div className="flex flex-col gap-3">
          {view.rows.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>ช่อง</TableHead>
                  <TableHead>ค่าเดิม</TableHead>
                  <TableHead>ค่าใหม่</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {view.rows.map((row) => (
                  <TableRow key={row.field}>
                    <TableCell className="text-muted-foreground">{row.label}</TableCell>
                    <TableCell className="whitespace-normal text-muted-foreground line-through decoration-muted-foreground/50">
                      {row.before}
                    </TableCell>
                    <TableCell className="whitespace-normal font-medium">{row.after}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
          {view.permissions && (
            <div className="flex flex-col gap-1 text-sm">
              <p>
                <span className="text-muted-foreground">สิทธิ์ที่เพิ่ม: </span>
                {view.permissions.added.length > 0 ? view.permissions.added.join(', ') : '—'}
              </p>
              <p>
                <span className="text-muted-foreground">สิทธิ์ที่เอาออก: </span>
                {view.permissions.removed.length > 0 ? view.permissions.removed.join(', ') : '—'}
              </p>
            </div>
          )}
          {view.rows.length === 0 && !view.permissions && (
            <p className="text-sm text-muted-foreground">บันทึกโดยไม่มีค่าที่เปลี่ยน</p>
          )}
        </div>
      )}

      {view.kind === 'fields' && (
        <div className="flex flex-col gap-1 text-sm">
          <p>
            <span className="text-muted-foreground">ช่องที่แก้ไข: </span>
            {view.labels.length > 0 ? view.labels.join(', ') : 'ไม่มีช่องที่เปลี่ยน'}
          </p>
          <p className="text-xs text-muted-foreground">ไม่แสดงค่าของข้อมูลผู้ป่วย (ระบบไม่เก็บค่าไว้ในประวัติ)</p>
        </div>
      )}

      {view.kind === 'deleted' && (
        <p className="text-sm">
          ลบเมื่อ {when} โดย {actorText(activity)}
        </p>
      )}

      {view.kind === 'note' && <p className="text-sm text-muted-foreground">{view.text}</p>}

      {view.kind === 'none' && (
        <p className="text-sm text-muted-foreground">
          {action.kind === 'export'
            ? describeTarget(activity)
            : 'รายการนี้บันทึกก่อนเริ่มเก็บรายละเอียดการเปลี่ยนแปลง ไม่มีค่าที่เปลี่ยนให้แสดง'}
        </p>
      )}
    </>
  )
}
