// ต้นไม้เมนู 2 ชั้นของหน้าตั้งค่าเพจ (PageTree.tsx) — ฟังก์ชันล้วน แยกจากไฟล์ component ให้ fast refresh ทำงาน
// (ชื่อไฟล์ต้องไม่ซ้ำ PageTree แบบต่างแค่ตัวพิมพ์ — Windows แยกไม่ออก)
import type { Page } from './pagesApi'

// เพจหลักหนึ่งหน้า + เพจย่อยที่แสดงตามลำดับเมนู (หัวข้อกลุ่มเท่านั้นที่มีเพจย่อย) — subPageCount = จำนวนเพจย่อยจริง
// (ตอนค้นหา children เหลือเฉพาะที่ตรง แต่จำนวนยังเป็นของทั้งกลุ่ม)
export interface PageNode {
  page: Page
  children: Page[]
  subPageCount: number
}

// list แบนจาก GET /pages (เรียงตามโครงเมนูแล้ว) → ต้นไม้ 2 ชั้น
export function buildTree(pages: Page[]): PageNode[] {
  const children = new Map<string, Page[]>()
  for (const page of pages) {
    if (page.refId) children.set(page.refId, [...(children.get(page.refId) ?? []), page])
  }
  return pages
    .filter((page) => page.refId === null)
    .map((page) => {
      const pageChildren = children.get(page.id) ?? []
      return { page, children: pageChildren, subPageCount: pageChildren.length }
    })
}

// เรียงเพจระดับหนึ่งใหม่ตาม ids (refId null = เพจหลัก) แล้วคืนเป็น list แบนลำดับเมนูเหมือนที่ backend ส่งมา
export function reorderLevel(pages: Page[], refId: string | null, ids: string[]): Page[] {
  const rank = new Map(ids.map((id, index) => [id, index]))
  const byRank = (a: Page, b: Page) => (rank.get(a.id) ?? 0) - (rank.get(b.id) ?? 0)
  return buildTree(pages)
    .sort((a, b) => (refId === null ? byRank(a.page, b.page) : 0))
    .flatMap(({ page, children }) => [
      page,
      ...(page.id === refId ? [...children].sort(byRank) : children),
    ])
}
