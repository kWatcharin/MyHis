import { createContext, useCallback, useContext, useEffect, useState } from 'react'
import type { ReactNode } from 'react'

import { api, ApiError } from '@/lib/api'

export interface MenuItem {
  id: string
  label: string
  icon: string | null
  route: string
  groupLabel: string | null
}

interface UserResponse {
  id: string
  username: string
  displayName: string
  mustChangePassword: boolean
  permissions: string[]
}

type AuthStatus = 'loading' | 'authenticated' | 'unauthenticated'

interface AuthContextValue {
  status: AuthStatus
  user: UserResponse | null
  menu: MenuItem[]
  hasPermission: (permission: string) => boolean
  login: (username: string, password: string) => Promise<void>
  logout: () => Promise<void>
}

const AuthContext = createContext<AuthContextValue | null>(null)

// backend ตั้งใจให้ error message เดียวกันไม่ว่าจะผิดเพราะอะไร (docs/auth-design.md ข้อ 6) —
// map เป็นข้อความไทยตรงนี้ที่เดียว ไม่โชว์ string ดิบจาก backend ให้ผู้ใช้เห็น
function loginErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    if (err.message === 'invalid credentials') {
      return 'ชื่อผู้ใช้หรือรหัสผ่านไม่ถูกต้อง'
    }
    if (err.message === 'account temporarily locked') {
      return err.retryAfterSeconds
        ? `บัญชีถูกล็อกชั่วคราว ลองใหม่อีกครั้งใน ${err.retryAfterSeconds} วินาที`
        : 'บัญชีถูกล็อกชั่วคราว ลองใหม่อีกครั้งภายหลัง'
    }
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}

export function AuthProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<AuthStatus>('loading')
  const [user, setUser] = useState<UserResponse | null>(null)
  const [menu, setMenu] = useState<MenuItem[]>([])

  // เช็ค session ตอนโหลดแอปครั้งแรก (เช่น refresh หน้า) — ของจริงอิง cookie ที่ JS อ่านไม่ได้
  // เลยต้องถาม backend ผ่าน /auth/me เสมอ อ่าน localStorage เองไม่ได้แบบ mock รอบ 1
  useEffect(() => {
    let cancelled = false

    async function checkSession() {
      try {
        const [me, menuItems] = await Promise.all([
          api.get<UserResponse>('/auth/me'),
          api.get<MenuItem[]>('/menu'),
        ])
        if (cancelled) return
        setUser(me)
        setMenu(menuItems)
        setStatus('authenticated')
      } catch {
        // 401 คือผลลัพธ์ปกติของคนยังไม่ login ไม่ใช่ error ที่ต้องโชว์อะไร
        if (cancelled) return
        setUser(null)
        setMenu([])
        setStatus('unauthenticated')
      }
    }

    void checkSession()
    return () => {
      cancelled = true
    }
  }, [])

  const login = useCallback(async (username: string, password: string) => {
    try {
      const me = await api.post<UserResponse>('/auth/login', { username, password })
      const menuItems = await api.get<MenuItem[]>('/menu')
      setUser(me)
      setMenu(menuItems)
      setStatus('authenticated')
    } catch (err) {
      throw new Error(loginErrorMessage(err))
    }
  }, [])

  const logout = useCallback(async () => {
    try {
      await api.post('/auth/logout')
    } finally {
      setUser(null)
      setMenu([])
      setStatus('unauthenticated')
    }
  }, [])

  const hasPermission = useCallback(
    (permission: string) => user?.permissions.includes(permission) ?? false,
    [user],
  )

  return (
    <AuthContext.Provider value={{ status, user, menu, hasPermission, login, logout }}>
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth() {
  const ctx = useContext(AuthContext)
  if (!ctx) {
    throw new Error('useAuth ต้องถูกเรียกภายใน <AuthProvider>')
  }
  return ctx
}
