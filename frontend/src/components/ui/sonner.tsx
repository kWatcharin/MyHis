"use client"

import { Toaster as Sonner, type ToasterProps } from "sonner"
import { isDarkTheme, useTheme } from "@/features/theme/ThemeContext"
import { CircleCheckIcon, InfoIcon, TriangleAlertIcon, OctagonXIcon, Loader2Icon } from "lucide-react"

// เอา next-themes ออก — โปรเจกต์นี้ไม่ได้ใช้ next-themes จัดการธีม ใช้ ThemeContext ที่เขียนเอง
// (src/features/theme/ThemeContext.tsx) แทน: ธีม light = toast สว่าง นอกนั้น (dark, ocean) = toast มืด
const Toaster = ({ ...props }: ToasterProps) => {
  const { theme } = useTheme()
  return (
    <Sonner
      theme={isDarkTheme(theme) ? "dark" : "light"}
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
          // sonner บังคับกล่องไอคอนกว้าง 16px — ขยายเป็น 24px ให้พอดีกับไอคอนวงกลมสีของ lib/notify.tsx
          icon: "size-6!",
        },
      }}
      {...props}
    />
  )
}

export { Toaster }
