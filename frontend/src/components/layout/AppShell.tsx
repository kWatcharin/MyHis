import { ChevronRight, HeartPulse, KeyRound, LogOut } from 'lucide-react'
import { useState } from 'react'
import { Link, Outlet, useLocation } from 'react-router'

import { Button } from '@/components/ui/button'
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
import type { MenuItem } from '@/features/auth/AuthContext'
import { ChangePasswordDialog } from '@/features/auth/ChangePasswordDialog'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'
import { RoleSwitcher } from '@/features/role-preview/RoleSwitcher'
import { ThemeSwitcher } from '@/features/theme/ThemeSwitcher'
import { resolveIcon } from '@/lib/icons'

// เมนูซ้ายมือ + โครงหน้าที่ต้อง login ทุกหน้า — รายการเมนูใช้ effectiveMenu (สิทธิ์จริง หรือ
// สิทธิ์ที่กำลัง preview อยู่ผ่าน RoleSwitcher มุมขวาบนของ header — ดู RolePreviewContext.tsx)
// ไม่ใช่ useAuth().menu ตรง ๆ
// จัดกลุ่มเมนูตาม groupLabel — เก็บลำดับเดิมจาก array ที่ backend เรียง sort_order มาให้แล้ว
// (Map เก็บลำดับ key ตามที่ insert ครั้งแรก) groupLabel ว่าง/null รวมกันเป็นกลุ่ม "ไม่มีชื่อ"
// (เช่น Dashboard) แสดงแบบไม่มีหัวข้อคั่น
function groupMenuItems(items: MenuItem[]): [string | null, MenuItem[]][] {
  const groups = new Map<string | null, MenuItem[]>()
  for (const item of items) {
    const bucket = groups.get(item.groupLabel)
    if (bucket) {
      bucket.push(item)
    } else {
      groups.set(item.groupLabel, [item])
    }
  }
  return [...groups.entries()]
}

// เมนูถือว่า active เมื่ออยู่หน้านั้นพอดีหรือหน้าย่อยของมัน (เช่น /lab/appointments/<id> ยังไฮไลต์ "นัดหมายแล็ป")
function isRouteActive(pathname: string, route: string): boolean {
  return pathname === route || pathname.startsWith(`${route}/`)
}

export function AppShell() {
  const { user, logout } = useAuth()
  const { effectiveMenu } = useRolePreview()
  const location = useLocation()
  const menuGroups = groupMenuItems(effectiveMenu)
  const [isChangingPassword, setIsChangingPassword] = useState(false)
  // ยังใช้รหัสผ่านชั่วคราวที่แอดมินตั้งให้ — แค่เตือนที่ปุ่ม ไม่บังคับเปลี่ยน
  const shouldChangePassword = user?.mustChangePassword ?? false

  return (
    <SidebarProvider>
      <Sidebar>
        <SidebarHeader className="flex-row items-center gap-2 px-3 py-3">
          <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <HeartPulse className="size-4" />
          </span>
          <span className="text-sm font-semibold tracking-tight">my-his</span>
        </SidebarHeader>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                {menuGroups.map(([groupLabel, items]) =>
                  groupLabel === null ? (
                    items.map((item) => {
                      const Icon = resolveIcon(item.icon)
                      return (
                        <SidebarMenuItem key={item.id}>
                          <SidebarMenuButton
                            render={<Link to={item.route} />}
                            isActive={isRouteActive(location.pathname, item.route)}
                          >
                            <Icon />
                            <span>{item.label}</span>
                          </SidebarMenuButton>
                        </SidebarMenuItem>
                      )
                    })
                  ) : (
                    // กลุ่มที่มีชื่อ (เช่น "การตั้งค่า") = main page เดียวที่กด toggle เปิด/ปิด
                    // ดู sub page ข้างใน ไม่ใช่ลิงก์ที่คลิกแล้วมีเนื้อหาของตัวเอง — defaultOpen
                    // เสมอกันคนเผลอมองไม่เห็นเมนูตั้งค่าที่เคยกดใช้อยู่แล้ว
                    <Collapsible key={groupLabel} defaultOpen className="group/collapsible">
                      <SidebarMenuItem>
                        <CollapsibleTrigger
                          render={
                            <SidebarMenuButton
                              isActive={items.some(
                                (item) => isRouteActive(location.pathname, item.route),
                              )}
                            >
                              <span>{groupLabel}</span>
                              <ChevronRight className="ml-auto transition-transform group-data-[state=open]/collapsible:rotate-90" />
                            </SidebarMenuButton>
                          }
                        />
                        <CollapsibleContent>
                          <SidebarMenuSub>
                            {items.map((item) => {
                              const Icon = resolveIcon(item.icon)
                              return (
                                <SidebarMenuSubItem key={item.id}>
                                  <SidebarMenuSubButton
                                    render={<Link to={item.route} />}
                                    isActive={isRouteActive(location.pathname, item.route)}
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
                  ),
                )}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarFooter className="gap-3">
          <ThemeSwitcher />
          <div className="flex items-center justify-between gap-2 rounded-lg border bg-sidebar-accent/50 px-2.5 py-2">
            <p className="truncate text-sm font-medium">
              {user?.displayName ?? user?.username}
            </p>
            <div className="flex shrink-0 items-center">
              <Button
                variant="ghost"
                size="icon-sm"
                className="relative"
                onClick={() => setIsChangingPassword(true)}
                title={
                  shouldChangePassword
                    ? 'เปลี่ยนรหัสผ่าน — ยังใช้รหัสชั่วคราวอยู่ ควรเปลี่ยน'
                    : 'เปลี่ยนรหัสผ่าน'
                }
              >
                <KeyRound className="size-4" />
                {shouldChangePassword && (
                  <span
                    aria-hidden
                    className="absolute top-1 right-1 size-2 rounded-full bg-warning ring-2 ring-sidebar"
                  />
                )}
                <span className="sr-only">
                  เปลี่ยนรหัสผ่าน{shouldChangePassword ? ' (ยังใช้รหัสชั่วคราว ควรเปลี่ยน)' : ''}
                </span>
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={() => void logout()}
                title="ออกจากระบบ"
              >
                <LogOut className="size-4" />
                <span className="sr-only">ออกจากระบบ</span>
              </Button>
            </div>
          </div>
        </SidebarFooter>
      </Sidebar>
      <SidebarInset>
        <header className="flex h-12 shrink-0 items-center gap-2 border-b px-4">
          <SidebarTrigger />
          <span className="text-sm font-medium">my-his</span>
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
    </SidebarProvider>
  )
}
