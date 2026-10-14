import { useEffect } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import * as z from 'zod'

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
import { useAuth } from '@/features/auth/AuthContext'

import type { PermissionAction } from './permissions'
import { ACTION_LABELS, ALL_PERMISSIONS, groupPermissionResources, permissionKey } from './permissions'
import type { Role } from './rolesApi'

const roleFormSchema = z.object({
  name: z.string().min(1, 'กรอกชื่อ role'),
  description: z.string().optional(),
  permissions: z.array(z.string()),
})

export type RoleFormValues = z.infer<typeof roleFormSchema>

interface RoleFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: 'create' | 'edit'
  initialValue?: Role
  onSubmit: (values: RoleFormValues) => Promise<void>
}

// ใช้ร่วมกันทั้งสร้าง/แก้ไข role — name แก้ไม่ได้ถ้าเป็น system role (admin/nurse) ตรงกับที่
// backend ปฏิเสธ (400 "cannot rename a system role") ส่วน description/permissions แก้ได้ปกติ
export function RoleFormDialog({
  open,
  onOpenChange,
  mode,
  initialValue,
  onSubmit,
}: RoleFormDialogProps) {
  const form = useForm<RoleFormValues>({
    resolver: zodResolver(roleFormSchema),
    defaultValues: { name: '', description: '', permissions: [] },
  })

  // เติมค่าเดิมทุกครั้งที่เปิด dialog โหมดแก้ไข หรือล้างฟอร์มตอนเปิดโหมดสร้าง
  useEffect(() => {
    if (!open) return
    form.reset(
      mode === 'edit' && initialValue
        ? {
            name: initialValue.name,
            description: initialValue.description ?? '',
            permissions: initialValue.permissions,
          }
        : { name: '', description: '', permissions: [] },
    )
  }, [open, mode, initialValue, form])

  const isSystem = mode === 'edit' && initialValue?.isSystem === true
  // ชื่อเพจ/กลุ่มตามเมนู (การตั้งค่าเพจ) — GET /menu ทุกคนเรียกได้ ไม่ต้องมี page.view
  const { menu } = useAuth()
  const permissionGroups = groupPermissionResources(menu)

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
      {/* ตารางสิทธิ์รายเพจยาวเกินจอ — จำกัดความสูงแล้วเลื่อนในกรอบ dialog */}
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{mode === 'create' ? 'เพิ่ม role' : 'แก้ไข role'}</DialogTitle>
          {isSystem && (
            <DialogDescription>
              role ของระบบ เปลี่ยนชื่อไม่ได้ แต่แก้คำอธิบาย/สิทธิ์ได้ปกติ
            </DialogDescription>
          )}
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <FieldGroup>
            <Controller
              name="name"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="role-name">ชื่อ role</FieldLabel>
                  <Input
                    {...field}
                    id="role-name"
                    disabled={isSystem}
                    aria-invalid={fieldState.invalid}
                  />
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <Controller
              name="description"
              control={form.control}
              render={({ field }) => (
                <Field>
                  <FieldLabel htmlFor="role-description">คำอธิบาย</FieldLabel>
                  <Input {...field} id="role-description" />
                </Field>
              )}
            />
            <Controller
              name="permissions"
              control={form.control}
              render={({ field }) => {
                const selected = new Set(field.value)
                // ติ๊ก เพิ่ม/แก้ไข/ลบ/ส่งออก แล้วติ๊ก "ดู" ของเพจนั้นให้ด้วย (ไม่มีดูก็เข้าหน้านั้นไม่ได้) เอาออกทีละช่อง
                // ค่าที่ส่งเรียงตาม ALL_PERMISSIONS (ลำดับเดียวกับตาราง backend)
                const toggle = (resourceKey: string, action: PermissionAction, checked: boolean) => {
                  const next = new Set(field.value)
                  const key = permissionKey(resourceKey, action)
                  if (checked) {
                    next.add(key)
                    next.add(permissionKey(resourceKey, 'view'))
                  } else {
                    next.delete(key)
                  }
                  field.onChange(ALL_PERMISSIONS.filter((permission) => next.has(permission)))
                }
                return (
                  <Field>
                    <FieldLabel>สิทธิ์</FieldLabel>
                    <FieldDescription>
                      แยกตามเพจ — ติ๊ก เพิ่ม/แก้ไข/ลบ/ส่งออก แล้วจะติ๊ก &quot;ดู&quot; ให้อัตโนมัติ
                    </FieldDescription>
                    <div className="flex flex-col gap-4">
                      {permissionGroups.map((group) => (
                        <div key={group.label} className="flex flex-col gap-2">
                          <p className="text-sm font-semibold">{group.label}</p>
                          {group.resources.map(({ resource, label }) => (
                            <div
                              key={resource.key}
                              className="flex flex-col gap-1.5 rounded-lg border px-3 py-2"
                            >
                              <span className="text-sm font-medium">{label}</span>
                              <div className="flex flex-wrap gap-x-4 gap-y-1.5">
                                {resource.actions.map((action) => (
                                  <label
                                    key={action}
                                    className="flex items-center gap-1.5 text-sm font-normal"
                                  >
                                    <Checkbox
                                      checked={selected.has(permissionKey(resource.key, action))}
                                      onCheckedChange={(checked) =>
                                        toggle(resource.key, action, checked === true)
                                      }
                                    />
                                    {ACTION_LABELS[action]}
                                  </label>
                                ))}
                              </div>
                              {resource.requires &&
                                selected.has(permissionKey(resource.key, 'view')) &&
                                !selected.has(resource.requires.permission) && (
                                  <p className="text-xs text-warning">{resource.requires.message}</p>
                                )}
                            </div>
                          ))}
                        </div>
                      ))}
                    </div>
                  </Field>
                )
              }}
            />
            {form.formState.errors.root && (
              <Field data-invalid>
                <FieldError>{form.formState.errors.root.message}</FieldError>
              </Field>
            )}
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
