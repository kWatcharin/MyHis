import { useEffect, useMemo } from 'react'
import { Controller, useFieldArray, useForm, useWatch } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { Plus, X } from 'lucide-react'
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
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { NativeSelect } from '@/components/ui/native-select'

import type { Item, ItemInput, ItemOptions } from './itemsApi'

// ตรงกับ backend/src/item/service.rs: ชื่อไม่เกิน 100 ตัวอักษร, หน่วยแปลงไม่เกิน 10 หน่วยต่อรายการ
const MAX_NAME_LENGTH = 100
const MAX_CONVERSIONS = 10
// integer ของ Postgres — ค่าที่เกินนี้ backend รับไม่ได้
const MAX_INT = 2_147_483_647

// ช่องตัวเลขเก็บเป็นสตริงในฟอร์ม ('' = ว่าง) แล้วแปลงตอนส่ง (เหมือนช่องลำดับของฟอร์มนัดหมายแล็ป)
function isWholeNumber(value: string): boolean {
  const trimmed = value.trim()
  return /^\d+$/.test(trimmed) && Number(trimmed) <= MAX_INT
}

const itemFormSchema = z
  .object({
    name: z
      .string()
      .trim()
      .min(1, 'กรอกชื่อรายการ')
      .max(MAX_NAME_LENGTH, `ชื่อรายการยาวได้ไม่เกิน ${MAX_NAME_LENGTH} ตัวอักษร`),
    categoryId: z.string().min(1, 'เลือกหมวด'),
    baseUnitId: z.string().min(1, 'เลือกหน่วยฐาน'),
    trackLot: z.boolean(),
    trackExpiry: z.boolean(),
    patientLinkable: z.boolean(),
    reorderPoint: z.string(),
    isActive: z.boolean(),
    conversions: z
      .array(z.object({ unitId: z.string().min(1, 'เลือกหน่วย'), factor: z.string() }))
      .max(MAX_CONVERSIONS, `หน่วยแปลงได้ไม่เกิน ${MAX_CONVERSIONS} หน่วย`),
  })
  .superRefine((v, ctx) => {
    // วันหมดอายุเก็บแยกตามล็อต (backend ตรวจซ้ำ + CHECK ใน DB)
    if (v.trackExpiry && !v.trackLot) {
      ctx.addIssue({
        code: 'custom',
        path: ['trackExpiry'],
        message: 'ติดตามวันหมดอายุต้องติดตามล็อตด้วย',
      })
    }
    if (v.reorderPoint.trim() !== '' && !isWholeNumber(v.reorderPoint)) {
      ctx.addIssue({
        code: 'custom',
        path: ['reorderPoint'],
        message: 'จุดสั่งซื้อต้องเป็นจำนวนเต็มตั้งแต่ 0 ขึ้นไป',
      })
    }
    v.conversions.forEach((conversion, index) => {
      if (conversion.unitId && conversion.unitId === v.baseUnitId) {
        ctx.addIssue({
          code: 'custom',
          path: ['conversions', index, 'unitId'],
          message: 'ต้องไม่ใช่หน่วยฐาน',
        })
      } else if (
        conversion.unitId &&
        v.conversions.findIndex((c) => c.unitId === conversion.unitId) !== index
      ) {
        ctx.addIssue({
          code: 'custom',
          path: ['conversions', index, 'unitId'],
          message: 'หน่วยซ้ำกับแถวก่อนหน้า',
        })
      }
      if (!isWholeNumber(conversion.factor) || Number(conversion.factor) <= 1) {
        ctx.addIssue({
          code: 'custom',
          path: ['conversions', index, 'factor'],
          message: 'ต้องเป็นจำนวนเต็มมากกว่า 1',
        })
      }
    })
  })

type ItemFormValues = z.infer<typeof itemFormSchema>

const EMPTY_VALUES: ItemFormValues = {
  name: '',
  categoryId: '',
  baseUnitId: '',
  trackLot: false,
  trackExpiry: false,
  patientLinkable: false,
  reorderPoint: '',
  isActive: true,
  conversions: [],
}

function valuesFromItem(item: Item): ItemFormValues {
  return {
    name: item.name,
    categoryId: item.categoryId,
    baseUnitId: item.baseUnitId,
    trackLot: item.trackLot,
    trackExpiry: item.trackExpiry,
    patientLinkable: item.patientLinkable,
    reorderPoint: item.reorderPoint === null ? '' : String(item.reorderPoint),
    isActive: item.isActive,
    conversions: item.conversions.map((c) => ({ unitId: c.unitId, factor: String(c.factor) })),
  }
}

function toInput(v: ItemFormValues): ItemInput {
  return {
    name: v.name.trim(),
    categoryId: v.categoryId,
    baseUnitId: v.baseUnitId,
    trackLot: v.trackLot,
    trackExpiry: v.trackExpiry,
    patientLinkable: v.patientLinkable,
    reorderPoint: v.reorderPoint.trim() === '' ? null : Number(v.reorderPoint),
    isActive: v.isActive,
    conversions: v.conversions.map((c) => ({ unitId: c.unitId, factor: Number(c.factor) })),
  }
}

interface Choice {
  id: string
  name: string
  // ไม่อยู่ในตัวเลือกที่ใช้งานอยู่ = ถูกปิดใช้งานหลังจากรายการนี้ผูกไว้
  inactive: boolean
}

// ตัวเลือก = ที่ใช้งานอยู่ + ค่าที่รายการที่แก้อยู่ผูกไว้ (แม้ถูกปิดใช้งานภายหลัง ไม่งั้นค่าหายเงียบ ๆ)
function withCurrent(active: { id: string; name: string }[], current: { id: string; name: string }[]): Choice[] {
  const choices: Choice[] = active.map((c) => ({ id: c.id, name: c.name, inactive: false }))
  for (const c of current) {
    if (!choices.some((choice) => choice.id === c.id)) choices.push({ ...c, inactive: true })
  }
  return choices
}

function choiceLabel(choice: Choice): string {
  return choice.inactive ? `${choice.name} (ปิดใช้งาน)` : choice.name
}

interface ItemFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  mode: 'create' | 'edit'
  initialValue?: Item
  // ตัวเลือกหมวด/หน่วย (null = ยังโหลดไม่เสร็จ)
  options: ItemOptions | null
  onSubmit: (input: ItemInput) => Promise<void>
}

// ฟอร์มเพิ่ม/แก้ไขรายการคลัง: ชื่อ + หมวด + หน่วยฐาน + การติดตาม + จุดสั่งซื้อ + หน่วยแปลง
// เพิ่มใหม่: เลือกหมวดแล้วติ๊กการติดตามตามค่าตั้งต้นของหมวดให้ (แก้ต่อได้) — แก้ไข: ไม่เปลี่ยนตามหมวด
export function ItemFormDialog({
  open,
  onOpenChange,
  mode,
  initialValue,
  options,
  onSubmit,
}: ItemFormDialogProps) {
  const form = useForm<ItemFormValues>({
    resolver: zodResolver(itemFormSchema),
    defaultValues: EMPTY_VALUES,
  })
  const conversionRows = useFieldArray({ control: form.control, name: 'conversions' })

  useEffect(() => {
    if (!open) return
    form.reset(mode === 'edit' && initialValue ? valuesFromItem(initialValue) : EMPTY_VALUES)
  }, [open, mode, initialValue, form])

  const [baseUnitId, conversions] = useWatch({
    control: form.control,
    name: ['baseUnitId', 'conversions'],
  })

  const categories = useMemo(
    () =>
      withCurrent(
        options?.categories ?? [],
        initialValue ? [{ id: initialValue.categoryId, name: initialValue.categoryName }] : [],
      ),
    [options, initialValue],
  )
  const units = useMemo(
    () =>
      withCurrent(
        options?.units ?? [],
        initialValue
          ? [
              { id: initialValue.baseUnitId, name: initialValue.baseUnitName },
              ...initialValue.conversions.map((c) => ({ id: c.unitId, name: c.unitName })),
            ]
          : [],
      ),
    [options, initialValue],
  )
  const unitName = (id: string) => units.find((u) => u.id === id)?.name
  const baseUnitName = unitName(baseUnitId)

  const revalidate = { shouldDirty: true, shouldValidate: form.formState.isSubmitted }

  const handleSubmit = form.handleSubmit(async (values) => {
    try {
      await onSubmit(toInput(values))
      onOpenChange(false)
    } catch (err) {
      form.setError('root', {
        message: err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง',
      })
    }
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>{mode === 'create' ? 'เพิ่มรายการคลัง' : 'แก้ไขรายการคลัง'}</DialogTitle>
        </DialogHeader>
        {options === null ? (
          <p className="py-6 text-sm text-muted-foreground">กำลังโหลดตัวเลือก...</p>
        ) : (
          <form onSubmit={handleSubmit}>
            <FieldGroup>
              <Controller
                name="name"
                control={form.control}
                render={({ field, fieldState }) => (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor="item-name">ชื่อรายการ</FieldLabel>
                    <Input
                      {...field}
                      id="item-name"
                      maxLength={MAX_NAME_LENGTH}
                      placeholder="เช่น พาราเซตามอล 500 mg"
                      aria-invalid={fieldState.invalid}
                    />
                    <FieldDescription>ชื่อซ้ำกันไม่ได้ทั้งระบบ (ไม่สนตัวพิมพ์เล็ก/ใหญ่)</FieldDescription>
                    {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                  </Field>
                )}
              />

              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <Controller
                  name="categoryId"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="item-categoryId">หมวด</FieldLabel>
                      <NativeSelect
                        {...field}
                        id="item-categoryId"
                        aria-invalid={fieldState.invalid}
                        onChange={(e) => {
                          field.onChange(e)
                          // เพิ่มใหม่เท่านั้น: ติ๊กการติดตามตามค่าตั้งต้นของหมวด
                          if (mode !== 'create') return
                          const category = options?.categories.find((c) => c.id === e.target.value)
                          if (!category) return
                          form.setValue('trackLot', category.defaultTrackLot, revalidate)
                          form.setValue('trackExpiry', category.defaultTrackExpiry, revalidate)
                          form.setValue('patientLinkable', category.defaultPatientLinkable, revalidate)
                        }}
                      >
                        <option value="">— เลือก —</option>
                        {categories.map((category) => (
                          <option key={category.id} value={category.id}>
                            {choiceLabel(category)}
                          </option>
                        ))}
                      </NativeSelect>
                      {categories.length === 0 && (
                        <FieldDescription>
                          ยังไม่มีหมวด — เพิ่มได้ที่ การตั้งค่า &gt; ตั้งค่าหมวดคลัง
                        </FieldDescription>
                      )}
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
                <Controller
                  name="baseUnitId"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="item-baseUnitId">หน่วยฐาน</FieldLabel>
                      <NativeSelect {...field} id="item-baseUnitId" aria-invalid={fieldState.invalid}>
                        <option value="">— เลือก —</option>
                        {units.map((unit) => (
                          <option key={unit.id} value={unit.id}>
                            {choiceLabel(unit)}
                          </option>
                        ))}
                      </NativeSelect>
                      <FieldDescription>
                        {units.length === 0
                          ? 'ยังไม่มีหน่วยนับ — เพิ่มได้ที่ การตั้งค่า > ตั้งค่าหน่วยนับ'
                          : 'หน่วยเล็กสุดที่เบิกจริง จำนวนทุกช่องของรายการนี้นับเป็นหน่วยนี้ — เลือกให้ถูกตั้งแต่แรก'}
                      </FieldDescription>
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
              </div>

              <Field>
                <FieldLabel>การติดตาม</FieldLabel>
                <div className="flex flex-col gap-2">
                  <Controller
                    name="trackLot"
                    control={form.control}
                    render={({ field }) => (
                      <label className="flex items-center gap-2 text-sm font-normal">
                        <Checkbox
                          checked={field.value}
                          onCheckedChange={(checked) => {
                            field.onChange(checked === true)
                            // เลิกติดตามล็อต = เลิกติดตามวันหมดอายุด้วย (วันหมดอายุเก็บที่ล็อต)
                            if (checked !== true) form.setValue('trackExpiry', false, revalidate)
                          }}
                        />
                        ติดตามล็อต
                      </label>
                    )}
                  />
                  <Controller
                    name="trackExpiry"
                    control={form.control}
                    render={({ field, fieldState }) => (
                      <>
                        <label className="flex items-center gap-2 text-sm font-normal">
                          <Checkbox
                            checked={field.value}
                            onCheckedChange={(checked) => {
                              field.onChange(checked === true)
                              if (checked === true) form.setValue('trackLot', true, revalidate)
                            }}
                          />
                          ติดตามวันหมดอายุ (ต้องติดตามล็อตด้วย)
                        </label>
                        {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                      </>
                    )}
                  />
                  <Controller
                    name="patientLinkable"
                    control={form.control}
                    render={({ field }) => (
                      <label className="flex items-center gap-2 text-sm font-normal">
                        <Checkbox
                          checked={field.value}
                          onCheckedChange={(checked) => field.onChange(checked === true)}
                        />
                        ผูกผู้ป่วยตอนเบิกได้ (ระบุหรือไม่ก็ได้)
                      </label>
                    )}
                  />
                </div>
                {mode === 'create' && (
                  <FieldDescription>ค่าเริ่มต้นมาจากหมวดที่เลือก แก้ได้</FieldDescription>
                )}
              </Field>

              <Field>
                <FieldLabel>หน่วยแปลง</FieldLabel>
                <FieldDescription>
                  จำนวนหน่วยฐานต่อ 1 หน่วยนั้น เทียบหน่วยฐานเสมอ เช่น 1 แผง = 10 เม็ด, 1 กล่อง = 100 เม็ด
                </FieldDescription>
                {conversionRows.fields.map((row, index) => {
                  const current = conversions[index]
                  const factor = Number(current?.factor)
                  const rowUnitName = current ? unitName(current.unitId) : undefined
                  return (
                    <div key={row.id} className="flex flex-col gap-1">
                      <div className="grid grid-cols-[minmax(0,1fr)_7rem_auto] items-start gap-2">
                        <Controller
                          name={`conversions.${index}.unitId`}
                          control={form.control}
                          render={({ field, fieldState }) => (
                            <Field data-invalid={fieldState.invalid}>
                              <NativeSelect
                                {...field}
                                aria-label={`หน่วยแปลงแถวที่ ${index + 1}`}
                                aria-invalid={fieldState.invalid}
                              >
                                <option value="">— เลือกหน่วย —</option>
                                {units.map((unit) => (
                                  <option key={unit.id} value={unit.id}>
                                    {choiceLabel(unit)}
                                  </option>
                                ))}
                              </NativeSelect>
                              {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                            </Field>
                          )}
                        />
                        <Controller
                          name={`conversions.${index}.factor`}
                          control={form.control}
                          render={({ field, fieldState }) => (
                            <Field data-invalid={fieldState.invalid}>
                              <Input
                                {...field}
                                type="number"
                                inputMode="numeric"
                                min={2}
                                placeholder="จำนวน"
                                aria-label={`จำนวนหน่วยฐานต่อ 1 หน่วย แถวที่ ${index + 1}`}
                                aria-invalid={fieldState.invalid}
                              />
                              {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                            </Field>
                          )}
                        />
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          aria-label={`เอาหน่วยแปลงแถวที่ ${index + 1} ออก`}
                          onClick={() => conversionRows.remove(index)}
                        >
                          <X />
                        </Button>
                      </div>
                      {/* แสดงสูตรแปลงคู่กับค่าที่กรอกเสมอ ให้ตรวจด้วยตาได้ (CLAUDE.md) */}
                      {rowUnitName && baseUnitName && Number.isInteger(factor) && factor > 1 && (
                        <p className="text-xs text-muted-foreground">
                          1 {rowUnitName} = {factor.toLocaleString('th-TH')} {baseUnitName}
                        </p>
                      )}
                    </div>
                  )
                })}
                <div>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={conversionRows.fields.length >= MAX_CONVERSIONS}
                    onClick={() => conversionRows.append({ unitId: '', factor: '' })}
                  >
                    <Plus />
                    เพิ่มหน่วยแปลง
                  </Button>
                </div>
              </Field>

              <div className="grid grid-cols-1 items-start gap-4 sm:grid-cols-2">
                <Controller
                  name="reorderPoint"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="item-reorderPoint">
                        จุดสั่งซื้อ ({baseUnitName ?? 'หน่วยฐาน'})
                      </FieldLabel>
                      <Input
                        {...field}
                        id="item-reorderPoint"
                        type="number"
                        inputMode="numeric"
                        min={0}
                        aria-invalid={fieldState.invalid}
                      />
                      <FieldDescription>เตือนเมื่อคงเหลือ ≤ ค่านี้ — เว้นว่าง = ไม่ตั้ง</FieldDescription>
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
                <Controller
                  name="isActive"
                  control={form.control}
                  render={({ field }) => (
                    <Field>
                      <FieldLabel>สถานะ</FieldLabel>
                      <label className="flex items-center gap-2 text-sm font-normal">
                        <Checkbox
                          checked={field.value}
                          onCheckedChange={(checked) => field.onChange(checked === true)}
                        />
                        ใช้งานอยู่
                      </label>
                    </Field>
                  )}
                />
              </div>

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
        )}
      </DialogContent>
    </Dialog>
  )
}
