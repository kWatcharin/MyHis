import { KeyRound, Link2, UserCheck, UserX } from 'lucide-react'
import { useState } from 'react'

import { AddButton, EditButton } from '@/components/layout/ActionButtons'
import { ListState } from '@/components/layout/ListState'
import { ListToolbar } from '@/components/layout/ListToolbar'
import { PageHeader } from '@/components/layout/PageHeader'
import { SearchForm } from '@/components/layout/SearchForm'
import { TablePagination } from '@/components/layout/TablePagination'
import { usePagination } from '@/components/layout/usePagination'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { prenameConfig } from '@/features/admin/master/configs'
import { rolesApi } from '@/features/admin/rolesApi'
import { useAdminList } from '@/features/admin/useAdminList'
import { useAuth } from '@/features/auth/AuthContext'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'
import { usePagePermissions } from '@/features/role-preview/usePagePermissions'
import { formatThaiDateTime } from '@/lib/date'
import { notify } from '@/lib/notify'
import { hasCriteria, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'
import { cn } from '@/lib/utils'

import { userErrorMessage } from './errors'
import type { IssuedLink } from './RegistrationLinkDialog'
import { RegistrationLinkDialog } from './RegistrationLinkDialog'
import type { CreateUserValues, EditUserValues } from './UserFormDialog'
import { CreateUserDialog, EditUserDialog } from './UserFormDialog'
import type { User } from './usersApi'
import { fullName, usersApi } from './usersApi'

type PendingAction = { kind: 'deactivate' | 'activate' | 'reset'; user: User } | null

function sameIds(a: string[], b: string[]): boolean {
  return a.length === b.length && a.every((id) => b.includes(id))
}

// ว่าง → null (ล้างค่า) ตามสัญญาของ PUT /users/{id}
const orNull = (value: string) => (value.trim() === '' ? null : value.trim())

// ป้ายสถานะการลงทะเบียน/ลิงก์ที่ค้าง — ยังไม่ลงทะเบียน = login ไม่ได้จนกว่าจะตั้งรหัสผ่านผ่านลิงก์
function RegistrationBadge({ user }: { user: User }) {
  const until = user.registrationExpiresAt ? formatThaiDateTime(user.registrationExpiresAt) : ''
  if (user.registeredAt === null) {
    if (user.linkState === 'pending') {
      return (
        <Badge variant="warning" title={`ลิงก์ลงทะเบียนใช้ได้ถึง ${until}`}>
          รอลงทะเบียน
        </Badge>
      )
    }
    return (
      <Badge variant="danger" title="ยังเข้าระบบไม่ได้ — กด &quot;ออกลิงก์ใหม่&quot; แล้วส่งให้ผู้ใช้">
        {user.linkState === 'expired' ? 'ลิงก์ลงทะเบียนหมดอายุ' : 'ยังไม่ลงทะเบียน — ไม่มีลิงก์'}
      </Badge>
    )
  }
  if (user.linkState === 'pending') {
    return (
      <Badge variant="warning" title={`ลิงก์ตั้งรหัสใหม่ใช้ได้ถึง ${until}`}>
        รอตั้งรหัสใหม่
      </Badge>
    )
  }
  if (user.linkState === 'expired') {
    return (
      <Badge variant="danger" title="รหัสเดิมใช้ไม่ได้แล้ว — กด &quot;รีเซ็ตรหัสผ่าน&quot; เพื่อออกลิงก์ใหม่">
        ลิงก์ตั้งรหัสหมดอายุ
      </Badge>
    )
  }
  return null
}

// หน้า "ผู้ใช้งาน" (กลุ่มเมนูลงทะเบียน, ต้องมี user.view): สร้างบัญชี (user.create — ได้ลิงก์ลงทะเบียน ผู้ใช้กรอก
// ข้อมูลตัวและตั้งรหัสผ่านเอง), แก้ข้อมูล/role, ปิด-เปิดใช้งาน, รีเซ็ตรหัสผ่าน/ออกลิงก์ใหม่ (ทั้งหมด user.update) —
// แอดมินไม่เห็นรหัสผ่านของใคร ไม่มีการลบผู้ใช้จริง (ใช้ "ปิดใช้งาน" แทน — soft delete ด้วยคอลัมน์สถานะ) backend กัน
// แอดมินล็อกตัวเองออก (ปิดบัญชี/รีเซ็ตรหัสของตัวเอง/ถอดสิทธิ์จัดการ role ของตัวเอง) — หน้านี้ซ่อนปุ่มเหล่านั้นใน
// แถวของตัวเองให้ด้วย
export function UsersPage() {
  const { canCreate, canUpdate } = usePagePermissions('user')
  const { items: users, status, refetch } = useAdminList(usersApi.list)
  const { items: roles, status: rolesStatus } = useAdminList(rolesApi.list)
  const { items: prenames, status: prenamesStatus } = useAdminList(prenameConfig.api.list)
  const { user: me, refresh: refreshAuth } = useAuth()
  const { refetchCatalog } = useRolePreview()

  const [isCreating, setIsCreating] = useState(false)
  const [editing, setEditing] = useState<User | null>(null)
  const [pending, setPending] = useState<PendingAction>(null)
  const [isWorking, setIsWorking] = useState(false)
  // ลิงก์ที่เพิ่งออก (แสดงครั้งเดียว) — ล้างเป็น null ตอนปิด dialog
  const [issuedLink, setIssuedLink] = useState<IssuedLink | null>(null)
  // เงื่อนไขค้นหาที่ใช้กรองจริง (เปลี่ยนเมื่อกดปุ่ม "ค้นหา") — ช่องที่กรอกต้องตรงทุกช่อง
  const [criteria, setCriteria] = useState<SearchValues>({})

  const visibleUsers = users.filter((user) => {
    const name = criteria.name ?? ''
    const role = criteria.role ?? ''
    return (
      (textIncludes(user.username, name) ||
        textIncludes(user.displayName, name) ||
        textIncludes(fullName(user), name) ||
        textIncludes(user.position, name)) &&
      (!role.trim() || user.roles.some((userRole) => textIncludes(userRole.name, role)))
    )
  })
  const pagination = usePagination(visibleUsers)

  const isSelf = (user: User) => user.id === me?.id
  const formsReady = rolesStatus === 'ready'

  // หลังแก้บัญชี: โหลดตารางใหม่ — ถ้าเป็นบัญชีของตัวเอง (ชื่อ/role) โหลดสิทธิ์ เมนู และตัวเลือกของ
  // RoleSwitcher ใหม่ด้วย ให้ทุกอย่างตรงกับของจริงทันทีโดยไม่ต้อง login ใหม่
  const reloadAfterChange = async (user: User) => {
    await refetch()
    if (isSelf(user)) {
      refetchCatalog()
      await refreshAuth()
    }
  }

  const handleCreate = async (values: CreateUserValues) => {
    const result = await usersApi.create(values).catch((err: unknown) => {
      throw new Error(userErrorMessage(err))
    })
    await refetch()
    notify.created('เพิ่มผู้ใช้แล้ว — ส่งลิงก์ลงทะเบียนให้ผู้ใช้')
    setIssuedLink({
      username: result.user.username,
      token: result.registrationToken,
      expiresAt: result.registrationExpiresAt,
      reason: 'created',
    })
  }

  const handleEdit = async (values: EditUserValues) => {
    if (!editing) return
    const input = {
      displayName: values.displayName,
      prenameId: orNull(values.prenameId),
      firstName: orNull(values.firstName),
      lastName: orNull(values.lastName),
      position: orNull(values.position),
      roleIds: values.roleIds,
    }
    const unchanged =
      input.displayName === editing.displayName &&
      input.prenameId === editing.prenameId &&
      input.firstName === editing.firstName &&
      input.lastName === editing.lastName &&
      input.position === editing.position &&
      sameIds(
        input.roleIds,
        editing.roles.map((role) => role.id),
      )
    if (unchanged) return
    try {
      // ทุกช่อง + ชุด role ในทรานแซกชันเดียว — สำเร็จหรือไม่เปลี่ยนอะไรเลย
      await usersApi.replace(editing.id, input)
    } catch (err) {
      throw new Error(userErrorMessage(err))
    }
    await reloadAfterChange(editing)
    notify.updated('แก้ไขผู้ใช้แล้ว')
  }

  const handleConfirm = async () => {
    if (!pending) return
    const { kind, user } = pending
    setIsWorking(true)
    try {
      if (kind === 'reset') {
        const link = await usersApi.resetPassword(user.id)
        setPending(null)
        await refetch()
        setIssuedLink({
          username: user.username,
          token: link.registrationToken,
          expiresAt: link.registrationExpiresAt,
          reason: user.registeredAt === null ? 'reissued' : 'reset',
        })
      } else {
        await usersApi.update(user.id, { isActive: kind === 'activate' })
        setPending(null)
        await refetch()
        notify.updated(kind === 'activate' ? 'เปิดใช้งานผู้ใช้แล้ว' : 'ปิดใช้งานผู้ใช้แล้ว')
      }
    } catch (err) {
      notify.error(userErrorMessage(err))
    } finally {
      setIsWorking(false)
    }
  }

  // ข้อความยืนยันตามการกระทำ — "reset" ของคนที่ยังไม่ลงทะเบียน = ออกลิงก์ลงทะเบียนใหม่
  const confirmText = (() => {
    if (!pending) return null
    const who = `"${pending.user.displayName}" (${pending.user.username})`
    if (pending.kind === 'reset' && pending.user.registeredAt === null) {
      return {
        title: 'ยืนยันออกลิงก์ลงทะเบียนใหม่',
        description: `ลิงก์ลงทะเบียนเดิมของ ${who} จะใช้ไม่ได้ทันที แล้วได้ลิงก์ใหม่ (ใช้ได้ครั้งเดียว อายุ 3 วัน)`,
        action: 'ออกลิงก์ใหม่',
        variant: 'warning' as const,
      }
    }
    if (pending.kind === 'reset') {
      return {
        title: 'ยืนยันรีเซ็ตรหัสผ่าน',
        description: `รหัสผ่านเดิมของ ${who} จะใช้ไม่ได้ทันที ผู้ใช้ถูกออกจากระบบทุกอุปกรณ์ (ถ้าถูกล็อกจะปลดล็อก) แล้วได้ลิงก์ให้ผู้ใช้ตั้งรหัสใหม่เอง (ใช้ได้ครั้งเดียว อายุ 3 วัน)`,
        action: 'รีเซ็ตรหัสผ่าน',
        variant: 'warning' as const,
      }
    }
    if (pending.kind === 'deactivate') {
      return {
        title: 'ยืนยันปิดใช้งานผู้ใช้',
        description: `ปิดใช้งาน ${who} ใช่ไหม? ผู้ใช้จะถูกออกจากระบบทุกอุปกรณ์ทันที และเข้าสู่ระบบ/ใช้ลิงก์ลงทะเบียนไม่ได้จนกว่าจะเปิดใช้งานอีกครั้ง (ข้อมูลที่เคยบันทึกยังอยู่ครบ)`,
        action: 'ปิดใช้งาน',
        variant: 'danger' as const,
      }
    }
    return {
      title: 'ยืนยันเปิดใช้งานผู้ใช้',
      description: `เปิดใช้งาน ${who} อีกครั้ง — เข้าสู่ระบบได้ด้วยรหัสผ่านเดิม (หรือใช้ลิงก์เดิมถ้ายังไม่หมดอายุ)`,
      action: 'เปิดใช้งาน',
      variant: 'success' as const,
    }
  })()

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        fallbackTitle="ผู้ใช้งาน"
        description="สร้างบัญชีแล้วส่งลิงก์ลงทะเบียนให้ผู้ใช้ตั้งรหัสผ่านเอง กำหนด role ปิด/เปิดใช้งาน และรีเซ็ตรหัสผ่าน"
      />

      <SearchForm
        id="user-search"
        ariaLabel="ค้นหาผู้ใช้"
        fields={[
          { name: 'name', label: 'ชื่อผู้ใช้ / ชื่อ / ตำแหน่ง' },
          { name: 'role', label: 'Role', placeholder: 'เช่น admin หรือ nurse' },
        ]}
        onSearch={(values) => {
          setCriteria(values)
          pagination.reset()
        }}
      />

      <ListToolbar
        ready={status === 'ready'}
        total={users.length}
        shown={visibleUsers.length}
        filtered={hasCriteria(criteria)}
      >
        {canCreate && (
          <AddButton disabled={!formsReady} onClick={() => setIsCreating(true)}>
            เพิ่มผู้ใช้
          </AddButton>
        )}
      </ListToolbar>

      <Card>
        <CardContent className="flex flex-col gap-3 px-0 sm:px-2">
          {status === 'loading' && <ListState kind="loading" />}
          {status === 'error' && (
            <ListState kind="error">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</ListState>
          )}
          {status === 'ready' && users.length > 0 && visibleUsers.length === 0 && (
            <ListState kind="noMatch">
              ไม่พบผู้ใช้ที่ตรงกับเงื่อนไข — กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด
            </ListState>
          )}
          {status === 'ready' && visibleUsers.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>ชื่อผู้ใช้</TableHead>
                  <TableHead>ชื่อ</TableHead>
                  <TableHead>Role</TableHead>
                  <TableHead>สถานะ</TableHead>
                  <TableHead>สร้างเมื่อ</TableHead>
                  {canUpdate && <TableHead className="text-right">จัดการ</TableHead>}
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((user) => {
                  const name = fullName(user)
                  const details = [name, user.position].filter(Boolean).join(' · ')
                  return (
                    <TableRow key={user.id} className={cn(!user.isActive && 'text-muted-foreground')}>
                      <TableCell className="font-mono text-sm">
                        {user.username}
                        {isSelf(user) && (
                          <Badge variant="secondary" className="ml-2 font-sans">
                            คุณ
                          </Badge>
                        )}
                      </TableCell>
                      <TableCell>
                        <div className="font-medium">{user.displayName}</div>
                        {details && <div className="text-xs text-muted-foreground">{details}</div>}
                      </TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-1">
                          {user.roles.map((role) => (
                            <Badge key={role.id} variant="outline">
                              {role.name}
                            </Badge>
                          ))}
                        </div>
                      </TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-1">
                          <Badge variant={user.isActive ? 'success' : 'outline'}>
                            {user.isActive ? 'ใช้งานอยู่' : 'ปิดใช้งาน'}
                          </Badge>
                          <RegistrationBadge user={user} />
                          {user.mustChangePassword && (
                            <Badge
                              variant="warning"
                              title="บัญชีเดิมที่ยังไม่ได้เปลี่ยนรหัสผ่านชั่วคราว"
                            >
                              ยังใช้รหัสชั่วคราว
                            </Badge>
                          )}
                          {user.isLocked && user.lockedUntil && (
                            <Badge
                              variant="danger"
                              title="ใส่รหัสผิดหลายครั้ง — รอให้พ้นเวลา หรือรีเซ็ตรหัสผ่านเพื่อปลดล็อกทันที"
                            >
                              ล็อกถึง {formatThaiDateTime(user.lockedUntil)}
                            </Badge>
                          )}
                        </div>
                      </TableCell>
                      <TableCell className="text-muted-foreground">
                        {formatThaiDateTime(user.createdAt, { time: false })}
                      </TableCell>
                      {canUpdate && (
                      <TableCell className="text-right">
                        <div className="flex flex-wrap justify-end gap-1">
                          {/* ต้องโหลดคำนำหน้าได้ก่อน ไม่งั้นฟอร์มจะล้างคำนำหน้าเดิมของผู้ใช้ตอนบันทึก */}
                          <EditButton
                            disabled={!formsReady || prenamesStatus !== 'ready'}
                            onClick={() => setEditing(user)}
                          />
                          {!isSelf(user) && (
                            <>
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => setPending({ kind: 'reset', user })}
                              >
                                {user.registeredAt === null ? <Link2 /> : <KeyRound />}
                                {user.registeredAt === null ? 'ออกลิงก์ใหม่' : 'รีเซ็ตรหัสผ่าน'}
                              </Button>
                              {user.isActive ? (
                                <Button
                                  variant="danger"
                                  size="sm"
                                  onClick={() => setPending({ kind: 'deactivate', user })}
                                >
                                  <UserX />
                                  ปิดใช้งาน
                                </Button>
                              ) : (
                                <Button
                                  variant="success"
                                  size="sm"
                                  onClick={() => setPending({ kind: 'activate', user })}
                                >
                                  <UserCheck />
                                  เปิดใช้งาน
                                </Button>
                              )}
                            </>
                          )}
                        </div>
                      </TableCell>
                      )}
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          )}
          {status === 'ready' && visibleUsers.length > 0 && <TablePagination {...pagination.props} />}
          {rolesStatus === 'error' && (
            <p className="px-4 pb-2 text-sm text-destructive">
              โหลดรายชื่อ role ไม่สำเร็จ — เพิ่ม/แก้ไขผู้ใช้ไม่ได้จนกว่าจะรีเฟรชหน้า
            </p>
          )}
          {prenamesStatus === 'error' && (
            <p className="px-4 pb-2 text-sm text-destructive">
              โหลดรายการคำนำหน้าไม่สำเร็จ — แก้ไขผู้ใช้ไม่ได้จนกว่าจะรีเฟรชหน้า
            </p>
          )}
        </CardContent>
      </Card>

      <CreateUserDialog
        open={isCreating}
        onOpenChange={setIsCreating}
        roles={roles}
        onSubmit={handleCreate}
      />

      <EditUserDialog
        open={editing !== null}
        onOpenChange={(open) => {
          if (!open) setEditing(null)
        }}
        user={editing}
        isSelf={editing !== null && isSelf(editing)}
        roles={roles}
        prenames={prenames}
        onSubmit={handleEdit}
      />

      <AlertDialog
        open={pending !== null}
        onOpenChange={(open) => {
          if (!open) setPending(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{confirmText?.title}</AlertDialogTitle>
            <AlertDialogDescription>{confirmText?.description}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>ยกเลิก</AlertDialogCancel>
            <AlertDialogAction
              variant={confirmText?.variant}
              onClick={() => void handleConfirm()}
              disabled={isWorking}
            >
              {isWorking ? 'กำลังดำเนินการ...' : confirmText?.action}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <RegistrationLinkDialog value={issuedLink} onClose={() => setIssuedLink(null)} />
    </div>
  )
}
