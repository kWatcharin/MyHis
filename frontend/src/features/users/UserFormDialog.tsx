import { useEffect } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import * as z from 'zod'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { NativeSelect } from '@/components/ui/native-select'
import { grantsRoleManagement } from '@/features/admin/permissions'
import type { Role } from '@/features/admin/rolesApi'

import type { User } from './usersApi'

// ตรงกับกฎ backend (backend/src/users/service/users.rs) — ตรวจซ้ำที่นี่ให้ผู้ใช้เห็นก่อนส่ง backend ตรวจอีกรอบเสมอ
const USERNAME_PATTERN = /^[a-z0-9._-]{3,32}$/
const DISPLAY_NAME_MAX = 100

const displayNameSchema = z
  .string()
  .trim()
  .min(1, 'กรอกชื่อที่แสดง')
  .max(DISPLAY_NAME_MAX, `ชื่อที่แสดงยาวไม่เกิน ${DISPLAY_NAME_MAX} ตัวอักษร`)

function RootError({ message }: { message: string | undefined }) {
  if (!message) return null
  return (
    <Field data-invalid>
      <FieldError>{message}</FieldError>
    </Field>
  )
}

// ============================================================================
// สร้างผู้ใช้
// ============================================================================

const createSchema = z.object({
  // เก็บเป็นตัวพิมพ์เล็กที่ trim แล้ว (backend normalize แบบเดียวกัน)
  username: z
    .string()
    .trim()
    .toLowerCase()
    .regex(USERNAME_PATTERN, 'ยาว 3–32 ตัว ใช้ได้เฉพาะ a-z, 0-9, จุด (.), ขีดล่าง (_) และขีด (-)'),
  displayName: displayNameSchema,
  roleId: z.string().min(1, 'เลือก role'),
})

export type CreateUserValues = z.infer<typeof createSchema>

const EMPTY_CREATE: CreateUserValues = { username: '', displayName: '', roleId: '' }

interface CreateUserDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  roles: Role[]
  onSubmit: (values: CreateUserValues) => Promise<void>
}

// สร้างบัญชีใหม่ด้วย role เดียว (auth-design ข้อ 10) — ระบบสุ่มรหัสผ่านชั่วคราวให้ แสดงหลังบันทึกสำเร็จ
export function CreateUserDialog({ open, onOpenChange, roles, onSubmit }: CreateUserDialogProps) {
  const form = useForm<CreateUserValues>({
    resolver: zodResolver(createSchema),
    defaultValues: EMPTY_CREATE,
  })

  useEffect(() => {
    if (open) form.reset(EMPTY_CREATE)
  }, [open, form])

  const handleSubmit = form.handleSubmit(async (values) => {
    try {
      await onSubmit(values)
      onOpenChange(false)
    } catch (err) {
      form.setError('root', {
        message: err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง',
      })
    }
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>เพิ่มผู้ใช้</DialogTitle>
          <DialogDescription>
            บันทึกแล้วระบบสร้างลิงก์ลงทะเบียนให้ — ส่งลิงก์ให้ผู้ใช้เปิดเพื่อกรอกคำนำหน้า ชื่อ-สกุล ตำแหน่ง
            และตั้งรหัสผ่านเอง (แอดมินไม่เห็นรหัสผ่าน)
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} noValidate>
          <FieldGroup>
            <Controller
              name="username"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="user-username">ชื่อผู้ใช้ (ใช้เข้าสู่ระบบ)</FieldLabel>
                  <Input
                    {...field}
                    id="user-username"
                    autoComplete="off"
                    autoCapitalize="none"
                    spellCheck={false}
                    aria-invalid={fieldState.invalid}
                  />
                  <FieldDescription>
                    a-z, 0-9, จุด, ขีดล่าง, ขีด ยาว 3–32 ตัว — เก็บเป็นตัวพิมพ์เล็ก แก้ภายหลังไม่ได้
                  </FieldDescription>
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <Controller
              name="displayName"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="user-display-name">ชื่อที่แสดง</FieldLabel>
                  <Input {...field} id="user-display-name" aria-invalid={fieldState.invalid} />
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <Controller
              name="roleId"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="user-role">Role</FieldLabel>
                  <NativeSelect {...field} id="user-role" aria-invalid={fieldState.invalid}>
                    <option value="">— เลือก —</option>
                    {roles.map((role) => (
                      <option key={role.id} value={role.id}>
                        {role.name}
                        {role.description ? ` — ${role.description}` : ''}
                      </option>
                    ))}
                  </NativeSelect>
                  <FieldDescription>เพิ่ม role อื่นได้ภายหลังจากปุ่ม "แก้ไข"</FieldDescription>
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <RootError message={form.formState.errors.root?.message} />
          </FieldGroup>
          <DialogFooter className="mt-4">
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              ยกเลิก
            </Button>
            <Button type="submit" variant="success" disabled={form.formState.isSubmitting}>
              {form.formState.isSubmitting ? 'กำลังบันทึก...' : 'บันทึก'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

// ============================================================================
// แก้ไขผู้ใช้ (ชื่อที่แสดง + คำนำหน้า/ชื่อ/สกุล/ตำแหน่ง + ชุด role — บันทึกด้วย PUT /users/{id} ครั้งเดียว)
// ============================================================================

const PROFILE_TEXT_MAX = 100

// ช่องไม่บังคับ — ว่างได้ (ส่ง null) ยาวไม่เกิน 100 ตัว ตรงกับ backend
const optionalText = (label: string) =>
  z.string().trim().max(PROFILE_TEXT_MAX, `${label}ยาวไม่เกิน ${PROFILE_TEXT_MAX} ตัวอักษร`)

const editSchema = z.object({
  displayName: displayNameSchema,
  // '' = ไม่ระบุ
  prenameId: z.string(),
  firstName: optionalText('ชื่อ'),
  lastName: optionalText('นามสกุล'),
  position: optionalText('ตำแหน่ง'),
  roleIds: z.array(z.string()).min(1, 'เลือกอย่างน้อย 1 role'),
})

export type EditUserValues = z.infer<typeof editSchema>

export interface PrenameOption {
  id: string
  name: string
}

interface EditUserDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  user: User | null
  // แก้บัญชีของตัวเอง — role ที่เหลือต้องให้ role.view + role.update รวมกัน (backend ปฏิเสธแบบเดียวกัน)
  isSelf: boolean
  roles: Role[]
  // คำนำหน้าที่ยังไม่ถูกลบ (GET /prenames)
  prenames: PrenameOption[]
  onSubmit: (values: EditUserValues) => Promise<void>
}

export function EditUserDialog({
  open,
  onOpenChange,
  user,
  isSelf,
  roles,
  prenames,
  onSubmit,
}: EditUserDialogProps) {
  const form = useForm<EditUserValues>({
    resolver: zodResolver(editSchema),
    defaultValues: {
      displayName: '',
      prenameId: '',
      firstName: '',
      lastName: '',
      position: '',
      roleIds: [],
    },
  })

  // คำนำหน้าเดิมถูกลบจาก master ไปแล้ว — เลือกไม่ได้ (backend ปฏิเสธ) ต้องเลือกใหม่หรือเว้นว่าง
  const prenameRemoved =
    user?.prenameId != null && !prenames.some((prename) => prename.id === user.prenameId)

  // เติมค่าเดิมทุกครั้งที่เปิด
  useEffect(() => {
    if (open && user) {
      form.reset({
        displayName: user.displayName,
        prenameId:
          user.prenameId && prenames.some((prename) => prename.id === user.prenameId)
            ? user.prenameId
            : '',
        firstName: user.firstName ?? '',
        lastName: user.lastName ?? '',
        position: user.position ?? '',
        roleIds: user.roles.map((role) => role.id),
      })
    }
  }, [open, user, prenames, form])

  // สิทธิ์รวมของ role ที่เลือก (union) — กฎเดียวกับ backend: บัญชีตัวเองต้องยังจัดการ role ได้
  const keepsRoleManagement = (roleIds: string[]) =>
    grantsRoleManagement(
      roles.filter((role) => roleIds.includes(role.id)).flatMap((role) => role.permissions),
    )

  const handleSubmit = form.handleSubmit(async (values) => {
    if (isSelf && !keepsRoleManagement(values.roleIds)) {
      form.setError('roleIds', {
        message:
          'บัญชีของตัวเองต้องเหลือ role ที่ให้สิทธิ์ "ดู" และ "แก้ไข" การตั้งค่าสิทธิ์ (กันล็อกตัวเองออกจากหน้าจัดการ)',
      })
      return
    }
    try {
      await onSubmit(values)
      onOpenChange(false)
    } catch (err) {
      form.setError('root', {
        message: err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง',
      })
    }
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>แก้ไขผู้ใช้</DialogTitle>
          <DialogDescription>
            ชื่อผู้ใช้ <span className="font-mono text-foreground">{user?.username}</span>{' '}
            (แก้ไม่ได้){isSelf ? ' — บัญชีของคุณ' : ''}
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} noValidate>
          <FieldGroup>
            <Controller
              name="displayName"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="user-edit-display-name">ชื่อที่แสดง</FieldLabel>
                  <Input {...field} id="user-edit-display-name" aria-invalid={fieldState.invalid} />
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <div className="grid grid-cols-1 gap-4 sm:grid-cols-[8rem_1fr_1fr]">
              <Controller
                name="prenameId"
                control={form.control}
                render={({ field }) => (
                  <Field>
                    <FieldLabel htmlFor="user-edit-prename">คำนำหน้า</FieldLabel>
                    <NativeSelect {...field} id="user-edit-prename">
                      <option value="">— ไม่ระบุ —</option>
                      {prenames.map((prename) => (
                        <option key={prename.id} value={prename.id}>
                          {prename.name}
                        </option>
                      ))}
                    </NativeSelect>
                  </Field>
                )}
              />
              <Controller
                name="firstName"
                control={form.control}
                render={({ field, fieldState }) => (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor="user-edit-first-name">ชื่อ</FieldLabel>
                    <Input {...field} id="user-edit-first-name" aria-invalid={fieldState.invalid} />
                    {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                  </Field>
                )}
              />
              <Controller
                name="lastName"
                control={form.control}
                render={({ field, fieldState }) => (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor="user-edit-last-name">นามสกุล</FieldLabel>
                    <Input {...field} id="user-edit-last-name" aria-invalid={fieldState.invalid} />
                    {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                  </Field>
                )}
              />
            </div>
            {prenameRemoved && (
              <FieldDescription>
                คำนำหน้าเดิม &quot;{user?.prenameName}&quot; ถูกลบจากตั้งค่าคำนำหน้าแล้ว — เลือกใหม่หรือเว้นว่าง
              </FieldDescription>
            )}
            <Controller
              name="position"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="user-edit-position">ตำแหน่ง</FieldLabel>
                  <Input
                    {...field}
                    id="user-edit-position"
                    placeholder="เช่น พยาบาลวิชาชีพ"
                    aria-invalid={fieldState.invalid}
                  />
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <Controller
              name="roleIds"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel>Role (เลือกได้หลาย role — สิทธิ์รวมกัน)</FieldLabel>
                  <div className="flex flex-col gap-2">
                    {roles.map((role) => (
                      <label key={role.id} className="flex items-start gap-2 text-sm font-normal">
                        <Checkbox
                          className="mt-0.5"
                          checked={field.value.includes(role.id)}
                          onCheckedChange={(checked) =>
                            field.onChange(
                              checked === true
                                ? [...field.value, role.id]
                                : field.value.filter((id) => id !== role.id),
                            )
                          }
                        />
                        <span className="flex flex-col gap-0.5">
                          <span className="flex flex-wrap items-center gap-1 font-medium">
                            {role.name}
                            {grantsRoleManagement(role.permissions) && (
                              <Badge variant="outline">จัดการสิทธิ์</Badge>
                            )}
                          </span>
                          {role.description && (
                            <span className="text-xs text-muted-foreground">{role.description}</span>
                          )}
                        </span>
                      </label>
                    ))}
                  </div>
                  {isSelf && (
                    <FieldDescription>
                      บัญชีของตัวเองต้องเหลือสิทธิ์ &quot;ดู&quot; + &quot;แก้ไข&quot; การตั้งค่าสิทธิ์ (role ที่มีป้าย
                      &quot;จัดการสิทธิ์&quot; หรือหลาย role รวมกัน) กันล็อกตัวเองออกจากหน้าจัดการ
                    </FieldDescription>
                  )}
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <RootError message={form.formState.errors.root?.message} />
          </FieldGroup>
          <DialogFooter className="mt-4">
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              ยกเลิก
            </Button>
            <Button type="submit" disabled={form.formState.isSubmitting}>
              {form.formState.isSubmitting ? 'กำลังบันทึก...' : 'บันทึก'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
