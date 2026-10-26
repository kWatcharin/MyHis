import * as React from "react"
import { Popover as PopoverPrimitive } from "@base-ui/react/popover"
import { CalendarDays, ChevronLeft, ChevronRight } from "lucide-react"
import { cn } from "cn"

import { Button } from "@/components/ui/button"
import { InputGroup, InputGroupAddon, InputGroupInput } from "@/components/ui/input-group"
import { NativeSelect } from "@/components/ui/native-select"
import {
  BUDDHIST_ERA_OFFSET,
  THAI_MONTHS_FULL,
  daysInMonth,
  formatThaiDate,
  formatThaiNumericDate,
  parseIsoDate,
  parseThaiNumericDate,
  toIso,
  todayIso,
} from "@/lib/date"

// ช่องวันที่ของทั้งระบบแสดงปี พ.ศ. — <input type="date"> ของเบราว์เซอร์แสดงได้แค่ ค.ศ. จึงทำเอง
// ค่าที่รับ/ส่งยังเป็น "YYYY-MM-DD" ค.ศ. (ว่าง = ไม่มีค่า) เหมือน input type="date" เดิม API ไม่ต้องเปลี่ยน
// - พิมพ์ได้ "วว/ดด/ปปปป" (พ.ศ.) ส่งค่าทันทีที่เป็นวันที่ถูกต้อง พิมพ์ไม่ครบ/ผิดแล้วออกจากช่อง = กลับเป็นค่าเดิม
// - ปุ่มปฏิทินเปิดตารางรายเดือน เลือกเดือน/ปี พ.ศ. ได้ ใช้ลูกศรเลื่อนวัน, PageUp/PageDown เลื่อนเดือน
// - min/max ("YYYY-MM-DD") ปิดวันที่อยู่นอกช่วงทั้งตอนพิมพ์และในปฏิทิน

const WEEKDAYS = ["อา", "จ", "อ", "พ", "พฤ", "ศ", "ส"]
// ช่วงปีในตัวเลือกเมื่อไม่ได้กำหนด min/max (ค.ศ.) — ครอบวันเกิดผู้สูงอายุถึงนัดหมายล่วงหน้า
const DEFAULT_FIRST_YEAR = 1900
const DEFAULT_YEARS_AHEAD = 20

interface ThaiDateInputProps {
  id?: string
  name?: string
  value: string
  onChange: (value: string) => void
  onBlur?: () => void
  min?: string
  max?: string
  disabled?: boolean
  "aria-invalid"?: boolean
  ref?: React.Ref<HTMLInputElement>
  className?: string
}

function inRange(iso: string, min?: string, max?: string): boolean {
  return (!min || iso >= min) && (!max || iso <= max)
}

function shiftDays(iso: string, days: number): string {
  const [year, month, day] = parseIsoDate(iso) ?? [1970, 1, 1]
  const date = new Date(Date.UTC(year, month - 1, day + days))
  return toIso(date.getUTCFullYear(), date.getUTCMonth() + 1, date.getUTCDate())
}

function shiftMonths(iso: string, months: number): string {
  const [year, month, day] = parseIsoDate(iso) ?? [1970, 1, 1]
  const index = year * 12 + (month - 1) + months
  const targetYear = Math.floor(index / 12)
  const targetMonth = (index % 12) + 1
  return toIso(targetYear, targetMonth, Math.min(day, daysInMonth(targetYear, targetMonth)))
}

function clamp(iso: string, min?: string, max?: string): string {
  if (min && iso < min) return min
  if (max && iso > max) return max
  return iso
}

export function ThaiDateInput({
  id,
  name,
  value,
  onChange,
  onBlur,
  min,
  max,
  disabled,
  "aria-invalid": ariaInvalid,
  ref,
  className,
}: ThaiDateInputProps) {
  // ข้อความในช่อง — ซิงก์ใหม่เมื่อค่าเปลี่ยนจากข้างนอก (ไม่ใช่จากที่ผู้ใช้กำลังพิมพ์)
  const [text, setText] = React.useState(() => formatThaiNumericDate(value))
  const [syncedValue, setSyncedValue] = React.useState(value)
  if (value !== syncedValue) {
    setSyncedValue(value)
    setText(formatThaiNumericDate(value))
  }

  const [open, setOpen] = React.useState(false)
  // เดือนที่ปฏิทินแสดง + วันที่โฟกัสอยู่ (roving tabindex) — ทั้งคู่เป็น "YYYY-MM-DD"
  const [focused, setFocused] = React.useState(() => clamp(value || todayIso(), min, max))
  const popupRef = React.useRef<HTMLDivElement>(null)
  const moveFocus = React.useRef(false)

  React.useEffect(() => {
    if (!moveFocus.current) return
    moveFocus.current = false
    popupRef.current?.querySelector<HTMLButtonElement>(`[data-iso="${focused}"]`)?.focus()
  }, [focused])

  const commit = (iso: string) => {
    setSyncedValue(iso)
    onChange(iso)
  }

  const handleText = (next: string) => {
    setText(next)
    if (next.trim() === "") {
      if (value !== "") commit("")
      return
    }
    const iso = parseThaiNumericDate(next)
    if (iso && iso !== value && inRange(iso, min, max)) commit(iso)
  }

  const handleOpenChange = (next: boolean) => {
    if (next) setFocused(clamp(value || todayIso(), min, max))
    setOpen(next)
  }

  const pick = (iso: string) => {
    setText(formatThaiNumericDate(iso))
    commit(iso)
    setOpen(false)
  }

  const [viewYear, viewMonth] = parseIsoDate(focused) ?? [1970, 1]
  const firstYear = min ? parseIsoDate(min)?.[0] ?? DEFAULT_FIRST_YEAR : DEFAULT_FIRST_YEAR
  const lastYear = max
    ? parseIsoDate(max)?.[0] ?? new Date().getFullYear() + DEFAULT_YEARS_AHEAD
    : new Date().getFullYear() + DEFAULT_YEARS_AHEAD
  const years = Array.from({ length: lastYear - firstYear + 1 }, (_, i) => lastYear - i)
  const leading = new Date(Date.UTC(viewYear, viewMonth - 1, 1)).getUTCDay()
  const cells: (string | null)[] = [
    ...Array.from({ length: leading }, () => null),
    ...Array.from({ length: daysInMonth(viewYear, viewMonth) }, (_, i) => toIso(viewYear, viewMonth, i + 1)),
  ]
  const today = todayIso()

  const showMonth = (year: number, month: number) => {
    const day = Math.min(parseIsoDate(focused)?.[2] ?? 1, daysInMonth(year, month))
    setFocused(clamp(toIso(year, month, day), min, max))
  }

  const handleGridKey = (event: React.KeyboardEvent) => {
    const steps: Record<string, () => string> = {
      ArrowLeft: () => shiftDays(focused, -1),
      ArrowRight: () => shiftDays(focused, 1),
      ArrowUp: () => shiftDays(focused, -7),
      ArrowDown: () => shiftDays(focused, 7),
      PageUp: () => shiftMonths(focused, -1),
      PageDown: () => shiftMonths(focused, 1),
    }
    const step = steps[event.key]
    if (!step) return
    event.preventDefault()
    moveFocus.current = true
    setFocused(clamp(step(), min, max))
  }

  return (
    <PopoverPrimitive.Root open={open} onOpenChange={handleOpenChange}>
      <InputGroup className={className} data-disabled={disabled || undefined}>
        <InputGroupInput
          ref={ref}
          id={id}
          name={name}
          value={text}
          inputMode="numeric"
          autoComplete="off"
          placeholder="วว/ดด/ปปปป (พ.ศ.)"
          disabled={disabled}
          aria-invalid={ariaInvalid}
          onChange={(event) => handleText(event.target.value)}
          onBlur={() => {
            setText(formatThaiNumericDate(value))
            onBlur?.()
          }}
        />
        <InputGroupAddon align="inline-end">
          <PopoverPrimitive.Trigger
            disabled={disabled}
            aria-label={value ? `เลือกวันที่ (ตอนนี้ ${formatThaiDate(value)})` : "เลือกวันที่"}
            className="flex size-6 items-center justify-center rounded-[calc(var(--radius)-3px)] text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50"
          >
            <CalendarDays className="size-4" />
          </PopoverPrimitive.Trigger>
        </InputGroupAddon>
      </InputGroup>
      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Positioner className="isolate z-50 outline-none" side="bottom" align="end" sideOffset={4}>
          <PopoverPrimitive.Popup
            ref={popupRef}
            initialFocus={() => popupRef.current?.querySelector<HTMLElement>('[data-day][tabindex="0"]') ?? true}
            aria-label="ปฏิทิน (ปี พ.ศ.)"
            className="z-50 w-72 origin-(--transform-origin) rounded-lg bg-popover p-3 text-popover-foreground shadow-md ring-1 ring-foreground/10 outline-none duration-100 data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95"
          >
            <div className="flex items-center gap-1">
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="เดือนก่อนหน้า"
                onClick={() => setFocused(clamp(shiftMonths(focused, -1), min, max))}
              >
                <ChevronLeft />
              </Button>
              <NativeSelect
                aria-label="เดือน"
                value={viewMonth}
                onChange={(event) => showMonth(viewYear, Number(event.target.value))}
                className="h-7 flex-1"
              >
                {THAI_MONTHS_FULL.map((label, index) => (
                  <option key={label} value={index + 1}>
                    {label}
                  </option>
                ))}
              </NativeSelect>
              <NativeSelect
                aria-label="ปี พ.ศ."
                value={viewYear}
                onChange={(event) => showMonth(Number(event.target.value), viewMonth)}
                className="h-7 w-[5.5rem] tabular-nums"
              >
                {years.map((year) => (
                  <option key={year} value={year}>
                    {year + BUDDHIST_ERA_OFFSET}
                  </option>
                ))}
              </NativeSelect>
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="เดือนถัดไป"
                onClick={() => setFocused(clamp(shiftMonths(focused, 1), min, max))}
              >
                <ChevronRight />
              </Button>
            </div>

            {/* ปุ่มวันมี aria-label เป็นวันที่เต็ม (พ.ศ.) จึงซ่อนหัววันในสัปดาห์จาก screen reader */}
            <div
              role="group"
              aria-label={`${THAI_MONTHS_FULL[viewMonth - 1]} ${viewYear + BUDDHIST_ERA_OFFSET}`}
              onKeyDown={handleGridKey}
              className="mt-2"
            >
              <div aria-hidden className="grid grid-cols-7">
                {WEEKDAYS.map((weekday) => (
                  <span key={weekday} className="py-1 text-center text-xs font-medium text-muted-foreground">
                    {weekday}
                  </span>
                ))}
              </div>
              <div className="grid grid-cols-7 gap-y-0.5">
                {cells.map((iso, index) =>
                  iso === null ? (
                    <span key={`blank-${index}`} aria-hidden />
                  ) : (
                    <button
                      key={iso}
                      type="button"
                      data-day
                      data-iso={iso}
                      tabIndex={iso === focused ? 0 : -1}
                      disabled={!inRange(iso, min, max)}
                      aria-pressed={iso === value}
                      aria-label={formatThaiDate(iso)}
                      aria-current={iso === today ? "date" : undefined}
                      onClick={() => pick(iso)}
                      onFocus={() => setFocused(iso)}
                      className={cn(
                        "mx-auto flex size-8 items-center justify-center rounded-md text-sm tabular-nums outline-none",
                        "hover:bg-muted focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-35",
                        iso === today && "font-semibold text-primary ring-1 ring-primary/40",
                        iso === value && "bg-primary font-semibold text-primary-foreground hover:bg-primary",
                      )}
                    >
                      {Number(iso.slice(8))}
                    </button>
                  ),
                )}
              </div>
            </div>

            <div className="mt-2 flex items-center justify-between border-t pt-2">
              <Button
                variant="ghost"
                size="sm"
                disabled={!inRange(today, min, max)}
                onClick={() => pick(today)}
              >
                วันนี้
              </Button>
              {value && (
                <Button
                  variant="ghost"
                  size="sm"
                  className="text-muted-foreground"
                  onClick={() => {
                    setText("")
                    commit("")
                    setOpen(false)
                  }}
                >
                  ล้างวันที่
                </Button>
              )}
            </div>
          </PopoverPrimitive.Popup>
        </PopoverPrimitive.Positioner>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  )
}
