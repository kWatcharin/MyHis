import { useState } from 'react'
import { HeartPulse } from 'lucide-react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { Navigate, useLocation, useNavigate } from 'react-router'
import * as z from 'zod'

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

import { useAuth } from './AuthContext'

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

  const form = useForm<LoginFormValues>({
    resolver: zodResolver(loginSchema),
    defaultValues: { username: '', password: '' },
  })

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
      await login(values.username, values.password)
      const from = (location.state as { from?: string } | null)?.from
      navigate(from ?? '/dashboard', { replace: true })
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง')
    } finally {
      setIsSubmitting(false)
    }
  }

  return (
    <div
      className="flex min-h-svh items-center justify-center bg-background p-4"
      style={{
        // สีอ่อนมาก ๆ แค่แต่งพื้นหลังให้มีมิติ ไม่ใช่บล็อกสีเต็ม — ใช้ color-mix กับ primary ตรง ๆ
        // จะได้ปรับตามธีมที่เลือกเอง (dark/ocean) โดยไม่ต้อง hardcode สี
        backgroundImage:
          'radial-gradient(ellipse 80% 60% at 50% -10%, color-mix(in oklch, var(--primary) 18%, transparent), transparent 60%)',
        backgroundRepeat: 'no-repeat',
      }}
    >
      <Card className="w-full max-w-sm">
        <CardHeader className="items-center text-center">
          <div className="mb-1 flex size-11 items-center justify-center rounded-full bg-primary/10 text-primary">
            <HeartPulse className="size-5" />
          </div>
          <CardTitle>เข้าสู่ระบบ</CardTitle>
          <CardDescription>my-his — เครื่องมือช่วยงานพยาบาล</CardDescription>
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
                      aria-invalid={fieldState.invalid}
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
                    <Input
                      {...field}
                      id="password"
                      type="password"
                      autoComplete="current-password"
                      aria-invalid={fieldState.invalid}
                    />
                    {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
                  </Field>
                )}
              />
              {formError && (
                <Field data-invalid>
                  <FieldError>{formError}</FieldError>
                </Field>
              )}
              <Button type="submit" disabled={isSubmitting} className="w-full">
                {isSubmitting ? 'กำลังเข้าสู่ระบบ...' : 'เข้าสู่ระบบ'}
              </Button>
            </FieldGroup>
          </form>
        </CardContent>
      </Card>
    </div>
  )
}
