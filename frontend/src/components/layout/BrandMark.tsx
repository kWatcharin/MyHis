import { HeartPulse } from 'lucide-react'

import { cn } from '@/lib/utils'

// สัญลักษณ์ my-his (หัวใจกับเส้นชีพจรในกล่องสี primary) — ใช้ที่เดียวนี้ทุกจุด ขนาด/มุมโค้งจะได้ตรงกันทั้งแอป
// sm = หัว sidebar, md = หัวการ์ดหน้าทางเข้า (แถบ monitor)
const SIZE = {
  sm: 'size-8 [&_svg]:size-4.5',
  md: 'size-10 [&_svg]:size-5',
} as const

export function BrandMark({
  size = 'sm',
  className,
  iconClassName,
}: {
  size?: keyof typeof SIZE
  className?: string
  iconClassName?: string
}) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        'flex shrink-0 items-center justify-center rounded-lg bg-primary text-primary-foreground',
        SIZE[size],
        className,
      )}
    >
      <HeartPulse className={iconClassName} />
    </span>
  )
}
