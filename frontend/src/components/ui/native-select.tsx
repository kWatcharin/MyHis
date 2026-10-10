import * as React from "react"
import { cn } from "cn"

// <select> ของเบราว์เซอร์ที่จัดสไตล์ให้เข้ากับ Input (ขอบ, ความสูง, focus ring, โหมดมืด/สว่าง)
// ใช้แทน dropdown เต็มรูปแบบสำหรับตัวเลือกที่เป็นรายการสั้น ๆ (คำนำหน้า, สัญชาติ, เพศ) — ไม่ต้องเพิ่ม
// dependency และใช้งานบนมือถือได้ดี (ใช้ตัวเลือกของ OS)
function NativeSelect({ className, children, ...props }: React.ComponentProps<"select">) {
  return (
    <select
      data-slot="native-select"
      className={cn(
        "h-8 w-full min-w-0 rounded-lg border border-input bg-transparent px-2 py-1 text-base transition-colors outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive aria-invalid:ring-3 aria-invalid:ring-destructive/20 md:text-sm dark:bg-input/30 dark:aria-invalid:border-destructive/50 dark:aria-invalid:ring-destructive/40 [&>option]:bg-popover [&>option]:text-popover-foreground",
        className
      )}
      {...props}
    >
      {children}
    </select>
  )
}

export { NativeSelect }
