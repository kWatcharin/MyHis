// ตัวช่วยค้นหา/กรองรายการฝั่ง client ที่ใช้ร่วมกันทุกหน้ารายการ (ฟอร์มค้นหา: components/layout/SearchForm.tsx)
// ค่าของฟอร์มค้นหา = ชื่อช่อง → ข้อความที่พิมพ์ (ว่าง = ไม่กรองช่องนั้น; ช่องที่กรอกต้องตรงทุกช่อง)

export type SearchValues = Record<string, string>

// ตัดช่องว่างหัวท้าย, เป็นตัวพิมพ์เล็ก, ยุบช่องว่างซ้อน — ให้ "สมชาย  ใจดี" กับ "สมชาย ใจดี" เทียบกันได้
export function normalizeText(text: string): string {
  return text.trim().toLowerCase().replace(/\s+/g, ' ')
}

// value มีข้อความ query อยู่ในนั้นไหม (ไม่สนตัวพิมพ์เล็ก/ใหญ่) — query ว่าง = ผ่านเสมอ, value ว่าง/null
// ไม่ตรงกับ query ที่ไม่ว่าง
export function textIncludes(value: string | null | undefined, query: string): boolean {
  const q = normalizeText(query)
  if (!q) return true
  return normalizeText(value ?? '').includes(q)
}

// แยก query เป็นคำ (คั่นด้วยช่องว่าง) แล้วทุกคำต้องอยู่ใน value — ใช้กับชื่อ-สกุลที่พิมพ์สลับลำดับได้
// ("ใจดี สมชาย" ตรงกับ "สมชาย ใจดี")
export function allTokensIncluded(value: string, query: string): boolean {
  const q = normalizeText(query)
  if (!q) return true
  const haystack = normalizeText(value)
  return q.split(' ').every((token) => haystack.includes(token))
}

// มีเงื่อนไขค้นหาที่กรอกจริง (ไม่ใช่ว่างทั้งหมด) ไหม
export function hasCriteria(values: SearchValues): boolean {
  return Object.values(values).some((value) => value.trim() !== '')
}
