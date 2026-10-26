import { Search } from 'lucide-react'
import { useState } from 'react'
import type { FormEvent } from 'react'

import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Field, FieldLabel } from '@/components/ui/field'
import { Input } from '@/components/ui/input'
import { ThaiDateInput } from '@/components/ui/thai-date-input'
import { NativeSelect } from '@/components/ui/native-select'
import type { SearchValues } from '@/lib/search'
import { cn } from '@/lib/utils'

export interface SearchFieldDef {
  name: string
  label: string
  placeholder?: string
  // ชนิดช่อง: ข้อความ (ค่าเริ่มต้น), วันที่ (แสดง พ.ศ. ค่า "YYYY-MM-DD"), ตัวเลือก (ค่าว่าง = ทั้งหมด ข้อความตาม allLabel)
  type?: 'text' | 'date' | 'select'
  options?: { value: string; label: string }[]
  allLabel?: string
}

interface SearchFormProps {
  // ใช้ตั้ง id ของช่อง (ต้องไม่ซ้ำกันในหน้า) และ aria-label ของฟอร์ม
  id: string
  ariaLabel: string
  fields: SearchFieldDef[]
  // เรียกเมื่อกดปุ่ม "ค้นหา"/Enter (ส่งค่าที่พิมพ์) และเมื่อกด "ล้างข้อมูล" (ส่งค่าว่างทุกช่อง)
  onSearch: (values: SearchValues) => void
}

const GRID_COLUMNS: Record<number, string> = {
  1: 'sm:max-w-sm',
  2: 'sm:grid-cols-2',
  3: 'sm:grid-cols-3',
}

// ฟอร์มค้นหาของหน้ารายการ: ช่องกรอกตาม fields (ช่องที่กรอกต้องตรงทุกช่อง) + ปุ่ม "ค้นหา" (ไอคอนแว่นขยาย)
// อยู่กลางแถว + ปุ่ม "ล้างข้อมูล" — ค่าที่กำลังพิมพ์เก็บในฟอร์มนี้เอง ผลกรองเปลี่ยนเมื่อกดค้นหา/Enter เท่านั้น
// ผู้ใช้ต้องกรองรายการเองจากค่าที่ได้ผ่าน onSearch (ดู lib/search.ts)
export function SearchForm({ id, ariaLabel, fields, onSearch }: SearchFormProps) {
  const empty = (): SearchValues => Object.fromEntries(fields.map((f) => [f.name, '']))
  const [draft, setDraft] = useState<SearchValues>(empty)

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault()
    onSearch(draft)
  }

  const handleClear = () => {
    const cleared = empty()
    setDraft(cleared)
    onSearch(cleared)
  }

  return (
    <Card>
      <CardContent>
        <form onSubmit={handleSubmit} aria-label={ariaLabel} className="flex flex-col gap-4">
          <div
            className={cn(
              'grid grid-cols-1 gap-4',
              GRID_COLUMNS[Math.min(fields.length, 3)],
            )}
          >
            {fields.map((field) => {
              const fieldId = `${id}-${field.name}`
              const value = draft[field.name] ?? ''
              const set = (next: string) => setDraft({ ...draft, [field.name]: next })
              return (
                <Field key={field.name}>
                  <FieldLabel htmlFor={fieldId}>{field.label}</FieldLabel>
                  {field.type === 'select' ? (
                    <NativeSelect id={fieldId} value={value} onChange={(e) => set(e.target.value)}>
                      <option value="">{field.allLabel ?? 'ทั้งหมด'}</option>
                      {field.options?.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label}
                        </option>
                      ))}
                    </NativeSelect>
                  ) : field.type === 'date' ? (
                    <ThaiDateInput id={fieldId} value={value} onChange={set} />
                  ) : (
                    <Input
                      id={fieldId}
                      value={value}
                      onChange={(e) => set(e.target.value)}
                      placeholder={field.placeholder}
                    />
                  )}
                </Field>
              )
            })}
          </div>
          <div className="flex justify-center gap-2">
            <Button type="submit">
              <Search />
              ค้นหา
            </Button>
            <Button type="button" variant="outline" onClick={handleClear}>
              ล้างข้อมูล
            </Button>
          </div>
        </form>
      </CardContent>
    </Card>
  )
}
