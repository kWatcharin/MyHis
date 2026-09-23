import { createContext, useCallback, useContext, useEffect, useState } from 'react'
import type { ReactNode } from 'react'

export type Theme = 'dark' | 'ocean'

const STORAGE_KEY = 'my-his-theme'

// ต้องตรงกับ class ที่นิยามไว้ใน src/index.css (.dark, .theme-ocean) และ script bootstrap ใน
// index.html (กันกระพริบตอนโหลดหน้าแรก — แก้ตรงนี้ต้องไปแก้ที่นั่นด้วย)
const THEME_CLASS: Record<Theme, string> = {
  dark: 'dark',
  ocean: 'theme-ocean',
}

interface ThemeContextValue {
  theme: Theme
  setTheme: (theme: Theme) => void
}

const ThemeContext = createContext<ThemeContextValue | null>(null)

function readStoredTheme(): Theme {
  try {
    return localStorage.getItem(STORAGE_KEY) === 'ocean' ? 'ocean' : 'dark'
  } catch {
    return 'dark'
  }
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<Theme>(readStoredTheme)

  useEffect(() => {
    const root = document.documentElement
    for (const cls of Object.values(THEME_CLASS)) {
      root.classList.remove(cls)
    }
    root.classList.add(THEME_CLASS[theme])
    try {
      localStorage.setItem(STORAGE_KEY, theme)
    } catch {
      // localStorage อาจใช้ไม่ได้ (private mode ฯลฯ) — ยังสลับธีมได้ปกติ แค่ไม่จำข้ามรีเฟรช
    }
  }, [theme])

  const setTheme = useCallback((next: Theme) => setThemeState(next), [])

  return <ThemeContext.Provider value={{ theme, setTheme }}>{children}</ThemeContext.Provider>
}

export function useTheme() {
  const ctx = useContext(ThemeContext)
  if (!ctx) {
    throw new Error('useTheme ต้องถูกเรียกภายใน <ThemeProvider>')
  }
  return ctx
}
