import { useEffect, useMemo, useState } from 'react'
import { Controller, useFieldArray, useForm, useWatch } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { Plus, X } from 'lucide-react'
import * as z from 'zod'

import { Button } from '@/components/ui/button'
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
} from '@/components/ui/combobox'
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
import { Textarea } from '@/components/ui/textarea'
import { ThaiDateInput } from '@/components/ui/thai-date-input'
import { formatThaiDate, todayIso } from '@/lib/date'
import { textIncludes } from '@/lib/search'

import type {
  ReceiptBudgetYearOption,
  ReceiptItemOption,
  ReceiptOptions,
  ReceiptUnitOption,
  StockLot,
  StockReceiptInput,
} from '../stockApi'
import { RECEIPT_TYPE_LABELS, stockApi } from '../stockApi'

// ตรงกับ backend/src/stock/service/receipts.rs และความยาวคอลัมน์ใน migration stock_receipts
const MAX_LINES = 100
const MAX_BASE_QUANTITY = 10_000_000
const MAX_SOURCE_LENGTH = 200
const MAX_REF_NO_LENGTH = 50
const MAX_NOTE_LENGTH = 500
const MAX_LOT_NO_LENGTH = 50

// หน่วยที่เลือกได้ของรายการ: หน่วยฐาน (1 หน่วย = 1 หน่วยฐาน) + หน่วยแปลง
function itemUnits(item: ReceiptItemOption): ReceiptUnitOption[] {
  return [{ id: item.baseUnitId, name: item.baseUnitName, factor: 1 }, ...item.conversions]
}

// ช่องตัวเลขเก็บเป็นสตริงในฟอร์ม ('' = ว่าง) แล้วแปลงตอนส่ง (เหมือนฟอร์มรายการคลัง)
function positiveWholeNumber(value: string): number | null {
  const trimmed = value.trim()
  return /^\d+$/.test(trimmed) && Number(trimmed) > 0 ? Number(trimmed) : null
}

// ล็อตเดียวกันในใบ = รายการเดียวกัน + เลขล็อตเดียวกันไม่สนตัวพิมพ์ (รายการที่ไม่ติดตามล็อตมีล็อตไม่ระบุล็อตเดียว)
function lotKey(item: ReceiptItemOption, lotNo: string): string {
  return item.trackLot ? lotNo.trim().toLowerCase() : ''
}

// ปีงบที่ครอบวันที่ (ตัวเลือกเรียงปีล่าสุดก่อน — ช่วงวันทับกันได้ จึงเลือกปีแรกที่ครอบ แก้เองได้ในฟอร์ม)
function yearContaining(years: ReceiptBudgetYearOption[], date: string) {
  return years.find((year) => year.startDate <= date && date <= year.endDate)
}

const lineSchema = z.object({
  itemId: z.string(),
  unitId: z.string(),
  quantity: z.string(),
  lotNo: z.string(),
  expiryDate: z.string(),
})

// กฎของบรรทัดขึ้นกับธงติดตามของรายการ (มาจาก options) — backend ตรวจซ้ำทุกข้อ
function buildSchema(options: ReceiptOptions | null, today: string) {
  const items = new Map((options?.items ?? []).map((item) => [item.id, item]))
  return z
    .object({
      receiptType: z.enum(['receive', 'opening']),
      budgetYearId: z.string().min(1, 'เลือกปีงบประมาณ'),
      receivedOn: z.string().min(1, 'กรอกวันที่รับ'),
      source: z.string().max(MAX_SOURCE_LENGTH, `ยาวได้ไม่เกิน ${MAX_SOURCE_LENGTH} ตัวอักษร`),
      refNo: z.string().max(MAX_REF_NO_LENGTH, `ยาวได้ไม่เกิน ${MAX_REF_NO_LENGTH} ตัวอักษร`),
      note: z.string().max(MAX_NOTE_LENGTH, `ยาวได้ไม่เกิน ${MAX_NOTE_LENGTH} ตัวอักษร`),
      lines: z
        .array(lineSchema)
        .min(1, 'เพิ่มรายการอย่างน้อย 1 บรรทัด')
        .max(MAX_LINES, `ใบรับหนึ่งใบมีได้ไม่เกิน ${MAX_LINES} บรรทัด`),
    })
    .superRefine((v, ctx) => {
      const issue = (path: (string | number)[], message: string) =>
        ctx.addIssue({ code: 'custom', path, message })
      if (v.receivedOn && v.receivedOn > today) issue(['receivedOn'], 'วันที่รับต้องไม่เกินวันนี้')
      const year = options?.budgetYears.find((b) => b.id === v.budgetYearId)
      if (year && v.receivedOn && (v.receivedOn < year.startDate || v.receivedOn > year.endDate)) {
        issue(['receivedOn'], 'วันที่รับไม่อยู่ในช่วงของปีงบประมาณที่เลือก')
      }

      v.lines.forEach((line, index) => {
        const item = items.get(line.itemId)
        if (!item) {
          issue(['lines', index, 'itemId'], 'เลือกรายการ')
          return
        }
        const unit = itemUnits(item).find((u) => u.id === line.unitId)
        if (!unit) issue(['lines', index, 'unitId'], 'เลือกหน่วย')
        const quantity = positiveWholeNumber(line.quantity)
        if (quantity === null) {
          issue(['lines', index, 'quantity'], 'จำนวนเต็มมากกว่า 0')
        } else if (unit && quantity * unit.factor > MAX_BASE_QUANTITY) {
          issue(
            ['lines', index, 'quantity'],
            `ไม่เกิน ${MAX_BASE_QUANTITY.toLocaleString('th-TH')} ${item.baseUnitName} ต่อบรรทัด`,
          )
        }
        const lotNo = line.lotNo.trim()
        if (item.trackLot && !lotNo) issue(['lines', index, 'lotNo'], 'กรอกเลขล็อต')
        if (lotNo.length > MAX_LOT_NO_LENGTH) {
          issue(['lines', index, 'lotNo'], `ยาวได้ไม่เกิน ${MAX_LOT_NO_LENGTH} ตัวอักษร`)
        }
        if (item.trackExpiry && !line.expiryDate) {
          issue(['lines', index, 'expiryDate'], 'กรอกวันหมดอายุ')
        } else if (item.trackExpiry && v.receivedOn && line.expiryDate < v.receivedOn) {
          issue(['lines', index, 'expiryDate'], 'หมดอายุแล้ว (ก่อนวันที่รับ) รับเข้าไม่ได้')
        }
        const duplicate = v.lines
          .slice(0, index)
          .some((earlier) => earlier.itemId === line.itemId && lotKey(item, earlier.lotNo) === lotKey(item, line.lotNo))
        if (duplicate) {
          issue(
            ['lines', index, item.trackLot ? 'lotNo' : 'itemId'],
            'ล็อตซ้ำกับบรรทัดก่อนหน้า — รวมเป็นบรรทัดเดียว',
          )
        }
      })
    })
}

type ReceiptFormValues = z.infer<ReturnType<typeof buildSchema>>
type LineValues = ReceiptFormValues['lines'][number]

const EMPTY_LINE: LineValues = { itemId: '', unitId: '', quantity: '', lotNo: '', expiryDate: '' }

function emptyValues(options: ReceiptOptions | null, today: string): ReceiptFormValues {
  return {
    receiptType: 'receive',
    budgetYearId: yearContaining(options?.budgetYears ?? [], today)?.id ?? '',
    receivedOn: today,
    source: '',
    refNo: '',
    note: '',
    lines: [EMPTY_LINE],
  }
}

function blankToNull(value: string): string | null {
  const trimmed = value.trim()
  return trimmed === '' ? null : trimmed
}

function toInput(v: ReceiptFormValues, items: ReceiptItemOption[]): StockReceiptInput {
  return {
    receiptType: v.receiptType,
    budgetYearId: v.budgetYearId,
    receivedOn: v.receivedOn,
    source: blankToNull(v.source),
    refNo: blankToNull(v.refNo),
    note: blankToNull(v.note),
    lines: v.lines.map((line) => {
      const item = items.find((i) => i.id === line.itemId)
      return {
        itemId: line.itemId,
        unitId: line.unitId,
        quantity: Number(line.quantity),
        // ช่องที่ซ่อนอยู่ (รายการไม่ติดตาม) ไม่ส่ง — backend ปฏิเสธถ้ากรอกมา
        lotNo: item?.trackLot ? line.lotNo.trim() : null,
        expiryDate: item?.trackExpiry ? line.expiryDate : null,
      }
    }),
  }
}

// "2 กล่อง × 100 = 200 เม็ด" — สูตรแปลงเป็นหน่วยฐานแสดงคู่กับค่าที่กรอกเสมอ ให้ตรวจด้วยตาได้ (CLAUDE.md)
function quantityFormula(item: ReceiptItemOption, unit: ReceiptUnitOption, quantity: number): string {
  const n = (value: number) => value.toLocaleString('th-TH')
  if (unit.factor === 1) return `${n(quantity)} ${item.baseUnitName}`
  return `${n(quantity)} ${unit.name} × ${n(unit.factor)} = ${n(quantity * unit.factor)} ${item.baseUnitName}`
}

interface StockReceiptFormDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  // ตัวเลือกรายการ/ปีงบ/แหล่งที่มา (null = ยังโหลดไม่เสร็จ)
  options: ReceiptOptions | null
  onSubmit: (input: StockReceiptInput) => Promise<void>
}

// ฟอร์มเพิ่มใบรับเข้า/ยอดยกมา: หัวใบ + บรรทัด (รายการ, หน่วย, จำนวน, ล็อต/วันหมดอายุตามการติดตามของรายการ)
// บันทึกแล้วลงคลังทันที แก้ภายหลังได้แค่ข้อความหัวใบ — จำนวน/ล็อตผิดต้องยกเลิกทั้งใบแล้วทำใหม่
export function StockReceiptFormDialog({
  open,
  onOpenChange,
  options,
  onSubmit,
}: StockReceiptFormDialogProps) {
  const today = todayIso()
  const schema = useMemo(() => buildSchema(options, today), [options, today])
  const form = useForm<ReceiptFormValues>({
    resolver: zodResolver(schema),
    defaultValues: emptyValues(null, today),
  })
  const lineRows = useFieldArray({ control: form.control, name: 'lines' })
  // ล็อตเดิมของรายการ (เติมเลขล็อต/วันหมดอายุ) โหลดครั้งแรกที่เลือกรายการนั้นในฟอร์ม
  const [lotsByItem, setLotsByItem] = useState<Record<string, StockLot[]>>({})

  useEffect(() => {
    if (!open || !options) return
    form.reset(emptyValues(options, today))
  }, [open, options, today, form])

  const [receivedOn, budgetYearId, lines] = useWatch({
    control: form.control,
    name: ['receivedOn', 'budgetYearId', 'lines'],
  })
  const items = options?.items ?? []
  const budgetYears = options?.budgetYears ?? []
  const findItem = (id: string) => items.find((item) => item.id === id)
  const selectedYear = budgetYears.find((year) => year.id === budgetYearId)

  const revalidate = { shouldDirty: true, shouldValidate: form.formState.isSubmitted }

  const loadLots = async (itemId: string) => {
    if (lotsByItem[itemId]) return
    try {
      const lots = await stockApi.lots(itemId)
      setLotsByItem((prev) => ({ ...prev, [itemId]: lots }))
    } catch {
      // ไม่มีตัวช่วยเติมล็อตเดิม — ผู้ใช้กรอกเองได้ backend ตรวจวันหมดอายุของล็อตเดิมซ้ำอยู่แล้ว
    }
  }

  const handleItemChange = (index: number, item: ReceiptItemOption | null) => {
    form.setValue(`lines.${index}.unitId`, item?.baseUnitId ?? '', revalidate)
    form.setValue(`lines.${index}.lotNo`, '', revalidate)
    form.setValue(`lines.${index}.expiryDate`, '', revalidate)
    if (item?.trackLot) void loadLots(item.id)
  }

  // พิมพ์/เลือกเลขล็อตเดิมที่ยังมีของ → ใช้วันหมดอายุของล็อตนั้น (ล็อตเดิมที่ยังมีของ วันหมดอายุต้องตรง)
  const handleLotChange = (index: number, item: ReceiptItemOption, lotNo: string) => {
    if (!item.trackExpiry) return
    const existing = (lotsByItem[item.id] ?? []).find(
      (lot) => lot.lotNo.toLowerCase() === lotNo.trim().toLowerCase(),
    )
    if (existing?.expiryDate && existing.qtyOnHand > 0) {
      form.setValue(`lines.${index}.expiryDate`, existing.expiryDate, revalidate)
    }
  }

  const handleSubmit = form.handleSubmit(async (values) => {
    try {
      await onSubmit(toInput(values, items))
      // คงเหลือของล็อตเปลี่ยนแล้ว — โหลดใหม่ตอนเลือกรายการครั้งหน้า
      setLotsByItem({})
      onOpenChange(false)
    } catch (err) {
      form.setError('root', {
        message: err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง',
      })
    }
  })

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>เพิ่มใบรับเข้า</DialogTitle>
        </DialogHeader>
        {options === null ? (
          <p className="py-6 text-sm text-muted-foreground">กำลังโหลดตัวเลือก...</p>
        ) : (
          <form onSubmit={handleSubmit}>
            <FieldGroup>
              <div className="grid grid-cols-1 items-start gap-4 sm:grid-cols-3">
                <Controller
                  name="receiptType"
                  control={form.control}
                  render={({ field }) => (
                    <Field>
                      <FieldLabel htmlFor="receipt-type">ประเภท</FieldLabel>
                      <NativeSelect {...field} id="receipt-type">
                        <option value="receive">{RECEIPT_TYPE_LABELS.receive}</option>
                        <option value="opening">{RECEIPT_TYPE_LABELS.opening}</option>
                      </NativeSelect>
                      {field.value === 'opening' && (
                        <FieldDescription>ของที่มีอยู่แล้วตอนเริ่มใช้ระบบ</FieldDescription>
                      )}
                    </Field>
                  )}
                />
                <Controller
                  name="receivedOn"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="receipt-receivedOn">วันที่รับ</FieldLabel>
                      <ThaiDateInput
                        id="receipt-receivedOn"
                        value={field.value}
                        onChange={(value) => {
                          field.onChange(value)
                          // เติมปีงบจากวันที่ เมื่อปีที่เลือกอยู่ไม่ครอบวันนี้
                          const current = budgetYears.find((y) => y.id === form.getValues('budgetYearId'))
                          if (value && !(current && current.startDate <= value && value <= current.endDate)) {
                            form.setValue('budgetYearId', yearContaining(budgetYears, value)?.id ?? '', revalidate)
                          }
                        }}
                        onBlur={field.onBlur}
                        max={today}
                        aria-invalid={fieldState.invalid}
                      />
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
                <Controller
                  name="budgetYearId"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="receipt-budgetYearId">ปีงบประมาณ</FieldLabel>
                      <NativeSelect {...field} id="receipt-budgetYearId" aria-invalid={fieldState.invalid}>
                        <option value="">— เลือก —</option>
                        {budgetYears.map((year) => (
                          <option key={year.id} value={year.id}>
                            {year.year}
                          </option>
                        ))}
                      </NativeSelect>
                      <FieldDescription>
                        {budgetYears.length === 0
                          ? 'ยังไม่มีปีงบประมาณที่ใช้งานอยู่ — เพิ่มได้ที่ การตั้งค่า > ตั้งค่าปีงบประมาณ'
                          : selectedYear
                            ? `${formatThaiDate(selectedYear.startDate)} – ${formatThaiDate(selectedYear.endDate)} · เลขที่ใบรันต่อปีงบ`
                            : 'เลขที่ใบรันต่อปีงบ'}
                      </FieldDescription>
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
              </div>

              <div className="grid grid-cols-1 items-start gap-4 sm:grid-cols-2">
                <Controller
                  name="source"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="receipt-source">แหล่งที่มา</FieldLabel>
                      <Input
                        {...field}
                        id="receipt-source"
                        list="receipt-source-options"
                        maxLength={MAX_SOURCE_LENGTH}
                        placeholder="เช่น ห้องยา รพ."
                        aria-invalid={fieldState.invalid}
                      />
                      <datalist id="receipt-source-options">
                        {options.sources.map((source) => (
                          <option key={source} value={source} />
                        ))}
                      </datalist>
                      <FieldDescription>เลือกจากที่เคยกรอกได้ ให้สะกดตรงกัน</FieldDescription>
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
                <Controller
                  name="refNo"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="receipt-refNo">เลขที่เอกสารอ้างอิง</FieldLabel>
                      <Input
                        {...field}
                        id="receipt-refNo"
                        maxLength={MAX_REF_NO_LENGTH}
                        placeholder="เช่น เลขที่ใบส่งของ/ใบเบิก"
                        aria-invalid={fieldState.invalid}
                      />
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
              </div>

              <Controller
                name="note"
                control={form.control}
                render={({ field, fieldState }) => (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor="receipt-note">หมายเหตุ</FieldLabel>
                    <Textarea
                      {...field}
                      id="receipt-note"
                      rows={2}
                      maxLength={MAX_NOTE_LENGTH}
                      aria-invalid={fieldState.invalid}
                    />
                    {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                  </Field>
                )}
              />

              <Field data-invalid={Boolean(form.formState.errors.lines?.root ?? form.formState.errors.lines?.message)}>
                <FieldLabel>รายการที่รับ</FieldLabel>
                <FieldDescription>
                  จำนวนเลือกหน่วยได้ (หน่วยฐานหรือหน่วยแปลงของรายการ) ระบบเก็บเป็นหน่วยฐาน
                  {items.length === 0 && ' — ยังไม่มีรายการคลังที่ใช้งานอยู่ เพิ่มได้ที่ การตั้งค่า > ตั้งค่ารายการคลัง'}
                </FieldDescription>
                {lineRows.fields.map((row, index) => {
                  const line = lines[index]
                  const item = line ? findItem(line.itemId) : undefined
                  const unit = item ? itemUnits(item).find((u) => u.id === line?.unitId) : undefined
                  const quantity = line ? positiveWholeNumber(line.quantity) : null
                  const lots = item ? (lotsByItem[item.id] ?? []) : []
                  const typedLot = line?.lotNo.trim().toLowerCase() ?? ''
                  const existingLot = typedLot ? lots.find((lot) => lot.lotNo.toLowerCase() === typedLot) : undefined
                  return (
                    <div key={row.id} className="flex flex-col gap-3 rounded-lg border p-3">
                      <div className="flex items-center justify-between gap-2">
                        <span className="text-sm font-medium">บรรทัดที่ {index + 1}</span>
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon"
                          aria-label={`เอาบรรทัดที่ ${index + 1} ออก`}
                          disabled={lineRows.fields.length === 1}
                          onClick={() => lineRows.remove(index)}
                        >
                          <X />
                        </Button>
                      </div>

                      <Controller
                        name={`lines.${index}.itemId`}
                        control={form.control}
                        render={({ field, fieldState }) => (
                          <Field data-invalid={fieldState.invalid}>
                            <FieldLabel htmlFor={`receipt-line-${index}-item`}>รายการ</FieldLabel>
                            <Combobox
                              items={items}
                              value={findItem(field.value) ?? null}
                              onValueChange={(next) => {
                                field.onChange(next?.id ?? '')
                                handleItemChange(index, next)
                              }}
                              itemToStringLabel={(option) => option.name}
                              isItemEqualToValue={(a, b) => a.id === b.id}
                              filter={(option, query) =>
                                textIncludes(option.name, query) || textIncludes(option.categoryName, query)
                              }
                            >
                              <ComboboxInput
                                id={`receipt-line-${index}-item`}
                                placeholder="พิมพ์เพื่อค้นหารายการ"
                                aria-invalid={fieldState.invalid}
                              />
                              <ComboboxContent>
                                <ComboboxEmpty>ไม่พบรายการที่ค้นหา</ComboboxEmpty>
                                <ComboboxList>
                                  {(option: ReceiptItemOption) => (
                                    <ComboboxItem key={option.id} value={option}>
                                      {option.name}
                                      <span className="text-muted-foreground"> · {option.categoryName}</span>
                                    </ComboboxItem>
                                  )}
                                </ComboboxList>
                              </ComboboxContent>
                            </Combobox>
                            {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                          </Field>
                        )}
                      />

                      <div className="grid grid-cols-2 items-start gap-3">
                        <Controller
                          name={`lines.${index}.unitId`}
                          control={form.control}
                          render={({ field, fieldState }) => (
                            <Field data-invalid={fieldState.invalid}>
                              <FieldLabel htmlFor={`receipt-line-${index}-unit`}>หน่วย</FieldLabel>
                              <NativeSelect
                                {...field}
                                id={`receipt-line-${index}-unit`}
                                disabled={!item}
                                aria-invalid={fieldState.invalid}
                              >
                                {!item && <option value="">— เลือกรายการก่อน —</option>}
                                {item &&
                                  itemUnits(item).map((option) => (
                                    <option key={option.id} value={option.id}>
                                      {option.factor === 1
                                        ? `${option.name} (หน่วยฐาน)`
                                        : `${option.name} (= ${option.factor.toLocaleString('th-TH')} ${item.baseUnitName})`}
                                    </option>
                                  ))}
                              </NativeSelect>
                              {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                            </Field>
                          )}
                        />
                        <Controller
                          name={`lines.${index}.quantity`}
                          control={form.control}
                          render={({ field, fieldState }) => (
                            <Field data-invalid={fieldState.invalid}>
                              <FieldLabel htmlFor={`receipt-line-${index}-quantity`}>จำนวน</FieldLabel>
                              <Input
                                {...field}
                                id={`receipt-line-${index}-quantity`}
                                type="number"
                                inputMode="numeric"
                                min={1}
                                aria-invalid={fieldState.invalid}
                              />
                              {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                            </Field>
                          )}
                        />
                      </div>

                      {item?.trackLot && (
                        <div className="grid grid-cols-1 items-start gap-3 sm:grid-cols-2">
                          <Controller
                            name={`lines.${index}.lotNo`}
                            control={form.control}
                            render={({ field, fieldState }) => (
                              <Field data-invalid={fieldState.invalid}>
                                <FieldLabel htmlFor={`receipt-line-${index}-lot`}>เลขล็อต</FieldLabel>
                                <Input
                                  {...field}
                                  id={`receipt-line-${index}-lot`}
                                  list={`receipt-line-${index}-lots`}
                                  maxLength={MAX_LOT_NO_LENGTH}
                                  onChange={(e) => {
                                    field.onChange(e)
                                    handleLotChange(index, item, e.target.value)
                                  }}
                                  aria-invalid={fieldState.invalid}
                                />
                                <datalist id={`receipt-line-${index}-lots`}>
                                  {lots.map((lot) => (
                                    <option key={lot.lotNo} value={lot.lotNo} />
                                  ))}
                                </datalist>
                                {existingLot ? (
                                  <FieldDescription>
                                    {existingLot.qtyOnHand > 0
                                      ? `ล็อตเดิม คงเหลือ ${existingLot.qtyOnHand.toLocaleString('th-TH')} ${item.baseUnitName}`
                                      : 'ล็อตเดิม (ของหมดแล้ว)'}
                                    {existingLot.expiryDate && ` · หมด ${formatThaiDate(existingLot.expiryDate)}`}
                                  </FieldDescription>
                                ) : typedLot ? (
                                  <FieldDescription>ล็อตใหม่</FieldDescription>
                                ) : null}
                                {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                              </Field>
                            )}
                          />
                          {item.trackExpiry && (
                            <Controller
                              name={`lines.${index}.expiryDate`}
                              control={form.control}
                              render={({ field, fieldState }) => (
                                <Field data-invalid={fieldState.invalid}>
                                  <FieldLabel htmlFor={`receipt-line-${index}-expiry`}>วันหมดอายุ</FieldLabel>
                                  <ThaiDateInput
                                    id={`receipt-line-${index}-expiry`}
                                    value={field.value}
                                    onChange={field.onChange}
                                    onBlur={field.onBlur}
                                    min={receivedOn || undefined}
                                    aria-invalid={fieldState.invalid}
                                  />
                                  {existingLot && existingLot.qtyOnHand > 0 && (
                                    <FieldDescription>ล็อตเดิมที่ยังมีของ ต้องใช้วันหมดอายุเดิม</FieldDescription>
                                  )}
                                  {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                                </Field>
                              )}
                            />
                          )}
                        </div>
                      )}

                      {item && unit && quantity !== null && (
                        <p className="text-sm text-muted-foreground">{quantityFormula(item, unit, quantity)}</p>
                      )}
                    </div>
                  )
                })}
                {form.formState.errors.lines?.root && (
                  <FieldError errors={[form.formState.errors.lines.root]} />
                )}
                {form.formState.errors.lines?.message && (
                  <FieldError>{form.formState.errors.lines.message}</FieldError>
                )}
                <div>
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    disabled={lineRows.fields.length >= MAX_LINES}
                    onClick={() => lineRows.append(EMPTY_LINE)}
                  >
                    <Plus />
                    เพิ่มบรรทัด
                  </Button>
                </div>
              </Field>

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
                {form.formState.isSubmitting ? 'กำลังบันทึก...' : 'บันทึกรับเข้า'}
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  )
}
