import { Download, FileSpreadsheet, Printer } from 'lucide-react'

import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

import type { LabDocumentKind } from '../documents'
import { LAB_DOCUMENTS } from '../documents'

interface ExportMenuProps {
  disabled?: boolean
  onExcel: (kind: LabDocumentKind) => void
  onPdf: (kind: LabDocumentKind) => void
}

const KINDS: LabDocumentKind[] = ['cd4', 'roster']

// ปุ่ม "ส่งออกเอกสาร": เลือกเอกสาร (ใบนัด CD4/VL หรือใบรายชื่อนัดรับยา) แล้วเลือกรูปแบบ Excel หรือ PDF
// PDF = เปิดหน้าเอกสารพร้อมพิมพ์ในแท็บใหม่ แล้วเลือก "บันทึกเป็น PDF" ในหน้าต่างพิมพ์ของเบราว์เซอร์
export function ExportMenu({ disabled, onExcel, onPdf }: ExportMenuProps) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        disabled={disabled}
        render={
          <Button variant="outline">
            <Download />
            ส่งออกเอกสาร
          </Button>
        }
      />
      <DropdownMenuContent align="end" style={{ minWidth: '19rem' }}>
        {KINDS.map((kind, index) => (
          <DropdownMenuGroup key={kind}>
            {index > 0 && <DropdownMenuSeparator />}
            <DropdownMenuLabel>
              {LAB_DOCUMENTS[kind].title}
              <span className="block font-normal">{LAB_DOCUMENTS[kind].description}</span>
            </DropdownMenuLabel>
            <DropdownMenuItem onClick={() => onExcel(kind)}>
              <FileSpreadsheet />
              Excel (.xlsx)
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => onPdf(kind)}>
              <Printer />
              PDF (เปิดหน้าพิมพ์ → บันทึกเป็น PDF)
            </DropdownMenuItem>
          </DropdownMenuGroup>
        ))}
        <DropdownMenuSeparator />
        <p className="px-1.5 py-1 text-xs text-muted-foreground">
          เอกสารมี HN และชื่อคนไข้ เก็บไฟล์ให้ปลอดภัย
        </p>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
