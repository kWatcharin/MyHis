import { ChevronsUpDown, CircleUser, KeyRound, LogOut } from 'lucide-react'

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { SidebarMenuButton } from '@/components/ui/sidebar'
import { useAuth } from '@/features/auth/AuthContext'

interface UserMenuProps {
  onChangePassword: () => void
  // เปิดกล่องยืนยันเท่านั้น ยังไม่ออกจริง
  onLogout: () => void
}

// ปุ่มชื่อผู้ใช้ท้ายเมนูซ้าย กดแล้วเลือก เปลี่ยนรหัสผ่าน / ออกจากระบบ — dialog ทั้งสองอยู่ใน AppShell นอก Sidebar
// (บนมือถือเมนูเป็น sheet ที่ปิดแล้วถูก unmount ถ้า dialog อยู่ในนี้จะหายกลางคัน)
export function UserMenu({ onChangePassword, onLogout }: UserMenuProps) {
  const { user } = useAuth()
  const name = user?.displayName ?? user?.username
  // ยังใช้รหัสผ่านชั่วคราวที่แอดมินตั้งให้ — แค่เตือน ไม่บังคับเปลี่ยน
  const shouldChangePassword = user?.mustChangePassword ?? false

  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        render={
          <SidebarMenuButton
            size="lg"
            className="border bg-sidebar-accent/50 data-popup-open:bg-sidebar-accent"
          >
            <span className="relative">
              <CircleUser className="size-5!" />
              {shouldChangePassword && (
                <span
                  aria-hidden
                  className="absolute -top-0.5 -right-0.5 size-2 rounded-full bg-warning ring-2 ring-sidebar"
                />
              )}
            </span>
            <span className="grid min-w-0 flex-1 leading-tight">
              <span className="truncate font-medium">{name}</span>
              {user?.displayName && (
                <span className="truncate text-xs text-muted-foreground">{user.username}</span>
              )}
              {shouldChangePassword && (
                <span className="sr-only">ยังใช้รหัสผ่านชั่วคราว ควรเปลี่ยน</span>
              )}
            </span>
            <ChevronsUpDown className="ml-auto text-muted-foreground" />
          </SidebarMenuButton>
        }
      />
      <DropdownMenuContent side="top">
        {/* label ต้องอยู่ใน group เสมอ — base-ui throw (error #31) ถ้าไม่มี group ครอบ แล้วทั้งแอปหน้าขาว */}
        <DropdownMenuGroup>
          <DropdownMenuLabel>
            <span className="block truncate font-medium text-foreground">{name}</span>
            {user?.displayName && <span className="block truncate">{user.username}</span>}
          </DropdownMenuLabel>
        </DropdownMenuGroup>
        <DropdownMenuSeparator />
        <DropdownMenuItem onClick={onChangePassword}>
          <KeyRound />
          <span>
            เปลี่ยนรหัสผ่าน
            {shouldChangePassword && (
              <span className="block text-xs text-warning">ยังใช้รหัสชั่วคราว ควรเปลี่ยน</span>
            )}
          </span>
        </DropdownMenuItem>
        <DropdownMenuItem variant="destructive" onClick={onLogout}>
          <LogOut />
          ออกจากระบบ
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
