"use client"

import { Toaster as Sonner, type ToasterProps } from "sonner"
import { CircleCheckIcon, InfoIcon, TriangleAlertIcon, OctagonXIcon, Loader2Icon } from "lucide-react"

// เอา next-themes ออก — โปรเจกต์นี้ไม่ได้ใช้ next-themes จัดการธีม (ดู
// src/features/theme/ThemeContext.tsx ที่เขียนเองแยกต่างหาก) theme ที่มีตอนนี้ (dark, ocean)
// เป็นพื้นหลังเข้มทั้งคู่ ใส่ "dark" ตรง ๆ พอ ไม่ต้องอ่านจาก provider ที่ไม่มีอยู่จริงในแอปนี้
const Toaster = ({ ...props }: ToasterProps) => {
  return (
    <Sonner
      theme="dark"
      className="toaster group"
      icons={{
        success: (
          <CircleCheckIcon className="size-4" />
        ),
        info: (
          <InfoIcon className="size-4" />
        ),
        warning: (
          <TriangleAlertIcon className="size-4" />
        ),
        error: (
          <OctagonXIcon className="size-4" />
        ),
        loading: (
          <Loader2Icon className="size-4 animate-spin" />
        ),
      }}
      style={
        {
          "--normal-bg": "var(--popover)",
          "--normal-text": "var(--popover-foreground)",
          "--normal-border": "var(--border)",
          "--border-radius": "var(--radius)",
        } as React.CSSProperties
      }
      toastOptions={{
        classNames: {
          toast: "cn-toast",
        },
      }}
      {...props}
    />
  )
}

export { Toaster }
