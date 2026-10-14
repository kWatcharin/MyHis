import { createContext, useCallback, useContext, useEffect, useState } from 'react'
import type { ReactNode } from 'react'

import { api, ApiError } from '@/lib/api'

import { toLoginError } from './loginError'

// เมนู 2 ชั้น (ดู menu.ts): refId = uuid ของเพจหลัก (null = เมนูชั้นบน), route null = หัวข้อกลุ่ม
export interface MenuItem {
  id: string
  label: string
  icon: string | null
  route: string | null
  refId: string | null
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
  // turnstileToken: token ของ widget Cloudflare Turnstile (ไม่ส่งเมื่อ backend ปิดฟีเจอร์)
  login: (username: string, password: string, turnstileToken?: string) => Promise<void>
  logout: () => Promise<void>
  // โหลดข้อมูลผู้ใช้ (สิทธิ์) และเมนูใหม่จาก backend โดยไม่กลับไปสถานะ loading — เรียกหลังแก้ role/เพจ
  // เพื่อให้เมนูซ้ายและสิทธิ์ที่ใช้งานอยู่สะท้อนการเปลี่ยนแปลงทันที ไม่ต้อง login ใหม่
  refresh: () => Promise<void>
  // เปลี่ยนรหัสผ่านของตัวเอง (POST /auth/password) แล้วโหลดข้อมูลผู้ใช้ใหม่ (mustChangePassword → false)
  // backend เพิกถอน session อื่นทั้งหมดยกเว้นอันนี้ — error โยนเป็น ApiError ให้ผู้เรียกแปลข้อความเอง
  changePassword: (currentPassword: string, newPassword: string) => Promise<void>
}

const AuthContext = createContext<AuthContextValue | null>(null)

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

  const login = useCallback(async (username: string, password: string, turnstileToken?: string) => {
    try {
      const me = await api.post<UserResponse>('/auth/login', { username, password, turnstileToken })
      const menuItems = await api.get<MenuItem[]>('/menu')
      setUser(me)
      setMenu(menuItems)
      setStatus('authenticated')
    } catch (err) {
      // LoginError: ข้อความไทย + จำนวนครั้งที่เหลือ/เวลาล็อก ให้หน้า login แสดงตัวนับ
      throw toLoginError(err)
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

  const refresh = useCallback(async () => {
    try {
      const [me, menuItems] = await Promise.all([
        api.get<UserResponse>('/auth/me'),
        api.get<MenuItem[]>('/menu'),
      ])
      setUser(me)
      setMenu(menuItems)
    } catch (err) {
      // session หมดอายุระหว่างใช้งาน (401) → กลับไปหน้า login ส่วน error อื่นปล่อยข้อมูลเดิมไว้
      if (err instanceof ApiError && err.status === 401) {
        setUser(null)
        setMenu([])
        setStatus('unauthenticated')
      }
    }
  }, [])

  const changePassword = useCallback(
    async (currentPassword: string, newPassword: string) => {
      await api.post('/auth/password', { currentPassword, newPassword })
      await refresh()
    },
    [refresh],
  )

  const hasPermission = useCallback(
    (permission: string) => user?.permissions.includes(permission) ?? false,
    [user],
  )

  return (
    <AuthContext.Provider
      value={{ status, user, menu, hasPermission, login, logout, refresh, changePassword }}
    >
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
