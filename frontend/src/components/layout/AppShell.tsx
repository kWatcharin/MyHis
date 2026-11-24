import { ChevronRight } from 'lucide-react'
import { useState } from 'react'
import { Link, Outlet, useLocation } from 'react-router'

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/components/ui/collapsible'
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarMenuSub,
  SidebarMenuSubButton,
  SidebarMenuSubItem,
  SidebarProvider,
  SidebarTrigger,
} from '@/components/ui/sidebar'
import { useAuth } from '@/features/auth/AuthContext'
import { ChangePasswordDialog } from '@/features/auth/ChangePasswordDialog'
import { buildMenuTree, isRouteActive } from '@/features/auth/menu'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'
import { RoleSwitcher } from '@/features/role-preview/RoleSwitcher'
import { ThemeSwitcher } from '@/features/theme/ThemeSwitcher'
import { resolveIcon } from '@/lib/icons'

import { BrandMark } from './BrandMark'
import { UserMenu } from './UserMenu'

// เมนูซ้ายมือ + โครงหน้าที่ต้อง login ทุกหน้า — รายการเมนูใช้ effectiveMenu (สิทธิ์จริง หรือ
// สิทธิ์ที่กำลัง preview อยู่ผ่าน RoleSwitcher มุมขวาบนของ header — ดู RolePreviewContext.tsx)
// ไม่ใช่ useAuth().menu ตรง ๆ จัดเป็นต้นไม้ 2 ชั้นตาม refId (เพจหลัก → เพจย่อย) ด้วย buildMenuTree

// ลิงก์ที่ active (หน้าปัจจุบัน): ตัวอักษร+ไอคอนสี primary ของธีมบนพื้น accent ให้เห็นทันทีว่าอยู่หน้าไหน
const ACTIVE_LINK =
  'data-active:font-medium data-active:text-sidebar-primary data-active:[&>svg]:text-sidebar-primary'

export function AppShell() {
  const { logout } = useAuth()
  const { effectiveMenu } = useRolePreview()
  const location = useLocation()
  const menuTree = buildMenuTree(effectiveMenu)
  const [isChangingPassword, setIsChangingPassword] = useState(false)
  const [isConfirmingLogout, setIsConfirmingLogout] = useState(false)
  const [isLoggingOut, setIsLoggingOut] = useState(false)

  const handleLogout = async () => {
    setIsLoggingOut(true)
    try {
      await logout()
    } catch {
      // logout() ล้างสถานะฝั่งหน้าเว็บแล้วไม่ว่า POST จะสำเร็จไหม — RequireAuth พาไปหน้า login เอง
    } finally {
      setIsLoggingOut(false)
    }
  }

  return (
    <SidebarProvider>
      <Sidebar>
        <SidebarHeader className="flex-row items-center gap-2.5 px-3 py-3">
          <BrandMark size="sm" />
          <span className="text-base font-semibold tracking-tight">my-his</span>
        </SidebarHeader>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                {menuTree.map(({ item: main, children }) => {
                  if (children.length === 0) {
                    // เพจหลักที่เป็นลิงก์เดี่ยว (เช่น Dashboard) — หัวข้อกลุ่มที่ไม่มีเพจย่อยให้เห็นไม่แสดง
                    if (main.route === null) return null
                    const route = main.route
                    const Icon = resolveIcon(main.icon)
                    return (
                      <SidebarMenuItem key={main.id}>
                        <SidebarMenuButton
                          render={<Link to={route} />}
                          isActive={isRouteActive(location.pathname, route)}
                          className={ACTIVE_LINK}
                        >
                          <Icon />
                          <span>{main.label}</span>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    )
                  }
                  // เพจหลักที่มีเพจย่อย (เช่น "การตั้งค่า") = หัวข้อที่กด toggle เปิด/ปิดดูเพจย่อยข้างใน
                  // ไม่ใช่ลิงก์ที่คลิกแล้วมีเนื้อหาของตัวเอง — defaultOpen เสมอกันคนเผลอมองไม่เห็นเมนูที่เคย
                  // กดใช้อยู่แล้ว ไอคอนของหัวข้อแสดงเฉพาะเมื่อ admin ตั้งไว้ (ค่าเริ่มต้นไม่มี)
                  const MainIcon = main.icon ? resolveIcon(main.icon) : null
                  return (
                    <Collapsible key={main.id} defaultOpen className="group/collapsible">
                      <SidebarMenuItem>
                        <CollapsibleTrigger
                          render={
                            <SidebarMenuButton
                              isActive={children.some((item) =>
                                isRouteActive(location.pathname, item.route),
                              )}
                              // หัวกลุ่ม = ตัวพับ/กาง ไม่ใช่หน้า — ตัวหนาสีจาง ไม่ทาพื้นตอนเพจย่อย active (เพจย่อยทาเอง)
                              // แค่เปลี่ยนตัวอักษรเป็นสี primary ให้รู้ว่าอยู่กลุ่มไหนแม้พับไว้
                              className="font-semibold text-sidebar-foreground/75 data-active:bg-transparent data-active:text-sidebar-primary"
                            >
                              {MainIcon && <MainIcon />}
                              <span>{main.label}</span>
                              {/* Collapsible ของ base-ui ใส่ data-open ที่ตัวครอบตอนกาง (ไม่ใช่ data-state แบบ radix) */}
                              <ChevronRight className="ml-auto transition-transform duration-200 group-data-open/collapsible:rotate-90" />
                            </SidebarMenuButton>
                          }
                        />
                        <CollapsibleContent>
                          <SidebarMenuSub>
                            {children.map((item) => {
                              const Icon = resolveIcon(item.icon)
                              return (
                                <SidebarMenuSubItem key={item.id}>
                                  <SidebarMenuSubButton
                                    render={<Link to={item.route} />}
                                    isActive={isRouteActive(location.pathname, item.route)}
                                    className={ACTIVE_LINK}
                                  >
                                    <Icon />
                                    <span>{item.label}</span>
                                  </SidebarMenuSubButton>
                                </SidebarMenuSubItem>
                              )
                            })}
                          </SidebarMenuSub>
                        </CollapsibleContent>
                      </SidebarMenuItem>
                    </Collapsible>
                  )
                })}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarFooter className="gap-3">
          <ThemeSwitcher />
          <SidebarMenu>
            <SidebarMenuItem>
              <UserMenu
                onChangePassword={() => setIsChangingPassword(true)}
                onLogout={() => setIsConfirmingLogout(true)}
              />
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarFooter>
      </Sidebar>
      <SidebarInset>
        <header className="flex h-12 shrink-0 items-center gap-2 border-b px-4">
          <SidebarTrigger />
          {/* จอกว้างมีชื่อใน sidebar อยู่แล้ว — บนมือถือ sidebar เป็น sheet ที่ซ่อนอยู่ จึงแสดงชื่อที่นี่แทน */}
          <span className="text-sm font-semibold tracking-tight md:hidden">my-his</span>
          <div className="ml-auto">
            <RoleSwitcher />
          </div>
        </header>
        <main className="flex-1 p-4 md:p-6">
          <div className="mx-auto w-full max-w-6xl">
            <Outlet />
          </div>
        </main>
      </SidebarInset>
      {/* อยู่นอก Sidebar — บนมือถือเมนูเป็น sheet ที่ปิดแล้วถูก unmount ถ้าวางไว้ข้างในฟอร์มจะหายกลางคัน */}
      <ChangePasswordDialog open={isChangingPassword} onOpenChange={setIsChangingPassword} />
      <AlertDialog
        open={isConfirmingLogout}
        onOpenChange={(open) => {
          if (!open && !isLoggingOut) setIsConfirmingLogout(false)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>ยืนยันออกจากระบบ</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการออกจากระบบใช่ไหม? ต้องเข้าสู่ระบบใหม่ถึงจะใช้งานต่อได้
              (อุปกรณ์อื่นที่เข้าสู่ระบบไว้ไม่ถูกออกด้วย)
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={isLoggingOut}>ยกเลิก</AlertDialogCancel>
            <AlertDialogAction
              variant="danger"
              onClick={() => void handleLogout()}
              disabled={isLoggingOut}
            >
              {isLoggingOut ? 'กำลังออกจากระบบ...' : 'ออกจากระบบ'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </SidebarProvider>
  )
}
