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
import { Field, FieldError, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'

import { ALL_PERMISSIONS, PERMISSION_LABELS } from './permissions'
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
      <DialogContent>
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
              render={({ field }) => (
                <Field>
                  <FieldLabel>สิทธิ์</FieldLabel>
                  <div className="flex flex-col gap-2">
                    {ALL_PERMISSIONS.map((permission) => (
                      <label
                        key={permission}
                        className="flex items-center gap-2 text-sm font-normal"
                      >
                        <Checkbox
                          checked={field.value.includes(permission)}
                          onCheckedChange={(checked) => {
                            const next = checked === true
                            field.onChange(
                              next
                                ? [...field.value, permission]
                                : field.value.filter((p) => p !== permission),
                            )
                          }}
                        />
                        {PERMISSION_LABELS[permission]}
                        <span className="text-muted-foreground">({permission})</span>
                      </label>
                    ))}
                  </div>
                </Field>
              )}
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
