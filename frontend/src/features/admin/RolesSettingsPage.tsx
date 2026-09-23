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
import { mockRoles } from '@/mocks/fixtures'

// โครงหน้า + mock data เท่านั้นรอบนี้ (ตามที่ตกลงไว้) — ยังไม่ต่อ GET/POST/PATCH/DELETE /roles
// จริง ปุ่มแก้ไข/ลบ/เพิ่ม role มีให้เห็นแต่ยังไม่ submit ไปไหนจริง โครง field ตรงกับ RoleResponse
// ของ backend (backend/src/users/dto.rs) พอถึงเวลาต่อ API จริงจะได้แค่เปลี่ยนแหล่งข้อมูล
export function RolesSettingsPage() {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between gap-2">
        <div>
          <h1 className="text-lg font-medium">การตั้งค่า role</h1>
          <p className="text-sm text-muted-foreground">
            โครง + mock data — ยังไม่ต่อ API จริง
          </p>
        </div>
        <Button disabled>+ เพิ่ม role</Button>
      </div>

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>ชื่อ role</TableHead>
            <TableHead>คำอธิบาย</TableHead>
            <TableHead>สิทธิ์</TableHead>
            <TableHead className="text-right">จัดการ</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {mockRoles.map((role) => (
            <TableRow key={role.id}>
              <TableCell className="font-medium">
                {role.name}
                {role.isSystem && (
                  <Badge variant="secondary" className="ml-2">
                    system
                  </Badge>
                )}
              </TableCell>
              <TableCell className="text-muted-foreground">
                {role.description ?? '—'}
              </TableCell>
              <TableCell>
                <div className="flex flex-wrap gap-1">
                  {role.permissions.map((permission) => (
                    <Badge key={permission} variant="outline">
                      {permission}
                    </Badge>
                  ))}
                </div>
              </TableCell>
              <TableCell className="text-right">
                <Button variant="ghost" size="sm" disabled>
                  แก้ไข
                </Button>
                <Button variant="ghost" size="sm" disabled={role.isSystem}>
                  ลบ
                </Button>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  )
}
