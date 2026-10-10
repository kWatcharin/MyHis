import { cn } from '@/lib/utils'

import type { Theme } from './ThemeContext'
import { useTheme } from './ThemeContext'

const OPTIONS: { value: Theme; label: string; swatch: string }[] = [
  { value: 'light', label: 'สว่าง', swatch: '#f4f6fb' },
  { value: 'dark', label: 'มืด', swatch: '#171717' },
  { value: 'ocean', label: 'ทะเล', swatch: '#15d8b3' },
]

// ตัวเลือกธีมแบบวงกลมสี — เรียบง่ายกว่า dropdown สำหรับตัวเลือกไม่กี่อัน ไม่ต้องเพิ่ม shadcn
// component ใหม่ วงแหวนรอบวงกลมบอกว่าอันไหน active อยู่
export function ThemeSwitcher() {
  const { theme, setTheme } = useTheme()

  return (
    <div className="flex items-center gap-1.5 px-2">
      <span className="text-xs text-muted-foreground">ธีม</span>
      {OPTIONS.map((option) => (
        <button
          key={option.value}
          type="button"
          aria-pressed={theme === option.value}
          title={option.label}
          onClick={() => setTheme(option.value)}
          className={cn(
            'size-5 rounded-full border border-border ring-2 ring-offset-2 ring-offset-sidebar transition-colors',
            theme === option.value ? 'ring-foreground' : 'ring-transparent',
          )}
          style={{ backgroundColor: option.swatch }}
        >
          <span className="sr-only">ธีม{option.label}</span>
        </button>
      ))}
    </div>
  )
}
