import { ChevronRight } from 'lucide-react'
import { Link } from 'react-router'

import { PageHeader } from '@/components/layout/PageHeader'
import { useAuth } from '@/features/auth/AuthContext'
import { buildMenuTree } from '@/features/auth/menu'
import type { MenuLink } from '@/features/auth/menu'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'
import { resolveIcon } from '@/lib/icons'

// หน้าแรกหลัง login = ทางลัดไปงานประจำ (PRODUCT.md: งานจบเร็วบนมือถือ มือเดียว) — รายการหน้าที่ผู้ใช้คนนี้เข้าได้จริง
// จากเมนูเดียวกับ sidebar (effectiveMenu: สิทธิ์จริง หรือ role ที่กำลัง preview) จัดตามกลุ่มเมนู ไม่มีตัวเลขสรุป
// จนกว่าจะมีข้อมูลจริงให้สรุป (ตารางเวร ฯลฯ) — ห้ามใส่ตัวเลข/กราฟตัวอย่างที่แต่งขึ้น
const DASHBOARD_ROUTE = '/dashboard'

interface ShortcutGroup {
  id: string
  label: string | null
  links: MenuLink[]
}

function toGroups(tree: ReturnType<typeof buildMenuTree>): ShortcutGroup[] {
  const groups: ShortcutGroup[] = []
  // ลิงก์เดี่ยวชั้นบน (ไม่อยู่ในกลุ่ม) รวมเป็นกลุ่มแรกไม่มีหัวข้อ
  const standalone: MenuLink[] = []
  for (const { item, children } of tree) {
    if (children.length === 0) {
      if (item.route !== null && item.route !== DASHBOARD_ROUTE) {
        standalone.push({ ...item, route: item.route })
      }
      continue
    }
    const links = children.filter((link) => link.route !== DASHBOARD_ROUTE)
    if (links.length > 0) groups.push({ id: item.id, label: item.label, links })
  }
  return standalone.length > 0 ? [{ id: 'standalone', label: null, links: standalone }, ...groups] : groups
}

export function DashboardPage() {
  const { user } = useAuth()
  const { effectiveMenu } = useRolePreview()
  const groups = toGroups(buildMenuTree(effectiveMenu))

  return (
    <div className="flex flex-col gap-6">
      <PageHeader
        fallbackTitle="Dashboard"
        description={`สวัสดี ${user?.displayName ?? user?.username ?? ''}`}
      />

      {groups.length === 0 ? (
        <p className="text-sm text-muted-foreground">ยังไม่มีหน้าที่บัญชีนี้เข้าได้ ติดต่อผู้ดูแลระบบเพื่อขอสิทธิ์</p>
      ) : (
        // จอกว้างวางกลุ่มเป็น 2 คอลัมน์ (กลุ่มไม่แตกกลางคอลัมน์) มือถือเรียงเป็นรายการเดียว
        <div className="gap-x-8 md:columns-2">
          {groups.map((group) => (
            <section key={group.id} className="mb-6 break-inside-avoid">
              {group.label && (
                <h2 className="mb-1 px-3 text-xs font-medium text-muted-foreground">{group.label}</h2>
              )}
              <ul className="divide-y rounded-lg border">
                {group.links.map((link) => {
                  const Icon = resolveIcon(link.icon)
                  return (
                    <li key={link.id}>
                      <Link
                        to={link.route}
                        className="flex min-h-12 items-center gap-3 px-3 text-sm transition-colors outline-none hover:bg-muted/60 focus-visible:bg-muted/60 focus-visible:ring-3 focus-visible:ring-ring/50"
                      >
                        <Icon aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
                        <span className="flex-1">{link.label}</span>
                        <ChevronRight aria-hidden="true" className="size-4 shrink-0 text-muted-foreground" />
                      </Link>
                    </li>
                  )
                })}
              </ul>
            </section>
          ))}
        </div>
      )}
    </div>
  )
}
