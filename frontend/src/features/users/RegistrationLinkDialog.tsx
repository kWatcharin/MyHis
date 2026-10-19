import { Check, Copy, Link2, TriangleAlert } from 'lucide-react'
import { useRef, useState } from 'react'

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogMedia,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { formatThaiDateTime } from '@/lib/date'

import { registrationUrl } from './usersApi'

export interface IssuedLink {
  username: string
  token: string
  expiresAt: string
  // created = บัญชีใหม่, reissued = ออกลิงก์ลงทะเบียนใหม่ (ยังไม่เคยลงทะเบียน), reset = ตั้งรหัสใหม่หลังรีเซ็ต
  reason: 'created' | 'reissued' | 'reset'
}

interface RegistrationLinkDialogProps {
  // null = ปิด — ผู้เรียกล้างค่าเป็น null ตอนปิด token จึงไม่ค้างอยู่ใน state
  value: IssuedLink | null
  onClose: () => void
}

const TITLES: Record<IssuedLink['reason'], string> = {
  created: 'ลิงก์ลงทะเบียน',
  reissued: 'ลิงก์ลงทะเบียนใหม่',
  reset: 'ลิงก์ตั้งรหัสผ่านใหม่',
}

const DESCRIPTIONS: Record<IssuedLink['reason'], string> = {
  created: 'สร้างบัญชีแล้ว — ส่งลิงก์นี้ให้ผู้ใช้เปิดเพื่อกรอกข้อมูลตัวเองและตั้งรหัสผ่านเอง',
  reissued: 'ลิงก์เดิมใช้ไม่ได้แล้ว — ส่งลิงก์ใหม่นี้ให้ผู้ใช้เปิดเพื่อลงทะเบียน',
  reset: 'รหัสผ่านเดิมใช้ไม่ได้แล้วและผู้ใช้ถูกออกจากระบบทุกอุปกรณ์ — ส่งลิงก์นี้ให้ผู้ใช้ตั้งรหัสผ่านใหม่เอง',
}

// ระบบเปิดผ่านเครื่องนี้เอง — ลิงก์จะชี้ไป localhost ซึ่งอุปกรณ์อื่น (มือถือ) เปิดไม่ได้
function isLocalOrigin(): boolean {
  return ['localhost', '127.0.0.1', '[::1]', '::1'].includes(window.location.hostname)
}

// แสดงลิงก์ลงทะเบียน/ตั้งรหัสผ่าน **ครั้งเดียว** — backend เก็บแค่ hash ของ token ปิดแล้วดูอีกไม่ได้ (ออกใหม่ได้)
// AlertDialog: กดนอกกรอบแล้วไม่ปิดเอง (กันเผลอปิดก่อนคัดลอก) ไม่ใส่ลิงก์ใน toast/console/storage
export function RegistrationLinkDialog({ value, onClose }: RegistrationLinkDialogProps) {
  return (
    <AlertDialog
      open={value !== null}
      onOpenChange={(open) => {
        if (!open) onClose()
      }}
    >
      <AlertDialogContent className="sm:max-w-lg">
        {/* body ถูก unmount ตอนปิด (value = null) — สถานะ "คัดลอกแล้ว" จึงเริ่มใหม่ทุกครั้งที่เปิด */}
        {value && <RegistrationLinkBody value={value} onClose={onClose} />}
      </AlertDialogContent>
    </AlertDialog>
  )
}

type CopyState = 'idle' | 'copied' | 'manual'

function RegistrationLinkBody({ value, onClose }: { value: IssuedLink; onClose: () => void }) {
  const inputRef = useRef<HTMLInputElement>(null)
  const [copyState, setCopyState] = useState<CopyState>('idle')
  const url = registrationUrl(value.token)

  // clipboard API ใช้ได้เฉพาะ secure context (https หรือ localhost) — ไม่มีก็เลือกข้อความให้ผู้ใช้กดคัดลอกเอง
  const handleCopy = async () => {
    try {
      if (!navigator.clipboard) throw new Error('clipboard unavailable')
      await navigator.clipboard.writeText(url)
      setCopyState('copied')
    } catch {
      inputRef.current?.select()
      setCopyState('manual')
    }
  }

  return (
    <>
      <AlertDialogHeader>
        <AlertDialogMedia className="bg-primary/10 text-primary">
          <Link2 />
        </AlertDialogMedia>
        <AlertDialogTitle>
          {TITLES[value.reason]}ของ {value.username}
        </AlertDialogTitle>
        <AlertDialogDescription>{DESCRIPTIONS[value.reason]}</AlertDialogDescription>
      </AlertDialogHeader>

      <div className="flex gap-2">
        <Input
          ref={inputRef}
          readOnly
          value={url}
          aria-label="ลิงก์ลงทะเบียน"
          autoComplete="off"
          spellCheck={false}
          className="h-10 font-mono text-xs"
          onFocus={(event) => event.currentTarget.select()}
        />
        <Button type="button" variant="outline" className="h-10" onClick={() => void handleCopy()}>
          {copyState === 'copied' ? <Check /> : <Copy />}
          {copyState === 'copied' ? 'คัดลอกแล้ว' : 'คัดลอก'}
        </Button>
      </div>
      {copyState === 'manual' && (
        <p className="text-xs text-muted-foreground">
          คัดลอกอัตโนมัติไม่ได้ในหน้านี้ — เลือกข้อความไว้ให้แล้ว กด Ctrl+C (หรือแตะค้างแล้วเลือกคัดลอกบนมือถือ)
        </p>
      )}
      <ul className="list-disc space-y-1 pl-5 text-sm text-muted-foreground">
        <li>
          ใช้ได้ครั้งเดียว หมดอายุ {formatThaiDateTime(value.expiresAt)} — แสดงครั้งเดียว ปิดหน้าต่างนี้แล้วดูอีกไม่ได้
          (ถ้าทำหายให้ออกลิงก์ใหม่)
        </li>
        <li>ใครได้ลิงก์นี้ไปก็ตั้งรหัสผ่านของบัญชีนี้ได้ — ส่งถึงตัวผู้ใช้ทางช่องทางที่ปลอดภัยเท่านั้น</li>
      </ul>
      {isLocalOrigin() && (
        <p className="flex gap-2 rounded-lg border border-warning/40 bg-warning/10 p-2.5 text-sm">
          <TriangleAlert className="mt-0.5 size-4 shrink-0 text-warning-ink" />
          <span>
            ตอนนี้เปิดระบบผ่าน localhost — ลิงก์นี้เปิดได้เฉพาะบนเครื่องนี้ จะส่งให้มือถือ/เครื่องอื่นได้เมื่อระบบเปิดผ่าน
            โดเมน HTTPS
          </span>
        </p>
      )}

      <AlertDialogFooter>
        <AlertDialogAction onClick={onClose}>ปิด</AlertDialogAction>
      </AlertDialogFooter>
    </>
  )
}
