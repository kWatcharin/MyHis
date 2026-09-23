import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { resolveIcon } from '@/lib/icons'
import { mockPages } from '@/mocks/fixtures'

// โครงหน้า + mock data เท่านั้นรอบนี้ (ตามที่ตกลงไว้) — ยังไม่ต่อ GET/POST/PATCH/DELETE /pages
// จริง ปุ่มแก้ไข/ลบ/เพิ่ม page มีให้เห็นแต่ยังไม่ submit ไปไหนจริง โครง field ตรงกับ PageResponse
// ของ backend (backend/src/users/dto.rs) พอถึงเวลาต่อ API จริงจะได้แค่เปลี่ยนแหล่งข้อมูล
export function PagesSettingsPage() {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-2">
        <div>
          <h1 className="text-lg font-medium">การตั้งค่าเพจ</h1>
          <p className="text-sm text-muted-foreground">
            โครง + mock data — ยังไม่ต่อ API จริง
          </p>
        </div>
        <Button disabled>+ เพิ่มเพจ</Button>
      </div>

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>เพจ</TableHead>
            <TableHead>route</TableHead>
            <TableHead>ลำดับ</TableHead>
            <TableHead>สิทธิ์ที่ต้องมี</TableHead>
            <TableHead className="text-right">จัดการ</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {mockPages.map((page) => {
            const Icon = resolveIcon(page.icon)
            return (
              <TableRow key={page.id}>
                <TableCell className="flex items-center gap-2 font-medium">
                  <Icon className="size-4 text-muted-foreground" />
                  {page.label}
                </TableCell>
                <TableCell className="font-mono text-sm text-muted-foreground">
                  {page.route}
                </TableCell>
                <TableCell className="text-muted-foreground">{page.sortOrder}</TableCell>
                <TableCell>
                  {page.permissions.length === 0 ? (
                    <span className="text-sm text-muted-foreground">สาธารณะ</span>
                  ) : (
                    <div className="flex flex-wrap gap-1">
                      {page.permissions.map((permission) => (
                        <Badge key={permission} variant="outline">
                          {permission}
                        </Badge>
                      ))}
                    </div>
                  )}
                </TableCell>
                <TableCell className="text-right">
                  <Button variant="ghost" size="sm" disabled>
                    แก้ไข
                  </Button>
                  <Button variant="ghost" size="sm">
                    ลบ
                  </Button>
                </TableCell>
              </TableRow>
            )
          })}
        </TableBody>
      </Table>
    </div>
  )
}
