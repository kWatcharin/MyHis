import { Eye, EyeOff } from 'lucide-react'
import { useState } from 'react'
import type { ComponentProps } from 'react'

import { InputGroup, InputGroupAddon, InputGroupButton, InputGroupInput } from '@/components/ui/input-group'

// ช่องรหัสผ่านที่มีปุ่มรูปตาทางขวาสลับแสดง/ซ่อนรหัส (เริ่มที่ซ่อนเสมอ ปิดฟอร์ม/ออกจากหน้าแล้วกลับเป็นซ่อน)
// ใช้แทน <Input type="password"> ได้ตรง ๆ (รับ props เดิมทั้งหมด รวม {...field} ของ react-hook-form)
// - ปิด spellcheck/autocorrect เสมอ: ตอนแสดงเป็น type="text" เบราว์เซอร์อาจส่งข้อความไปตรวจคำสะกดที่เซิร์ฟเวอร์ภายนอก
// - ซ่อนปุ่มรูปตาของ Edge เอง (::-ms-reveal) ไม่ให้มีสองปุ่มซ้อนกัน
export function PasswordInput({ className, ...props }: Omit<ComponentProps<'input'>, 'type'>) {
  const [visible, setVisible] = useState(false)
  const label = visible ? 'ซ่อนรหัสผ่าน' : 'แสดงรหัสผ่าน'

  return (
    <InputGroup className={className}>
      <InputGroupInput
        {...props}
        type={visible ? 'text' : 'password'}
        spellCheck={false}
        autoCorrect="off"
        autoCapitalize="off"
        className="[&::-ms-clear]:hidden [&::-ms-reveal]:hidden"
      />
      <InputGroupAddon align="inline-end">
        <InputGroupButton
          size="icon-xs"
          aria-label={label}
          aria-pressed={visible}
          title={label}
          disabled={props.disabled}
          onClick={() => setVisible((value) => !value)}
        >
          {visible ? <EyeOff /> : <Eye />}
        </InputGroupButton>
      </InputGroupAddon>
    </InputGroup>
  )
}
