import { useEffect, useState } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { Navigate, useLocation, useNavigate } from 'react-router'
import * as z from 'zod'

import { EntryLayout } from '@/components/backdrop/EntryLayout'
import { PasswordInput } from '@/components/PasswordInput'
import { Button } from '@/components/ui/button'
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from '@/components/ui/card'
import { Field, FieldError, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api'
import { cn } from '@/lib/utils'

import { useAuth } from './AuthContext'
import { LoginError, MAX_LOGIN_ATTEMPTS } from './loginError'
import { TurnstileWidget } from './TurnstileWidget'

// ตรงกับ TURNSTILE_ACTION ใน backend/src/auth/service.rs (token ของ widget อื่นใช้ login ไม่ได้)
const TURNSTILE_ACTION = 'login'

// GET /auth/login-config — turnstileSiteKey null = backend ปิด Turnstile ไม่ต้องแสดง widget
interface LoginConfig {
  turnstileSiteKey: string | null
}

type LoginConfigState =
  | { status: 'loading' }
  | { status: 'error' }
  | { status: 'ready'; turnstileSiteKey: string | null }

const loginSchema = z.object({
  username: z.string().min(1, 'กรอกชื่อผู้ใช้'),
  password: z.string().min(1, 'กรอกรหัสผ่าน'),
})

type LoginFormValues = z.infer<typeof loginSchema>

export function LoginPage() {
  const { status, login } = useAuth()
  const navigate = useNavigate()
  const location = useLocation()
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [formError, setFormError] = useState<string | null>(null)
  // ตัวนับจาก backend (นับต่อชื่อผู้ใช้ที่ server — refresh แล้วหน้ากลับเป็น 5/5 แต่ server ยังจำ)
  const [remaining, setRemaining] = useState(MAX_LOGIN_ATTEMPTS)
  // ล็อกกี่วินาที (จาก Retry-After ของ 429) — ระหว่างล็อกปิดฟอร์มและนับถอยหลัง null = ไม่ล็อก
  const [lockSeconds, setLockSeconds] = useState<number | null>(null)
  const [secondsLeft, setSecondsLeft] = useState(0)
  const [loginConfig, setLoginConfig] = useState<LoginConfigState>({ status: 'loading' })
  const [turnstileToken, setTurnstileToken] = useState<string | null>(null)
  // เพิ่มทุกครั้งที่ส่ง token ไปแล้ว login ไม่สำเร็จ — token ใช้ได้ครั้งเดียว ใช้เป็น key ให้ widget mount ใหม่ออก token ใหม่
  const [turnstileRound, setTurnstileRound] = useState(0)
  // เพิ่มทุกครั้งที่ควรพาโฟกัสกลับไปช่องรหัสผ่าน (ดู effect ด้านล่าง)
  const [retryFocus, setRetryFocus] = useState(0)
  // มาจากหน้าลงทะเบียน (RegisterPage) — เติมชื่อผู้ใช้ไว้ให้ / มาจาก RequireAuth — หน้าเดิมที่จะกลับไป
  const navState = location.state as { from?: string; username?: string; registered?: boolean } | null

  const form = useForm<LoginFormValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: { username: navState?.username ?? '', password: '' },
  })

  // นับถอยหลังจากเวลาสิ้นสุด (ไม่นับลดทีละ 1 กันเวลาคลาดเมื่อแท็บถูกพัก) ครบแล้วเปิดฟอร์ม กลับเป็น 5/5
  useEffect(() => {
    if (lockSeconds === null) return
    const deadline = Date.now() + lockSeconds * 1000
    const timer = window.setInterval(() => {
      const left = Math.ceil((deadline - Date.now()) / 1000)
      if (left <= 0) {
        setLockSeconds(null)
        setRemaining(MAX_LOGIN_ATTEMPTS)
        setFormError(null)
        setRetryFocus((n) => n + 1)
      } else {
        setSecondsLeft(left)
      }
    }, 250)
    return () => window.clearInterval(timer)
  }, [lockSeconds])

  // ลองใหม่ได้ทันทีด้วยมือเดียว: login ไม่ผ่าน (ที่ไม่ใช่ล็อก) หรือพ้นล็อกแล้ว → โฟกัสช่องรหัสผ่านพร้อมเลือกข้อความเดิมไว้
  // พิมพ์ทับได้เลย ทำใน effect เพราะช่องต้องหาย disabled (render รอบถัดไป) ก่อนถึงจะโฟกัสได้
  useEffect(() => {
    if (retryFocus > 0) form.setFocus('password', { shouldSelect: true })
  }, [retryFocus, form])

  useEffect(() => {
    let cancelled = false
    api
      .get<LoginConfig>('/auth/login-config')
      .then((config) => {
        if (!cancelled) setLoginConfig({ status: 'ready', turnstileSiteKey: config.turnstileSiteKey })
      })
      .catch(() => {
        if (!cancelled) setLoginConfig({ status: 'error' })
      })
    return () => {
      cancelled = true
    }
  }, [])

  const locked = lockSeconds !== null
  const turnstileSiteKey = loginConfig.status === 'ready' ? loginConfig.turnstileSiteKey : null
  // ยังไม่รู้ config / โหลด config ไม่ได้ / เปิด Turnstile แต่ยังไม่ได้ token → ยังกดเข้าสู่ระบบไม่ได้
  const waitingForCheck =
    loginConfig.status !== 'ready' || (turnstileSiteKey !== null && !turnstileToken)

  // ระหว่างเช็ค /auth/me ตอนโหลดแอปครั้งแรก ยังไม่โชว์ฟอร์ม กันการกระพริบเห็นฟอร์ม login แป๊บเดียว
  // ก่อนเด้งไป dashboard ถ้าจริง ๆ แล้ว login ค้างอยู่แล้ว (เช่น refresh หน้านี้ทั้งที่ login อยู่)
  if (status === 'loading') {
    return null
  }

  // login แล้วไม่ต้องเห็นฟอร์ม login อีก เด้งไป dashboard ทันที
  if (status === 'authenticated') {
    return <Navigate to="/dashboard" replace />
  }

  const onSubmit = async (values: LoginFormValues) => {
    setFormError(null)
    setIsSubmitting(true)
    try {
      await login(values.username, values.password, turnstileToken ?? undefined)
      navigate(navState?.from ?? '/dashboard', { replace: true })
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง')
      if (turnstileSiteKey) {
        setTurnstileToken(null)
        setTurnstileRound((round) => round + 1)
      }
      const lockedNow = err instanceof LoginError && err.lockedForSeconds !== null
      if (err instanceof LoginError) {
        if (err.lockedForSeconds !== null) {
          setSecondsLeft(err.lockedForSeconds)
          setLockSeconds(err.lockedForSeconds)
          setRemaining(0)
        } else if (err.remainingAttempts !== null) {
          setRemaining(err.remainingAttempts)
        }
      }
      if (!lockedNow) {
        setRetryFocus((n) => n + 1)
      }
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    // สองฝั่ง: แผงเกลียว DNA + ชื่อ MyHis (EntryLayout) / การ์ดฟอร์ม — คำโปรยอยู่บนแผงแล้ว หัวการ์ดจึงมีแค่ชื่อหน้า
    <EntryLayout>
      <Card className="w-full max-w-sm gap-5">
        <CardHeader>
          <CardTitle className="text-lg font-semibold">
            <h1>เข้าสู่ระบบ</h1>
          </CardTitle>
          {navState?.registered && (
            <CardDescription>
              ตั้งรหัสผ่านเรียบร้อยแล้ว เข้าสู่ระบบด้วยรหัสผ่านที่เพิ่งตั้งได้เลย
            </CardDescription>
          )}
        </CardHeader>
        <CardContent>
          <form onSubmit={form.handleSubmit(onSubmit)} noValidate>
            <FieldGroup>
              <Controller
                name="username"
                control={form.control}
                render={({ field, fieldState }) => (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor="username">ชื่อผู้ใช้</FieldLabel>
                    <Input
                      {...field}
                      id="username"
                      autoComplete="username"
                      // หน้าเข้าระบบเปิดบนมือถือมือเดียวเป็นหลัก — ช่องและปุ่มสูง 40px (ค่ามาตรฐานของระบบ 32px)
                      className="h-10"
                      aria-invalid={fieldState.invalid}
                      disabled={locked}
                      onChange={(event) => {
                        field.onChange(event)
                        // นับแยกต่อชื่อผู้ใช้ — เปลี่ยนชื่อแล้วยังไม่รู้ค่าจริง server จะบอกในครั้งถัดไป
                        setRemaining(MAX_LOGIN_ATTEMPTS)
                      }}
                    />
                    {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                  </Field>
                )}
              />
              <Controller
                name="password"
                control={form.control}
                render={({ field, fieldState }) => (
                  <Field data-invalid={fieldState.invalid}>
                    <FieldLabel htmlFor="password">รหัสผ่าน</FieldLabel>
                    <PasswordInput
                      {...field}
                      id="password"
                      autoComplete="current-password"
                      className="h-10"
                      aria-invalid={fieldState.invalid}
                      disabled={locked}
                    />
                    {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                  </Field>
                )}
              />
              {turnstileSiteKey && (
                <TurnstileWidget
                  key={turnstileRound}
                  siteKey={turnstileSiteKey}
                  action={TURNSTILE_ACTION}
                  onToken={setTurnstileToken}
                />
              )}
              {/* สถานะของฟอร์มรวมเป็นก้อนเดียวเหนือปุ่ม — ระหว่างล็อกไม่โชว์ตัวนับ 0/5 ซ้ำ
                  เพราะข้อความล็อกกับปุ่ม "รอ … วินาที" บอกครบแล้ว */}
              <div className="flex flex-col gap-2">
                {loginConfig.status === 'error' && (
                  <FieldError>โหลดหน้าเข้าสู่ระบบไม่สำเร็จ ลองรีเฟรชหน้า</FieldError>
                )}
                {(locked || formError) && (
                  <FieldError>
                    {locked
                      ? `ลองผิดครบ ${MAX_LOGIN_ATTEMPTS} ครั้ง เข้าสู่ระบบได้อีกครั้งใน ${secondsLeft} วินาที`
                      : formError}
                  </FieldError>
                )}
                {/* ตัวนับแบบค่าอ่าน: ชื่อค่าทางซ้าย ตัวเลขทางขวา (ตัวเลขความกว้างเท่ากัน ไม่กระดิกตอนเปลี่ยน)
                    ซ่อนไว้ตอนยังเต็ม 5/5 (ยังไม่เคยลองผิด) แล้วโผล่เมื่อเหลือน้อยกว่านั้น */}
                {!locked && remaining < MAX_LOGIN_ATTEMPTS && (
                  <p
                    aria-live="polite"
                    className="flex items-baseline justify-between border-t border-dashed pt-3 text-sm text-muted-foreground"
                  >
                    <span>เหลือโอกาส</span>
                    <span
                      className={cn(
                        'tabular-nums',
                        remaining === 0
                          ? 'text-destructive'
                          : remaining <= 2
                            ? 'text-warning-ink'
                            : 'text-foreground',
                      )}
                    >
                      <span className="text-base font-semibold">
                        {remaining}/{MAX_LOGIN_ATTEMPTS}
                      </span>{' '}
                      ครั้ง
                    </span>
                  </p>
                )}
              </div>
              <Button
                type="submit"
                disabled={isSubmitting || locked || waitingForCheck}
                className="h-10 w-full"
              >
                {locked
                  ? `รอ ${secondsLeft} วินาที`
                  : isSubmitting
                    ? 'กำลังเข้าสู่ระบบ...'
                    : 'เข้าสู่ระบบ'}
              </Button>
            </FieldGroup>
          </form>
        </CardContent>
      </Card>
    </EntryLayout>
  )
}
