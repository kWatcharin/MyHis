import * as icons from 'lucide-react'
import type { ComponentType, SVGProps } from 'react'

type IconComponent = ComponentType<SVGProps<SVGSVGElement>>

function toPascalCase(kebab: string): string {
  return kebab
    .split('-')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join('')
}

// icon ที่ backend seed ไว้เป็นชื่อ slug ของ lucide (เช่น "layout-list" -> "LayoutList") ใช้ได้กับ
// ทุก icon ที่ admin เพิ่มเองทีหลังผ่าน POST /pages ด้วย ไม่ใช่แค่ 3 หน้าที่ seed ไว้ตอนนี้ — ชื่อที่
// ไม่รู้จัก/ไม่มี icon (null) fallback เป็น Circle
export function resolveIcon(name: string | null): IconComponent {
  if (!name) return icons.Circle as IconComponent
  const key = toPascalCase(name)
  const icon = (icons as unknown as Record<string, IconComponent>)[key]
  return icon ?? (icons.Circle as IconComponent)
}
