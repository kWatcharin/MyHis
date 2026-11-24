import { useEffect, useRef } from 'react'
import type { CSSProperties } from 'react'

import { BrandMark } from '@/components/layout/BrandMark'
import { cn } from '@/lib/utils'

// แถบ "จอ monitor" หัวการ์ดหน้าทางเข้า (login, register) — หัวใจเต้นพร้อมเส้น ECG ที่เลื่อนไปทางซ้าย บนพื้นกระดาษกราฟ ECG
// สีกลางของธีม (bg-ecg-grid) เป็นของตกแต่ง (aria-hidden) แต่จังหวะ/สีของเส้นบอกสถานะของฟอร์ม:
//   rest   = จังหวะปกติ เลื่อนช้า ๆ       busy = กำลังส่งฟอร์ม เต้นเร็วขึ้น
//   locked = ถูกล็อก เต้นช้าและหม่นลง (ไม่ใช้เส้นแบน — ความหมายผิดในงานพยาบาล)
//   alertKey เปลี่ยน (login ไม่ผ่าน) → เส้นกะพริบสี destructive ~1.6 วินาทีแล้วกลับสีเดิม
// prefers-reduced-motion → เส้นนิ่ง หัวใจไม่เต้น (สีกะพริบยังอยู่ — เป็นการบอกสถานะ ไม่ใช่การเคลื่อนที่)
// สีทั้งหมดจาก token ของธีม — แนวทางหน้า "ทางเข้า" อยู่ใน docs/ui-3d-backdrop.md

export type VitalsMode = 'rest' | 'busy' | 'locked'

// วินาทีต่อหนึ่งจังหวะ (เส้นเลื่อนหนึ่งช่วงคลื่นต่อหนึ่งจังหวะ หัวใจเต้นหนึ่งครั้ง) — ผู้ใช้ขอให้ช้าลง (2026-11-24)
// ให้ตามองแล้วสบาย ไม่ใช่อัตราการเต้นของหัวใจจริง
const BEAT_SECONDS: Record<VitalsMode, number> = { rest: 1.5, busy: 0.75, locked: 2.6 }

const TONE: Record<VitalsMode, string> = {
  rest: 'text-primary',
  busy: 'text-primary',
  locked: 'text-muted-foreground',
}

const ALERT_MS = 1600

// หนึ่งช่วงคลื่นกว้าง 100 หน่วย สูง 40 (เส้นฐาน y = 24): P — QRS — T แล้วเส้นฐานยาว
const BEAT_WIDTH = 100
const BEATS = 6
function beatPath(offset: number): string {
  const x = (n: number) => offset + n
  return [
    `L${x(16)},24`,
    `C${x(18)},24 ${x(19)},20 ${x(22)},20`,
    `C${x(25)},20 ${x(26)},24 ${x(28)},24`,
    `L${x(35)},24 L${x(37.5)},27.5 L${x(41.5)},4 L${x(45.5)},34 L${x(48.5)},24`,
    `L${x(57)},24`,
    `C${x(61)},24 ${x(63)},16.5 ${x(68)},16.5`,
    `C${x(73)},16.5 ${x(75)},24 ${x(79)},24`,
    `L${x(BEAT_WIDTH)},24`,
  ].join(' ')
}
const TRACE = `M0,24 ${Array.from({ length: BEATS }, (_, i) => beatPath(i * BEAT_WIDTH)).join(' ')}`

export function VitalsBand({ mode, alertKey = 0 }: { mode: VitalsMode; alertKey?: number }) {
  const style = { '--beat': `${BEAT_SECONDS[mode]}s` } as CSSProperties
  const bandRef = useRef<HTMLDivElement>(null)

  // กะพริบด้วย Web Animations API (ทับสีชั่วคราวแล้วคืนสีของ mode เอง) — ไม่ต้องมี state/timer ฝั่งหน้า
  useEffect(() => {
    const band = bandRef.current
    if (!alertKey || !band) return
    const alert = getComputedStyle(band).getPropertyValue('--destructive').trim()
    const animation = band.animate(
      [{ color: alert }, { color: alert, offset: 0.7 }, {}],
      { duration: ALERT_MS, easing: 'ease-out' },
    )
    return () => animation.cancel()
  }, [alertKey])

  return (
    <div
      ref={bandRef}
      aria-hidden="true"
      style={style}
      className={cn(
        'bg-ecg-grid flex h-16 items-center gap-3 border-b px-(--card-spacing) transition-colors duration-300',
        TONE[mode],
      )}
    >
      <BrandMark size="md" iconClassName="animate-heartbeat motion-reduce:animate-none" />
      {/* ขอบซ้าย/ขวาจางหาย ให้เส้นดูเหมือนวิ่งผ่านจอ ไม่ใช่รูปที่ถูกตัด */}
      <div className="relative h-10 min-w-0 flex-1 overflow-hidden [mask-image:linear-gradient(90deg,transparent,black_18%,black_82%,transparent)]">
        <svg
          viewBox={`0 0 ${BEAT_WIDTH * BEATS} 40`}
          width={BEAT_WIDTH * BEATS}
          height={40}
          className="absolute top-0 left-0 max-w-none animate-ecg-scroll motion-reduce:animate-none"
        >
          <path
            d={TRACE}
            fill="none"
            stroke="currentColor"
            strokeWidth={1.75}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
      </div>
    </div>
  )
}
