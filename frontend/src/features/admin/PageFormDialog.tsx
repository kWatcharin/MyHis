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
import { NativeSelect } from '@/components/ui/native-select'
import { useAuth } from '@/features/auth/AuthContext'
import { resolveIcon } from '@/lib/icons'

import { ACTION_LABELS, ALL_PERMISSIONS, parsePermission, resourceLabel } from './permissions'
import type { Page } from './pagesApi'

// เมนู 2 ชั้น: route ว่าง = เพจหลักที่เป็นหัวข้อกลุ่ม (ไม่มีหน้าของตัวเอง) refId ว่าง = เป็นเพจหลักเอง
// ไม่มีช่องลำดับ — จัดลำดับด้วยการลากวางในหน้ารายการ เพจใหม่/เพจที่ย้ายกลุ่มไปต่อท้ายกลุ่ม (backend ตั้งเอง)
// เพจย่อย (เลือกเพจหลัก) ต้องมี route — backend ตรวจกฎโครงเมนูซ้ำอีกชั้นเสมอ
const pageFormSchema = z
  .object({
    label: z.string().min(1, 'กรอกชื่อเพจ'),
    icon: z.string().optional(),
    route: z.string().trim().regex(/^(\/.*)?$/, 'route ต้องขึ้นต้นด้วย /'),
    permissions: z.array(z.string()),
    refId: z.string(),
  })
  .refine((v) => v.refId === '' || v.route !== '', {
    path: ['route'],
    message: 'เพจย่อยต้องมี route',
  })

// route ว่าง / refId ว่าง แปลงเป็น body ของ API ที่ toPageInput (PagesSettingsPage.tsx)
export type PageFormValues = z.infer<typeof pageFormSchema>

interface PageFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: 'create' | 'edit'
  initialValue?: Page
  onSubmit: (values: PageFormValues) => Promise<void>
  // เพจหลักที่เป็นหัวข้อกลุ่ม (refId null และไม่มี route) — ตัวเลือกของช่อง "เพจหลัก" (ตัดเพจที่กำลังแก้ออกเอง)
  groupPages: Page[]
}

const EMPTY_VALUES: PageFormValues = {
  label: '',
  icon: '',
  route: '',
  permissions: [],
  refId: '',
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
  groupPages,
}: PageFormDialogProps) {
  const form = useForm<PageFormValues>({
    resolver: zodResolver(pageFormSchema),
    defaultValues: EMPTY_VALUES,
  })

  useEffect(() => {
    if (!open) return
    form.reset(
      mode === 'edit' && initialValue
        ? {
            label: initialValue.label,
            icon: initialValue.icon ?? '',
            route: initialValue.route ?? '',
            permissions: initialValue.permissions,
            refId: initialValue.refId ?? '',
          }
        : EMPTY_VALUES,
    )
  }, [open, mode, initialValue, form])

  const iconValue = form.watch('icon')
  const IconPreview = resolveIcon(iconValue || null)
  const mainPageOptions = groupPages.filter((page) => page.id !== initialValue?.id)

  // การเห็นเพจในเมนู = สิทธิ์ "ดู" ของเพจไหนก็ได้ (ชื่อตามเมนู) — คีย์อื่นที่เพจนี้ผูกไว้แล้ว (เช่น ที่ migration แปลงมา)
  // แสดงด้วยให้เอาออกได้ ไม่หายเงียบตอนบันทึก (PATCH แทนที่ทั้งชุด)
  const { menu } = useAuth()
  const viewPermissions = ALL_PERMISSIONS.filter((key) => key.endsWith('.view'))
  const visibilityOptions = (current: string[]) => [
    ...viewPermissions,
    ...current.filter((key) => !viewPermissions.includes(key)),
  ]
  const labelOf = (key: string) => {
    const parsed = parsePermission(key)
    return parsed ? `${resourceLabel(parsed.resource, menu)}: ${ACTION_LABELS[parsed.action]}` : key
  }

  const handleSubmit = form.handleSubmit(async (values) => {
    // PATCH ล้าง route ไม่ได้ — เพจที่มีหน้าของตัวเองอยู่แล้วเปลี่ยนเป็นหัวข้อกลุ่มไม่ได้
    if (mode === 'edit' && initialValue?.route && values.route === '') {
      form.setError('route', { message: 'เพจนี้มี route อยู่แล้ว ล้างเป็นค่าว่างไม่ได้' })
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
      {/* เนื้อหายาว (ช่องเพจหลัก + รายการสิทธิ์) เกินจอได้ — จำกัดความสูงแล้วเลื่อนในกรอบ dialog แบบ dialog อื่น */}
      <DialogContent className="max-h-[90vh] overflow-y-auto">
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
                    ชื่อ slug ของ lucide เช่น home, layout-list, shield-check — ดูรายชื่อไอคอนทั้งหมดได้ที่{' '}
                    {/* แท็บใหม่ ไม่ส่ง opener/referrer ไปเว็บภายนอก */}
                    <a
                      href="https://lucide.dev/icons/"
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-primary underline underline-offset-2"
                    >
                      lucide.dev/icons
                    </a>
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
                  <FieldDescription>
                    เว้นว่าง = เพจหลักที่เป็นหัวข้อกลุ่มในเมนู (กดพับ/กางดูเพจย่อย ไม่มีหน้าของตัวเอง)
                  </FieldDescription>
                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                </Field>
              )}
            />
            <Controller
              name="refId"
              control={form.control}
              render={({ field }) => (
                <Field>
                  <FieldLabel htmlFor="page-ref-id">เพจหลัก</FieldLabel>
                  <NativeSelect {...field} id="page-ref-id">
                    <option value="">— ไม่มี (เป็นเพจหลักในเมนูชั้นบน) —</option>
                    {mainPageOptions.map((page) => (
                      <option key={page.id} value={page.id}>
                        {page.label}
                      </option>
                    ))}
                  </NativeSelect>
                  <FieldDescription>
                    เลือกได้เฉพาะเพจหลักที่เป็นหัวข้อกลุ่ม (ไม่มี route) — เมนูมี 2 ชั้น
                  </FieldDescription>
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
                    มีสิทธิ์ข้อใดข้อหนึ่งก็เห็น — ไม่เลือกเลย = หน้าสาธารณะ ทุกคนที่ login แล้วเห็นได้
                  </FieldDescription>
                  <div className="flex flex-col gap-2">
                    {visibilityOptions(field.value).map((permission) => (
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
                        {labelOf(permission)}
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
