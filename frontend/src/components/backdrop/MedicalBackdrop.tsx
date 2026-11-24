import { useEffect, useRef, useState } from 'react'

import { readThemeColor } from '@/lib/cssColor'
import type { Rgb } from '@/lib/cssColor'
import { cn } from '@/lib/utils'

import type { HelixPalette, HelixScene } from './dnaHelixScene'

// พื้นหลัง 3D (DNA helix) สำหรับหน้า "ทางเข้า" — canvas เป็น `absolute inset-0` เต็มกล่องแม่ที่เป็น `relative` (ปกติคือแผงเกลียว
// ของ EntryLayout) เนื้อหาที่ซ้อนบนแผงให้ `relative z-10`; ส่วนที่ไม่ควรนับการคลิกเป็นการ "ดีด" เกลียว ใส่ `data-backdrop-ignore`
// กฎการใช้งาน/สี/งบประสิทธิภาพ: docs/ui-3d-backdrop.md
//
// - three โหลดแบบ dynamic import ตอน mount เท่านั้น (chunk แยก) ระหว่างโหลด/โหลดไม่ได้/ไม่มี WebGL เห็นพื้นหลัง CSS ของหน้าแทน
// - canvas เป็น pointer-events-none — ฟังเมาส์ที่ window ฟอร์มและ Turnstile จึงใช้งานได้ตามปกติ
// - prefers-reduced-motion → เฟรมนิ่ง, แท็บซ่อน → หยุด rAF, เปลี่ยนธีม → อ่านสีใหม่จาก CSS variable

const IGNORE_SELECTOR = '[data-backdrop-ignore]'

function luminance([r, g, b]: Rgb): number {
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

// สีของเกลียวจาก token ของธีม (ไม่ hardcode) แล้วดันให้สดขึ้น (vivid: ความอิ่มสี/ความสว่างอยู่ในช่วงที่ตัดกับพื้นของธีมนั้น)
// - เส้นหลัก: ไล่สีจาก --primary ไปหาสีใน chart/accent ที่ hue ห่างจาก primary มากที่สุด (ธีมมืด = ครามอ่อน→มิ้นท์, ธีมสว่าง = ครามเข้ม→มิ้นท์,
//   ธีมทะเล = มิ้นท์→คราม) เส้นที่สองไล่ย้อนกลับ ขอบแก้วเรืองสีปลายทาง
// - เบส 4 ชนิดใช้สีความหมายของธีม: A = ฟ้า (--chart-2), T = เหลือง/ส้ม (warning), C = แดง (danger), G = เขียว (success)
// ค่าหลัง ?? = สีสำรองเมื่ออ่าน token ไม่ได้ (ใกล้ธีม dark)

function toHsl([r, g, b]: Rgb): [h: number, s: number, l: number] {
  const max = Math.max(r, g, b)
  const min = Math.min(r, g, b)
  const l = (max + min) / 2
  const d = max - min
  if (d === 0) return [0, 0, l]
  const s = d / (1 - Math.abs(2 * l - 1))
  let h: number
  if (max === r) h = ((g - b) / d) % 6
  else if (max === g) h = (b - r) / d + 2
  else h = (r - g) / d + 4
  return [(h * 60 + 360) % 360, s, l]
}

function fromHsl(h: number, s: number, l: number): Rgb {
  const c = (1 - Math.abs(2 * l - 1)) * s
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1))
  const m = l - c / 2
  const sector = Math.floor(h / 60) % 6
  const [r, g, b] = [[c, x, 0], [x, c, 0], [0, c, x], [0, x, c], [x, 0, c], [c, 0, x]][sector]
  return [r + m, g + m, b + m]
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max)
}

// ดันสีให้สด: อิ่มสีอย่างน้อย 0.75; ธีมมืดสว่างพอตัดกับพื้นมืด (L 0.58–0.68), ธีมสว่างเข้มพอตัดกับพื้นขาว (L 0.36–0.46)
function vivid(rgb: Rgb, dark: boolean): Rgb {
  const [h, s, l] = toHsl(rgb)
  return fromHsl(h, Math.max(s, 0.75), dark ? clamp(l, 0.58, 0.68) : clamp(l, 0.36, 0.46))
}

function hueDistance(a: number, b: number): number {
  const d = Math.abs(a - b) % 360
  return d > 180 ? 360 - d : d
}

// สีปลายทางของการไล่สี: token สีของธีมที่ hue ห่างจากสีเริ่มมากที่สุด (ข้ามสีที่จืด/มืดเกินไป)
function pickPartner(from: Rgb, dark: boolean): Rgb | null {
  const fromHue = toHsl(from)[0]
  let best: Rgb | null = null
  let bestDistance = -1
  for (const name of ['--chart-1', '--chart-2', '--chart-3', '--chart-4', '--chart-5', '--accent']) {
    const color = readThemeColor(name)
    if (!color) continue
    const [h, s, l] = toHsl(color)
    if (s < 0.4 || l < 0.2 || l > 0.8) continue
    const distance = hueDistance(h, fromHue)
    if (distance > bestDistance) {
      best = vivid(color, dark)
      bestDistance = distance
    }
  }
  return best
}

function readPalette(): HelixPalette {
  const background = readThemeColor('--background') ?? [0, 0, 0]
  const dark = luminance(background) < 0.5
  const tone = (ink: string, fill: string, fallback: Rgb): Rgb =>
    vivid(readThemeColor(dark ? ink : fill) ?? fallback, dark)
  const backbone = vivid(readThemeColor('--primary') ?? [0.6, 0.66, 0.95], dark)
  const backbone2 = pickPartner(backbone, dark) ?? vivid([0.08, 0.85, 0.7], dark)
  return {
    backbone,
    backbone2,
    baseA: vivid(readThemeColor('--chart-2') ?? [0.29, 0.64, 0.73], dark),
    baseT: tone('--warning-ink', '--warning', [0.95, 0.78, 0.4]),
    baseC: tone('--danger-ink', '--danger', [0.93, 0.48, 0.45]),
    baseG: tone('--success-ink', '--success', [0.45, 0.85, 0.6]),
    bond: readThemeColor(dark ? '--foreground' : '--muted-foreground') ?? [0.9, 0.92, 0.96],
    rim: backbone2,
    dark,
  }
}

export function MedicalBackdrop({ className }: { className?: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const [status, setStatus] = useState<'loading' | 'ready' | 'failed'>('loading')

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    let cancelled = false
    let scene: HelixScene | null = null
    const teardown: Array<() => void> = []
    const listen = <K extends keyof WindowEventMap>(type: K, handler: (event: WindowEventMap[K]) => void) => {
      window.addEventListener(type, handler, { passive: true })
      teardown.push(() => window.removeEventListener(type, handler))
    }

    import('./dnaHelixScene')
      .then(({ createDnaHelixScene }) => {
        if (cancelled) return
        const motionQuery = window.matchMedia('(prefers-reduced-motion: reduce)')
        try {
          scene = createDnaHelixScene(canvas, {
            reducedMotion: motionQuery.matches,
            palette: readPalette(),
          })
        } catch (err) {
          // ไม่มี WebGL ฯลฯ — ตอน dev แสดงสาเหตุไว้ไล่ปัญหา (production เงียบ ถอยไปแผงพื้นเรียบ)
          if (import.meta.env.DEV) console.warn('3D backdrop unavailable:', err)
          setStatus('failed')
          return
        }
        const s = scene
        s.resize(canvas.clientWidth, canvas.clientHeight)
        s.setActive(!document.hidden)

        // พิกัดเทียบกับกรอบของ canvas เอง (อยู่ในแผงของ EntryLayout ไม่ใช่เต็มจอ) — เมาส์นอกแผง = ปล่อยเกลียวกลับเข้าที่
        const toNdc = (event: PointerEvent): boolean => {
          const rect = canvas.getBoundingClientRect()
          const x = (event.clientX - rect.left) / rect.width
          const y = (event.clientY - rect.top) / rect.height
          if (x < 0 || x > 1 || y < 0 || y > 1) {
            s.clearPointer()
            return false
          }
          s.setPointer(x * 2 - 1, -y * 2 + 1)
          return true
        }
        listen('pointermove', (event) => {
          toNdc(event)
        })
        listen('pointerdown', (event) => {
          const inside = toNdc(event)
          const target = event.target
          if (inside && !(target instanceof Element && target.closest(IGNORE_SELECTOR))) s.impulse()
        })
        // นิ้วยกขึ้น/เมาส์ออกนอกหน้าต่าง (pointerout ที่ไม่มี relatedTarget) → เกลียวค่อย ๆ กลับเข้าที่
        listen('pointerup', (event) => {
          if (event.pointerType !== 'mouse') s.clearPointer()
        })
        listen('pointercancel', () => s.clearPointer())
        listen('pointerout', (event) => {
          if (!event.relatedTarget) s.clearPointer()
        })
        listen('blur', () => s.clearPointer())

        const onVisibility = () => s.setActive(!document.hidden)
        document.addEventListener('visibilitychange', onVisibility)
        teardown.push(() => document.removeEventListener('visibilitychange', onVisibility))

        const onMotionChange = () => s.setReducedMotion(motionQuery.matches)
        motionQuery.addEventListener('change', onMotionChange)
        teardown.push(() => motionQuery.removeEventListener('change', onMotionChange))

        const resizeObserver = new ResizeObserver(() => s.resize(canvas.clientWidth, canvas.clientHeight))
        resizeObserver.observe(canvas)
        teardown.push(() => resizeObserver.disconnect())

        // ธีมสลับด้วย class บน <html> (ThemeContext) — ดู class ตรง ๆ แทน theme state ของ React เพราะ effect ของ
        // component ลูกรันก่อน effect ของ ThemeProvider ที่สลับ class (อ่านตาม state จะได้สีธีมเก่า)
        const themeObserver = new MutationObserver(() => s.setPalette(readPalette()))
        themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
        teardown.push(() => themeObserver.disconnect())

        setStatus('ready')
      })
      .catch((err) => {
        // chunk โหลดไม่สำเร็จ (เน็ตหลุด/deploy ใหม่ระหว่างเปิดหน้า) หรือฉากพังตอนตั้งค่า — แผงพื้นเรียบยังอยู่ หน้าใช้งานได้ปกติ
        if (import.meta.env.DEV) console.warn('3D backdrop unavailable:', err)
        if (!cancelled) setStatus('failed')
      })

    return () => {
      cancelled = true
      for (const fn of teardown) fn()
      scene?.dispose()
    }
  }, [])

  if (status === 'failed') return null

  return (
    <canvas
      ref={canvasRef}
      aria-hidden="true"
      className={cn(
        'pointer-events-none absolute inset-0 size-full transition-opacity duration-700 ease-out',
        status === 'ready' ? 'opacity-100' : 'opacity-0',
        className,
      )}
    />
  )
}
