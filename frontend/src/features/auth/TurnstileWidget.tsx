import { useEffect, useRef, useState } from 'react'

import { isDarkTheme, useTheme } from '@/features/theme/ThemeContext'

// Cloudflare Turnstile (กันบอทหน้า login — docs/auth-design.md ข้อ 6) โหลด script ของ Cloudflare เองแบบ explicit
// ไม่ใช้ npm package เพิ่ม — CSP ต้องอนุญาต challenges.cloudflare.com ใน script-src และ frame-src (frontend/Caddyfile)
// token ใช้ได้ครั้งเดียว อายุ 300 วินาที ห้ามเก็บลง localStorage/URL/console
// ขอ token ใหม่ = ผู้เรียกเปลี่ยน `key` ให้ component mount ใหม่ (remove + render) ไม่ใช้ turnstile.reset() เพราะ reset
// ของ test key ไม่ออก token ใหม่ให้ (ลองใน Chrome แล้ว 2026-10-14) ส่วน render ใหม่ได้ token ทุกครั้ง
const SCRIPT_URL = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit'

// widget 'flexible' กว้างอย่างน้อย 300px — ที่แคบกว่านั้น (มือถือจอเล็ก) ใช้แบบ compact แทน
const MIN_FLEXIBLE_WIDTH = 300

interface TurnstileRenderOptions {
  sitekey: string
  action: string
  theme: 'light' | 'dark'
  language: string
  size: 'flexible' | 'compact'
  'refresh-expired': 'auto'
  callback: (token: string) => void
  'expired-callback': () => void
  'error-callback': () => void
}

interface TurnstileApi {
  render: (container: HTMLElement, options: TurnstileRenderOptions) => string | undefined
  remove: (widgetId: string) => void
}

declare global {
  interface Window {
    turnstile?: TurnstileApi
  }
}

let scriptPromise: Promise<TurnstileApi> | null = null

// โหลด script ครั้งเดียวต่อหน้า (กลับมาหน้า login ซ้ำไม่โหลดใหม่) — โหลดไม่สำเร็จแล้วลองใหม่ได้ในครั้งถัดไป
function loadTurnstile(): Promise<TurnstileApi> {
  if (window.turnstile) return Promise.resolve(window.turnstile)
  scriptPromise ??= new Promise<TurnstileApi>((resolve, reject) => {
    const script = document.createElement('script')
    script.src = SCRIPT_URL
    script.onload = () => {
      if (window.turnstile) resolve(window.turnstile)
      else reject(new Error('turnstile api missing'))
    }
    script.onerror = () => {
      scriptPromise = null
      script.remove()
      reject(new Error('turnstile script failed to load'))
    }
    document.head.appendChild(script)
  })
  return scriptPromise
}

interface TurnstileWidgetProps {
  siteKey: string
  // ต้องตรงกับที่ backend ตรวจ (TURNSTILE_ACTION ใน backend/src/auth/service.rs)
  action: string
  // token ใหม่ / null เมื่อหมดอายุ, error หรือ widget ถูกถอด
  onToken: (token: string | null) => void
}

export function TurnstileWidget({ siteKey, action, onToken }: TurnstileWidgetProps) {
  const { theme } = useTheme()
  const dark = isDarkTheme(theme)
  const containerRef = useRef<HTMLDivElement>(null)
  // callback ล่าสุดของผู้เรียก — widget ไม่ต้อง render ใหม่ทุกครั้งที่ฟังก์ชันเปลี่ยน
  const onTokenRef = useRef(onToken)
  const [loadFailed, setLoadFailed] = useState(false)

  useEffect(() => {
    onTokenRef.current = onToken
  }, [onToken])

  useEffect(() => {
    let cancelled = false
    let widgetId: string | undefined
    loadTurnstile()
      .then((turnstile) => {
        const container = containerRef.current
        if (cancelled || !container) return
        widgetId = turnstile.render(container, {
          sitekey: siteKey,
          action,
          theme: dark ? 'dark' : 'light',
          language: 'th',
          size: container.offsetWidth < MIN_FLEXIBLE_WIDTH ? 'compact' : 'flexible',
          'refresh-expired': 'auto',
          callback: (token) => onTokenRef.current(token),
          'expired-callback': () => onTokenRef.current(null),
          'error-callback': () => onTokenRef.current(null),
        })
      })
      .catch(() => {
        if (!cancelled) setLoadFailed(true)
      })
    return () => {
      cancelled = true
      if (widgetId) window.turnstile?.remove(widgetId)
      onTokenRef.current(null)
    }
  }, [siteKey, action, dark])

  return (
    <div className="flex flex-col items-center gap-1">
      <div ref={containerRef} className="flex w-full justify-center" />
      {loadFailed && (
        <p className="text-center text-sm text-destructive">
          โหลดตัวตรวจสอบความปลอดภัยไม่สำเร็จ ตรวจการเชื่อมต่อแล้วรีเฟรชหน้า
        </p>
      )}
    </div>
  )
}
