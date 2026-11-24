import { useEffect, useState } from 'react'
import type { ReactNode } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { useLocation, useNavigate } from 'react-router'
import * as z from 'zod'

import { EntryLayout } from '@/components/backdrop/EntryLayout'
import { VitalsBand } from '@/components/backdrop/VitalsBand'
import { PasswordInput } from '@/components/PasswordInput'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { NativeSelect } from '@/components/ui/native-select'
import { useAuth } from '@/features/auth/AuthContext'
import { formatThaiDateTime } from '@/lib/date'

import { isInvalidLink, registrationErrorMessage } from './errors'
import type { RegistrationInfo } from './registrationApi'
import { registrationApi } from './registrationApi'

// เกณฑ์เดียวกับ backend (docs/auth-design.md ข้อ 8 และ backend/src/registration/service.rs)
const MIN_PASSWORD_LENGTH = 12
const PROFILE_TEXT_MAX = 100

const text = (label: string) =>
  z.string().trim().max(PROFILE_TEXT_MAX, `${label}ยาวไม่เกิน ${PROFILE_TEXT_MAX} ตัวอักษร`)

// ชนิดค่าเดียวกันทั้งสองโหมด — โหมดตั้งรหัสใหม่ไม่แสดง/ไม่ส่งช่องข้อมูลตัว จึงบังคับกรอกเฉพาะโหมดลงทะเบียน
// (ตรวจใน superRefine พร้อมรหัสผ่าน ให้ error ทุกช่องขึ้นพร้อมกัน)
function buildSchema(isRegister: boolean) {
  return z
    .object({
      prenameId: z.string(),
      firstName: text('ชื่อ'),
      lastName: text('นามสกุล'),
      position: text('ตำแหน่ง'),
      newPassword: z.string(),
      confirmPassword: z.string(),
    })
    .superRefine((v, ctx) => {
      const issue = (path: string, message: string) =>
        ctx.addIssue({ code: 'custom', path: [path], message })
      if (isRegister) {
        if (!v.prenameId) issue('prenameId', 'เลือกคำนำหน้า')
        if (!v.firstName) issue('firstName', 'กรอกชื่อ')
        if (!v.lastName) issue('lastName', 'กรอกนามสกุล')
      }
      if (v.newPassword.length < MIN_PASSWORD_LENGTH) {
        issue('newPassword', `รหัสผ่านต้องยาวอย่างน้อย ${MIN_PASSWORD_LENGTH} ตัวอักษร`)
      }
      if (v.confirmPassword !== v.newPassword) {
        issue('confirmPassword', 'ยืนยันรหัสผ่านไม่ตรงกับรหัสผ่าน')
      }
    })
}

type RegisterValues = z.infer<ReturnType<typeof buildSchema>>

type PageState =
  | { status: 'loading' }
  | { status: 'invalid' }
  | { status: 'ready'; info: RegistrationInfo }
  | { status: 'done'; info: RegistrationInfo }

// โครงเดียวกับหน้า login: แผงเกลียว DNA + ชื่อ my-his (EntryLayout) และการ์ดที่หัวเป็นแถบ monitor (VitalsBand)
// busy = กำลังบันทึก, alertKey เปลี่ยน = บันทึกไม่สำเร็จ (เส้นกะพริบสีแดง) — ดู docs/ui-3d-backdrop.md
function Shell({
  children,
  busy = false,
  alertKey = 0,
}: {
  children: ReactNode
  busy?: boolean
  alertKey?: number
}) {
  return (
    <EntryLayout>
      <Card className="w-full max-w-md gap-5 pt-0">
        <VitalsBand mode={busy ? 'busy' : 'rest'} alertKey={alertKey} />
        {children}
      </Card>
    </EntryLayout>
  )
}

function Header({ title, description }: { title: string; description: ReactNode }) {
  return (
    <CardHeader>
      <CardTitle className="text-lg font-semibold">
        <h1>{title}</h1>
      </CardTitle>
      <CardDescription>{description}</CardDescription>
    </CardHeader>
  )
}

// หน้าลงทะเบียนผู้ใช้งาน / ตั้งรหัสผ่านใหม่ จากลิงก์ที่แอดมินออกให้ (`/register#<token>` — ไม่ต้อง login)
// token อยู่หลัง # จึงไม่ถูกส่งไป server ตอนเปิดหน้า อ่านครั้งเดียวแล้วลบออกจาก address bar/ประวัติทันที
// ไม่เก็บ token ใน storage/console — ถ้า refresh หน้า ต้องเปิดจากลิงก์อีกครั้ง (ลิงก์ยังใช้ได้จนกว่าจะลงทะเบียนสำเร็จ)
export function RegisterPage() {
  const location = useLocation()
  const navigate = useNavigate()
  // lazy initializer อ่านอย่างเดียว — StrictMode render/mount ซ้ำก็ยังได้ token เดิม (ลบ hash ทำใน effect)
  const [token] = useState(() => window.location.hash.replace(/^#/, ''))
  const [state, setState] = useState<PageState>(() =>
    token ? { status: 'loading' } : { status: 'invalid' },
  )

  // ลบ token ออกจาก URL ผ่าน router (ไม่ใช่ history.replaceState ตรง ๆ) ให้ router รู้ตำแหน่งจริง
  useEffect(() => {
    if (location.hash) navigate(location.pathname, { replace: true })
  }, [location.hash, location.pathname, navigate])

  useEffect(() => {
    if (!token) return
    let cancelled = false
    registrationApi
      .lookup(token)
      .then((info) => {
        if (!cancelled) setState({ status: 'ready', info })
      })
      .catch(() => {
        if (!cancelled) setState({ status: 'invalid' })
      })
    return () => {
      cancelled = true
    }
  }, [token])

  if (state.status === 'loading') {
    return (
      <Shell>
        <Header title="ลงทะเบียนผู้ใช้งาน" description="กำลังตรวจสอบลิงก์..." />
      </Shell>
    )
  }

  if (state.status === 'invalid') {
    return (
      <Shell>
        <Header title="ลิงก์ใช้ไม่ได้" description="ลิงก์ไม่ถูกต้อง หมดอายุ หรือถูกใช้ไปแล้ว" />
        <CardContent className="flex flex-col items-start gap-4 text-sm text-muted-foreground">
          <p>ขอลิงก์ใหม่จากผู้ดูแลระบบ (ลิงก์ใช้ได้ครั้งเดียว และหมดอายุใน 3 วันหลังออกให้)</p>
          <Button variant="outline" className="h-10" onClick={() => navigate('/login')}>
            ไปหน้าเข้าสู่ระบบ
          </Button>
        </CardContent>
      </Shell>
    )
  }

  if (state.status === 'done') {
    return <DoneView info={state.info} />
  }

  return (
    <RegisterForm
      key={state.info.mode}
      token={token}
      info={state.info}
      onDone={() => setState({ status: 'done', info: state.info })}
      onInvalid={() => setState({ status: 'invalid' })}
    />
  )
}

interface RegisterFormProps {
  token: string
  info: RegistrationInfo
  onDone: () => void
  onInvalid: () => void
}

function RegisterForm({ token, info, onDone, onInvalid }: RegisterFormProps) {
  const isRegister = info.mode === 'register'
  const [submitError, setSubmitError] = useState<string | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)
  // เพิ่มทุกครั้งที่บันทึกไม่สำเร็จ → แถบ monitor กะพริบสีแดงหนึ่งครั้ง
  const [alertKey, setAlertKey] = useState(0)

  // คำนำหน้าเดิม (ถ้าแอดมินกรอกไว้ก่อน) ต้องยังอยู่ในตัวเลือก
  const initialPrename =
    info.prenameId && info.prenames.some((p) => p.id === info.prenameId) ? info.prenameId : ''
  // ฟอร์มสร้างครั้งเดียวต่อโหมด (component ถูก key ด้วย mode) — schema ของโหมดนี้จึงคงที่ตลอดอายุฟอร์ม
  const form = useForm<RegisterValues>({
    resolver: zodResolver(buildSchema(isRegister)),
    defaultValues: {
      prenameId: initialPrename,
      firstName: info.firstName ?? '',
      lastName: info.lastName ?? '',
      position: info.position ?? '',
      newPassword: '',
      confirmPassword: '',
    },
  })

  const handleSubmit = form.handleSubmit(async (values) => {
    setSubmitError(null)
    setIsSubmitting(true)
    try {
      await registrationApi.complete(
        isRegister
          ? {
              token,
              newPassword: values.newPassword,
              prenameId: values.prenameId,
              firstName: values.firstName,
              lastName: values.lastName,
              position: values.position === '' ? null : values.position,
            }
          : { token, newPassword: values.newPassword },
      )
      onDone()
    } catch (err) {
      if (isInvalidLink(err)) {
        onInvalid()
        return
      }
      setSubmitError(registrationErrorMessage(err))
      setAlertKey((n) => n + 1)
    } finally {
      setIsSubmitting(false)
    }
  })

  const textField = (
    name: 'firstName' | 'lastName' | 'position',
    label: string,
    autoComplete: string,
  ) => (
    <Controller
      name={name}
      control={form.control}
      render={({ field, fieldState }) => (
        <Field data-invalid={fieldState.invalid}>
          <FieldLabel htmlFor={`register-${name}`}>{label}</FieldLabel>
          <Input
            {...field}
            id={`register-${name}`}
            autoComplete={autoComplete}
            aria-invalid={fieldState.invalid}
            className="h-10"
          />
          {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
        </Field>
      )}
    />
  )

  const passwordField = (name: 'newPassword' | 'confirmPassword', label: string) => (
    <Controller
      name={name}
      control={form.control}
      render={({ field, fieldState }) => (
        <Field data-invalid={fieldState.invalid}>
          <FieldLabel htmlFor={`register-${name}`}>{label}</FieldLabel>
          <PasswordInput
            {...field}
            id={`register-${name}`}
            autoComplete="new-password"
            aria-invalid={fieldState.invalid}
            className="h-10"
          />
          {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
        </Field>
      )}
    />
  )

  return (
    <Shell busy={isSubmitting} alertKey={alertKey}>
      <Header
        title={isRegister ? 'ลงทะเบียนผู้ใช้งาน' : 'ตั้งรหัสผ่านใหม่'}
        description={
          <>
            บัญชี <span className="font-medium text-foreground">{info.username}</span> ({info.displayName})
            <br />
            ลิงก์ใช้ได้ครั้งเดียว ถึง {formatThaiDateTime(info.expiresAt)}
          </>
        }
      />
      <CardContent>
        <form onSubmit={handleSubmit} noValidate>
          {/* ช่วยให้ตัวจัดการรหัสผ่านของเบราว์เซอร์บันทึกรหัสคู่กับบัญชีนี้ */}
          <input type="text" name="username" autoComplete="username" value={info.username} readOnly hidden />
          <FieldGroup>
            {isRegister && (
              <>
                <Controller
                  name="prenameId"
                  control={form.control}
                  render={({ field, fieldState }) => (
                    <Field data-invalid={fieldState.invalid}>
                      <FieldLabel htmlFor="register-prename">คำนำหน้า</FieldLabel>
                      <NativeSelect
                        {...field}
                        id="register-prename"
                        aria-invalid={fieldState.invalid}
                        className="h-10"
                      >
                        <option value="">— เลือก —</option>
                        {info.prenames.map((prename) => (
                          <option key={prename.id} value={prename.id}>
                            {prename.name}
                          </option>
                        ))}
                      </NativeSelect>
                      {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                    </Field>
                  )}
                />
                <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                  {textField('firstName', 'ชื่อ', 'given-name')}
                  {textField('lastName', 'นามสกุล', 'family-name')}
                </div>
                {textField('position', 'ตำแหน่ง (ไม่บังคับ)', 'organization-title')}
              </>
            )}
            {passwordField('newPassword', `รหัสผ่าน (อย่างน้อย ${MIN_PASSWORD_LENGTH} ตัวอักษร)`)}
            {passwordField('confirmPassword', 'ยืนยันรหัสผ่าน')}
            <FieldDescription>
              ตั้งเองและจำไว้ ผู้ดูแลระบบมองไม่เห็นรหัสผ่านของคุณ ถ้าลืมให้ขอลิงก์ตั้งรหัสใหม่
            </FieldDescription>
            {submitError && (
              <Field data-invalid>
                <FieldError>{submitError}</FieldError>
              </Field>
            )}
            <Button type="submit" disabled={isSubmitting} className="h-10 w-full">
              {isSubmitting ? 'กำลังบันทึก...' : isRegister ? 'ลงทะเบียน' : 'ตั้งรหัสผ่านใหม่'}
            </Button>
          </FieldGroup>
        </form>
      </CardContent>
    </Shell>
  )
}

function DoneView({ info }: { info: RegistrationInfo }) {
  const navigate = useNavigate()
  const { status, user, logout } = useAuth()
  // เบราว์เซอร์นี้เข้าสู่ระบบด้วยบัญชีอื่นอยู่ (เช่นแอดมินลองเปิดลิงก์เองบนเครื่องเดียวกัน) — ต้องออกก่อนถึงจะเข้าบัญชีใหม่ได้
  const otherAccount = status === 'authenticated' && user?.username !== info.username

  const goToLogin = async () => {
    if (status === 'authenticated') await logout()
    navigate('/login', { replace: true, state: { username: info.username, registered: true } })
  }

  return (
    <Shell>
      <Header
        title={info.mode === 'register' ? 'ลงทะเบียนสำเร็จ' : 'ตั้งรหัสผ่านใหม่แล้ว'}
        description="เข้าสู่ระบบด้วยชื่อผู้ใช้และรหัสผ่านที่เพิ่งตั้งได้เลย"
      />
      <CardContent className="flex flex-col gap-4 text-sm">
        <p>
          ชื่อผู้ใช้ <span className="font-medium">{info.username}</span>
        </p>
        {otherAccount && (
          <p className="text-muted-foreground">
            เบราว์เซอร์นี้เข้าสู่ระบบด้วยบัญชี <span className="font-medium">{user?.username}</span> อยู่
            ปุ่มด้านล่างจะออกจากระบบบัญชีนั้นก่อน
          </p>
        )}
        <Button className="h-10 w-full" onClick={() => void goToLogin()}>
          {otherAccount ? 'ออกจากระบบแล้วไปหน้าเข้าสู่ระบบ' : 'ไปหน้าเข้าสู่ระบบ'}
        </Button>
      </CardContent>
    </Shell>
  )
}
