import { useEffect } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import * as z from 'zod'

import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { resolveIcon } from '@/lib/icons'

import { ALL_PERMISSIONS, PERMISSION_LABELS } from './permissions'
import type { Page } from './pagesApi'

const pageFormSchema = z.object({
  label: z.string().min(1, 'กรอกชื่อเพจ'),
  icon: z.string().optional(),
  route: z
    .string()
    .min(1, 'กรอก route')
    .regex(/^\//, 'route ต้องขึ้นต้นด้วย /'),
  sortOrder: z.number().int(),
  permissions: z.array(z.string()),
  groupLabel: z.string().optional(),
})

export type PageFormValues = z.infer<typeof pageFormSchema>

interface PageFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: 'create' | 'edit'
  initialValue?: Page
  onSubmit: (values: PageFormValues) => Promise<void>
  // ชื่อกลุ่มที่มีอยู่แล้ว (distinct) ใช้แนะนำผ่าน <datalist> กันพิมพ์ชื่อกลุ่มเพี้ยนโดยไม่ตั้งใจ
  // ยังพิมพ์ชื่อใหม่ได้ตามปกติ ไม่ใช่ dropdown บังคับเลือก
  existingGroupLabels: string[]
}

// ใช้ร่วมกันทั้งสร้าง/แก้ไขเพจ — icon เป็น text input พิมพ์ชื่อ slug ของ lucide เอง (เช่น "home")
// ยังไม่ทำ icon picker เต็มรูปแบบ มี preview สดข้าง ๆ ช่วยเช็คว่าพิมพ์ชื่อถูกไหม ใช้ resolveIcon
// ตัวเดียวกับที่ sidebar ใช้จริง
export function PageFormDialog({
  open,
  onOpenChange,
  mode,
  initialValue,
  onSubmit,
  existingGroupLabels,
}: PageFormDialogProps) {
  const form = useForm<PageFormValues>({
    resolver: zodResolver(pageFormSchema),
    defaultValues: { label: '', icon: '', route: '', sortOrder: 0, permissions: [], groupLabel: '' },
  })

  useEffect(() => {
    if (!open) return
    form.reset(
      mode === 'edit' && initialValue
        ? {
            label: initialValue.label,
            icon: initialValue.icon ?? '',
            route: initialValue.route,
            sortOrder: initialValue.sortOrder,
            permissions: initialValue.permissions,
            groupLabel: initialValue.groupLabel ?? '',
          }
        : { label: '', icon: '', route: '', sortOrder: 0, permissions: [], groupLabel: '' },
    )
  }, [open, mode, initialValue, form])

  const iconValue = form.watch('icon')
  const IconPreview = resolveIcon(iconValue || null)

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
          <DialogTitle>{mode === 'create' ? 'เพิ่มเพจ' : 'แก้ไขเพจ'}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit}>
          <FieldGroup>
            <Controller
              name="label"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="page-label">ชื่อเพจ</FieldLabel>
                  <Input {...field} id="page-label" aria-invalid={fieldState.invalid} />
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <Controller
              name="icon"
              control={form.control}
              render={({ field }) => (
                <Field>
                  <FieldLabel htmlFor="page-icon">ไอคอน</FieldLabel>
                  <div className="flex items-center gap-2">
                    <span className="flex size-8 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
                      <IconPreview className="size-4" />
                    </span>
                    <Input {...field} id="page-icon" placeholder="home" />
                  </div>
                  <FieldDescription>
                    ชื่อ slug ของ lucide เช่น home, layout-list, shield-check
                  </FieldDescription>
                </Field>
              )}
            />
            <Controller
              name="route"
              control={form.control}
              render={({ field, fieldState }) => (
                <Field data-invalid={fieldState.invalid}>
                  <FieldLabel htmlFor="page-route">route</FieldLabel>
                  <Input
                    {...field}
                    id="page-route"
                    placeholder="/settings/example"
                    className="font-mono"
                    aria-invalid={fieldState.invalid}
                  />
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <Controller
              name="groupLabel"
              control={form.control}
              render={({ field }) => (
                <Field>
                  <FieldLabel htmlFor="page-group-label">กลุ่มเมนู</FieldLabel>
                  <Input
                    {...field}
                    id="page-group-label"
                    list="page-group-label-options"
                    placeholder="เช่น การตั้งค่า (เว้นว่างไว้ถ้าไม่อยู่ในกลุ่มไหน)"
                  />
                  <datalist id="page-group-label-options">
                    {existingGroupLabels.map((label) => (
                      <option key={label} value={label} />
                    ))}
                  </datalist>
                </Field>
              )}
            />
            <Controller
              name="sortOrder"
              control={form.control}
              render={({ field: { onChange, value, ...field } }) => (
                <Field>
                  <FieldLabel htmlFor="page-sort-order">ลำดับการแสดงผล</FieldLabel>
                  <Input
                    {...field}
                    id="page-sort-order"
                    type="number"
                    value={value}
                    onChange={(e) => onChange(e.target.valueAsNumber || 0)}
                  />
                </Field>
              )}
            />
            <Controller
              name="permissions"
              control={form.control}
              render={({ field }) => (
                <Field>
                  <FieldLabel>สิทธิ์ที่ต้องมีถึงจะเห็นเพจนี้</FieldLabel>
                  <FieldDescription>
                    ไม่เลือกเลย = หน้าสาธารณะ ทุกคนที่ login แล้วเห็นได้
                  </FieldDescription>
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
