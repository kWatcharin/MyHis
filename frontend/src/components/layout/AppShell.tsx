import { Link, Outlet, useLocation } from 'react-router'

import { Button } from '@/components/ui/button'
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
  SidebarProvider,
  SidebarTrigger,
} from '@/components/ui/sidebar'
import { useAuth } from '@/features/auth/AuthContext'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'
import { RoleSwitcher } from '@/features/role-preview/RoleSwitcher'
import { ThemeSwitcher } from '@/features/theme/ThemeSwitcher'
import { resolveIcon } from '@/lib/icons'

// เมนูซ้ายมือ + โครงหน้าที่ต้อง login ทุกหน้า — รายการเมนูใช้ effectiveMenu (สิทธิ์จริง หรือ
// สิทธิ์ที่กำลัง preview อยู่ผ่าน RoleSwitcher มุมขวาบนของ header — ดู RolePreviewContext.tsx)
// ไม่ใช่ useAuth().menu ตรง ๆ
export function AppShell() {
  const { user, logout } = useAuth()
  const { effectiveMenu } = useRolePreview()
  const location = useLocation()

  return (
    <SidebarProvider>
      <Sidebar>
        <SidebarHeader className="px-2 py-1.5 text-sm font-semibold">my-his</SidebarHeader>
        <SidebarContent>
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                {effectiveMenu.map((item) => {
                  const Icon = resolveIcon(item.icon)
                  return (
                    <SidebarMenuItem key={item.id}>
                      <SidebarMenuButton
                        render={<Link to={item.route} />}
                        isActive={location.pathname === item.route}
                      >
                        <Icon />
                        <span>{item.label}</span>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  )
                })}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>
        <SidebarFooter className="gap-2">
          <ThemeSwitcher />
          <p className="truncate px-2 text-sm text-muted-foreground">
            {user?.displayName ?? user?.username}
          </p>
          <Button variant="outline" size="sm" onClick={() => void logout()}>
            ออกจากระบบ
          </Button>
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
        <main className="flex-1 p-4">
          <Outlet />
        </main>
      </SidebarInset>
    </SidebarProvider>
  )
}
