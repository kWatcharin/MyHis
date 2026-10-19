import { cn } from '@/lib/utils'

import type { Theme } from './ThemeContext'
import { useTheme } from './ThemeContext'

// สีพื้น + สีเด่น (primary) ของแต่ละธีม — ค่าเดียวกับ --background / --primary ใน index.css
const OPTIONS: { value: Theme; label: string; base: string; accent: string }[] = [
  { value: 'light', label: 'สว่าง', base: 'oklch(0.985 0.004 250)', accent: '#2f39a9' },
  { value: 'dark', label: 'มืด', base: 'oklch(0.16 0.012 265)', accent: 'oklch(0.76 0.105 272)' },
  { value: 'ocean', label: 'ทะเล', base: 'oklch(0.16 0.04 258)', accent: '#15d8b3' },
]

// ตัวเลือกธีมแบบวงกลมสี — เรียบง่ายกว่า dropdown สำหรับตัวเลือกไม่กี่อัน ไม่ต้องเพิ่ม shadcn
// component ใหม่ วงกลมแบ่งครึ่งสีพื้น/สีเด่นของธีม (มองเห็นได้บนทุกพื้น) วงแหวนบอกว่าอันไหน active อยู่
export function ThemeSwitcher() {
  const { theme, setTheme } = useTheme()

  return (
    <div className="flex items-center gap-2 px-2">
      <span className="text-xs text-muted-foreground">ธีม</span>
      {OPTIONS.map((option) => (
        <button
          key={option.value}
          type="button"
          aria-pressed={theme === option.value}
          title={option.label}
          onClick={() => setTheme(option.value)}
          className={cn(
            'size-6 rounded-full border border-sidebar-border ring-2 ring-offset-2 ring-offset-sidebar transition-shadow',
            theme === option.value ? 'ring-sidebar-primary' : 'ring-transparent hover:ring-sidebar-border',
          )}
          style={{
            background: `linear-gradient(135deg, ${option.base} 50%, ${option.accent} 50%)`,
          }}
        >
          <span className="sr-only">ธีม{option.label}</span>
        </button>
      ))}
    </div>
  )
}
