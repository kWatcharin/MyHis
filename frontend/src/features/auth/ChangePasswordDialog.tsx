import { useEffect } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import * as z from 'zod'

import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Field, FieldError, FieldGroup, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { ApiError } from '@/lib/api'
import { notify } from '@/lib/notify'

import { useAuth } from './AuthContext'

// ความยาวขั้นต่ำตาม docs/auth-design.md ข้อ 8 (backend ตรวจซ้ำ)
const MIN_PASSWORD_LENGTH = 12

const changePasswordSchema = z
  .object({
    currentPassword: z.string().min(1, 'กรอกรหัสผ่านปัจจุบัน'),
    newPassword: z
      .string()
      .min(MIN_PASSWORD_LENGTH, `รหัสผ่านใหม่ต้องยาวอย่างน้อย ${MIN_PASSWORD_LENGTH} ตัวอักษร`),
    confirmPassword: z.string(),
  })
  .superRefine((v, ctx) => {
    if (v.newPassword && v.newPassword === v.currentPassword) {
      ctx.addIssue({
        code: 'custom',
        path: ['newPassword'],
        message: 'รหัสผ่านใหม่ต้องไม่ซ้ำกับรหัสผ่านปัจจุบัน',
      })
    }
    if (v.confirmPassword !== v.newPassword) {
      ctx.addIssue({
        code: 'custom',
        path: ['confirmPassword'],
        message: 'ยืนยันรหัสผ่านไม่ตรงกับรหัสผ่านใหม่',
      })
    }
  })

type ChangePasswordValues = z.infer<typeof changePasswordSchema>

const EMPTY_VALUES: ChangePasswordValues = {
  currentPassword: '',
  newPassword: '',
  confirmPassword: '',
}

// ข้อความ error ที่รู้จักจาก backend (POST /auth/password — backend/src/auth/service.rs) แปลเป็นไทย
function changePasswordErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    if (err.message === 'invalid credentials') return 'รหัสผ่านปัจจุบันไม่ถูกต้อง'
    if (err.message === 'new password must be different from current password') {
      return 'รหัสผ่านใหม่ต้องไม่ซ้ำกับรหัสผ่านปัจจุบัน'
    }
    if (err.message.includes(`at least ${MIN_PASSWORD_LENGTH} characters`)) {
      return `รหัสผ่านใหม่ต้องยาวอย่างน้อย ${MIN_PASSWORD_LENGTH} ตัวอักษร`
    }
    if (err.message === 'not authenticated') {
      return 'เซสชันหมดอายุ — ออกจากระบบแล้วเข้าสู่ระบบใหม่อีกครั้ง'
    }
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}

interface ChangePasswordDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

// เปลี่ยนรหัสผ่านของตัวเอง (ไม่บังคับ — เปิดจากปุ่มกุญแจท้ายเมนู) สำเร็จแล้ว backend เพิกถอน session อื่น
// ทั้งหมดยกเว้นอุปกรณ์นี้ และล้างสถานะ "ยังใช้รหัสชั่วคราว" — รหัสผ่านไม่ถูกเก็บ/log ที่ไหนนอกจากส่งไป backend
export function ChangePasswordDialog({ open, onOpenChange }: ChangePasswordDialogProps) {
  const { user, changePassword } = useAuth()
  const form = useForm<ChangePasswordValues>({
    resolver: zodResolver(changePasswordSchema),
    defaultValues: EMPTY_VALUES,
  })

  // ล้างช่องรหัสทุกครั้งที่เปิด/ปิด ไม่ให้รหัสค้างอยู่ในฟอร์ม
  useEffect(() => {
    form.reset(EMPTY_VALUES)
  }, [open, form])

  const handleSubmit = form.handleSubmit(async (values) => {
    try {
      await changePassword(values.currentPassword, values.newPassword)
      notify.updated('เปลี่ยนรหัสผ่านแล้ว — อุปกรณ์อื่นที่เข้าสู่ระบบไว้ถูกออกจากระบบ')
      onOpenChange(false)
    } catch (err) {
      form.setError('root', { message: changePasswordErrorMessage(err) })
    }
  })

  const passwordField = (
    name: keyof ChangePasswordValues,
    label: string,
    autoComplete: 'current-password' | 'new-password',
  ) => (
    <Controller
      name={name}
      control={form.control}
      render={({ field, fieldState }) => (
        <Field data-invalid={fieldState.invalid}>
          <FieldLabel htmlFor={`change-password-${name}`}>{label}</FieldLabel>
          <Input
            {...field}
            id={`change-password-${name}`}
            type="password"
            autoComplete={autoComplete}
            aria-invalid={fieldState.invalid}
          />
          {fieldState.invalid && <FieldError errors={[fieldState.error]} />}
        </Field>
      )}
    />
  )

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-md">
        <DialogHeader>
          <DialogTitle>เปลี่ยนรหัสผ่าน</DialogTitle>
          <DialogDescription>
            {user?.mustChangePassword
              ? 'คุณยังใช้รหัสผ่านชั่วคราวที่แอดมินตั้งให้ — ควรเปลี่ยนเป็นรหัสของตัวเอง'
              : `รหัสผ่านใหม่ยาวอย่างน้อย ${MIN_PASSWORD_LENGTH} ตัวอักษร`}
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} noValidate>
          {/* ช่วยให้ตัวจัดการรหัสผ่านของเบราว์เซอร์รู้ว่าเป็นรหัสของบัญชีไหน */}
          <input
            type="text"
            name="username"
            autoComplete="username"
            value={user?.username ?? ''}
            readOnly
            hidden
          />
          <FieldGroup>
            {passwordField('currentPassword', 'รหัสผ่านปัจจุบัน', 'current-password')}
            {passwordField('newPassword', `รหัสผ่านใหม่ (อย่างน้อย ${MIN_PASSWORD_LENGTH} ตัว)`, 'new-password')}
            {passwordField('confirmPassword', 'ยืนยันรหัสผ่านใหม่', 'new-password')}
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
              {form.formState.isSubmitting ? 'กำลังบันทึก...' : 'เปลี่ยนรหัสผ่าน'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
