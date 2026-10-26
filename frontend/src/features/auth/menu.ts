import type { MenuItem } from './AuthContext'

// เมนู 2 ชั้นจาก pages.ref_id (docs/auth-design.md ข้อ 11): เพจหลัก (refId null) มี route = ลิงก์เดี่ยว เช่น
// Dashboard หรือไม่มี route = หัวข้อกลุ่มที่กดพับ/กางดูเพจย่อย, เพจย่อย (refId = เพจหลัก) มี route เสมอ

interface MenuPage {
  id: string
  refId: string | null
  route: string | null
  permissions: string[]
}

// กรองเพจตามชุดสิทธิ์ด้วยกฎเดียวกับ GET /menu (backend/src/users/service/pages.rs menu_for) — ใช้ตอน preview
// สิทธิ์ของ role อื่น: เพจผ่านเมื่อมี permission ตรงอย่างน้อยหนึ่งตัวหรือไม่ผูก permission เลย, เพจย่อยเห็นเมื่อ
// ทั้งตัวเองและเพจหลักผ่าน, หัวข้อกลุ่มเห็นเมื่อมีเพจย่อยที่เห็นได้ (ไม่โชว์หัวข้อเปล่า)
export function filterMenuPages<T extends MenuPage>(pages: T[], permissions: readonly string[]): T[] {
  const allowed = new Set(
    pages
      .filter(
        (page) =>
          page.permissions.length === 0 ||
          page.permissions.some((permission) => permissions.includes(permission)),
      )
      .map((page) => page.id),
  )
  const subPageVisible = (page: T) =>
    page.refId !== null && allowed.has(page.id) && allowed.has(page.refId)
  const groupsWithVisiblePages = new Set(pages.filter(subPageVisible).map((page) => page.refId))
  return pages.filter((page) =>
    page.refId !== null
      ? subPageVisible(page)
      : allowed.has(page.id) && (page.route !== null || groupsWithVisiblePages.has(page.id)),
  )
}

// route ของเมนูต้องเป็น path ภายในแอป: ขึ้นต้น "/" ตัวเดียว (ไม่ใช่ "//host" หรือ "/\host" ที่เบราว์เซอร์อ่านเป็นโดเมนอื่น) และไม่มี
// "\", ช่องว่าง, control — กฎเดียวกับ backend (users/dto.rs valid_menu_route) ใช้ทั้งตอนกรอกฟอร์มเพจและตอนสร้างเมนู เพราะ <Link>
// ของ react-router ถือ route ที่ไม่ใช่ path ภายในเป็นลิงก์ภายนอก แล้วเรนเดอร์เป็น <a href> ธรรมดา (เมนูของทุกคนที่เห็นเพจนั้นจะพาออกนอกโดเมน)
export function isInternalRoute(route: string): boolean {
  if (!route.startsWith('/') || route.startsWith('//')) return false
  for (const char of route) {
    const code = char.codePointAt(0) ?? 0
    if (char === '\\' || code <= 0x20 || code === 0x7f || /\s/.test(char)) return false
  }
  return true
}

// เมนูถือว่า active เมื่ออยู่หน้านั้นพอดีหรือหน้าย่อยของมัน (เช่น /lab/appointments/<id> ยังไฮไลต์ "นัดหมายแล็ป")
export function isRouteActive(pathname: string, route: string): boolean {
  return pathname === route || pathname.startsWith(`${route}/`)
}

// เพจในเมนูของ path ปัจจุบัน — route ที่ยาวที่สุดที่ active (หน้าย่อย /lab/appointments/<id> ได้ "นัดหมายแล็ป"
// และถ้าวันหน้ามีเพจ /lab ก็ไม่บัง /lab/appointments) ไม่พบ = undefined
export function findMenuPage(items: MenuItem[], pathname: string): MenuLink | undefined {
  return items
    .filter(isLink)
    .filter((item) => isRouteActive(pathname, item.route))
    .reduce<MenuLink | undefined>(
      (best, item) => (best && best.route.length >= item.route.length ? best : item),
      undefined,
    )
}

export type MenuLink = MenuItem & { route: string }

export interface MenuNode {
  item: MenuItem
  // เพจย่อยตามลำดับเดิม — ว่าง = ลิงก์เดี่ยวชั้นบน
  children: MenuLink[]
}

function isLink(item: MenuItem): item is MenuLink {
  return item.route !== null && isInternalRoute(item.route)
}

// จัดรายการเมนูแบบแบน (backend เรียงตามโครงเมนูมาแล้ว) เป็นต้นไม้ 2 ชั้น คงลำดับเดิมในแต่ละชั้น
// เพจย่อยที่หาเพจหลักไม่เจอหรือไม่มี route (ไม่ควรเกิด — backend กันไว้) ถูกตัดทิ้ง และรายการที่ route ไม่ใช่ path ภายในแอป
// (ข้อมูลเก่า/ถูกแก้ตรง) ไม่ถูกแสดงเป็นลิงก์เลย — ทั้ง sidebar จริงและโหมดดูสิทธิ์ของ role อื่นสร้างเมนูผ่านฟังก์ชันนี้
export function buildMenuTree(rawItems: MenuItem[]): MenuNode[] {
  const items = rawItems.filter((item) => item.route === null || isInternalRoute(item.route))
  const nodes: MenuNode[] = items
    .filter((item) => item.refId === null)
    .map((item) => ({ item, children: [] }))
  const byId = new Map(nodes.map((node) => [node.item.id, node]))
  for (const item of items) {
    if (item.refId !== null && isLink(item)) byId.get(item.refId)?.children.push(item)
  }
  return nodes
}
