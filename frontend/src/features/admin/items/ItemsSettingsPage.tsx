import { useCallback, useMemo, useState } from 'react'

import { AddButton, DeleteButton, EditButton } from '@/components/layout/ActionButtons'
import { ListState } from '@/components/layout/ListState'
import { ListToolbar } from '@/components/layout/ListToolbar'
import { PageHeader } from '@/components/layout/PageHeader'
import { SearchForm } from '@/components/layout/SearchForm'
import { TablePagination } from '@/components/layout/TablePagination'
import { usePagination } from '@/components/layout/usePagination'
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
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import { usePagePermissions } from '@/features/role-preview/usePagePermissions'
import { ApiError } from '@/lib/api'
import { notify } from '@/lib/notify'
import { STATUS_SEARCH_FIELD, hasCriteria, statusMatches, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'

import { ItemFormDialog } from './ItemFormDialog'
import type { Item, ItemInput, ItemOptions } from './itemsApi'
import { itemsApi } from './itemsApi'
import { useAdminList } from '../useAdminList'

// ข้อความ error ที่รู้จักจาก backend (backend/src/item/service.rs) แปลงเป็นไทย
const ITEM_ERRORS: Record<string, string> = {
  'item name already taken': 'ชื่อรายการนี้มีอยู่แล้ว',
  'expiry tracking requires lot tracking': 'ติดตามวันหมดอายุต้องติดตามล็อตด้วย',
  'item category is inactive': 'หมวดที่เลือกถูกปิดใช้งาน เลือกหมวดอื่น',
  'item unit is inactive': 'หน่วยที่เลือกถูกปิดใช้งาน เลือกหน่วยอื่น',
  'unknown item category': 'ไม่พบหมวดที่เลือก (อาจถูกลบไปแล้ว) ปิดแล้วเปิดฟอร์มใหม่',
  'unknown item unit': 'ไม่พบหน่วยที่เลือก (อาจถูกลบไปแล้ว) ปิดแล้วเปิดฟอร์มใหม่',
  'conversion unit must differ from base unit': 'หน่วยแปลงต้องไม่ใช่หน่วยฐาน',
  'duplicate conversion unit': 'หน่วยแปลงซ้ำกัน',
  'conversion factor must be greater than 1': 'จำนวนหน่วยฐานต่อหน่วยแปลงต้องมากกว่า 1',
  'too many conversions': 'หน่วยแปลงได้ไม่เกิน 10 หน่วย',
  'reorder point must not be negative': 'จุดสั่งซื้อต้องไม่ติดลบ',
  'name is required': 'กรอกชื่อรายการ',
  // กฎที่ขึ้นกับคลัง (ยอดเดิมนับเป็นหน่วยฐาน/ล็อตเดิม)
  'item has stock movements':
    'รายการนี้มีการเคลื่อนไหวในคลังแล้ว เปลี่ยนหน่วยฐานหรือการติดตามล็อต/วันหมดอายุไม่ได้',
  'item still has stock': 'รายการนี้ยังมีของคงเหลือในคลัง ลบไม่ได้',
}

function itemErrorMessage(err: unknown): string {
  if (err instanceof ApiError) {
    const known = ITEM_ERRORS[err.message]
    if (known) return known
    if (err.message.endsWith('must be at most 100 characters')) return 'ชื่อรายการยาวเกิน 100 ตัวอักษร'
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}

// "1 แผง = 10 เม็ด · 1 กล่อง = 100 เม็ด" — สูตรแปลงแสดงคู่กับรายการเสมอ ให้ตรวจด้วยตาได้
function conversionText(item: Item): string | null {
  if (item.conversions.length === 0) return null
  return item.conversions
    .map((c) => `1 ${c.unitName} = ${c.factor.toLocaleString('th-TH')} ${item.baseUnitName}`)
    .join(' · ')
}

type FormState = { mode: 'create' } | { mode: 'edit'; item: Item } | null

// หน้าตั้งค่ารายการคลัง (backend/src/item) — POST/PUT/DELETE คืนแค่ status ไม่มี body จึง refetch() ทั้ง list
// หลังทุก mutation; รายการเรียงตามหมวด (ลำดับของหน้าตั้งค่าหมวด) แล้วตามชื่อมาจาก backend แล้ว
export function ItemsSettingsPage() {
  const { canCreate, canUpdate, canDelete } = usePagePermissions('item')
  const { items, status, refetch } = useAdminList(itemsApi.list)
  const [options, setOptions] = useState<ItemOptions | null>(null)
  const [formState, setFormState] = useState<FormState>(null)
  const [pendingDelete, setPendingDelete] = useState<Item | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  // เงื่อนไขค้นหาที่ใช้กรองจริง (เปลี่ยนเมื่อกดปุ่ม "ค้นหา") — ช่องที่กรอกต้องตรงทุกช่อง
  const [criteria, setCriteria] = useState<SearchValues>({})

  // ตัวเลือกช่องค้นหา "หมวด" = หมวดที่มีรายการอยู่จริง (รวมหมวดที่ปิดใช้งานแล้วแต่ยังมีรายการ) ตามลำดับที่ backend
  // เรียงมา (ลำดับของหน้าตั้งค่าหมวด) — ค่าที่เลือกคือ uuid ของหมวด กรองแบบตรงตัว ไม่ใช่ข้อความบางส่วน
  const categoryOptions = useMemo(() => {
    const names = new Map<string, string>()
    for (const item of items) {
      if (!names.has(item.categoryId)) names.set(item.categoryId, item.categoryName)
    }
    return [...names].map(([value, label]) => ({ value, label }))
  }, [items])

  const visibleItems = items.filter(
    (item) =>
      textIncludes(item.name, criteria.name ?? '') &&
      (!criteria.category || item.categoryId === criteria.category) &&
      statusMatches(item.isActive, criteria.status),
  )
  const pagination = usePagination(visibleItems)
  // คอลัมน์ "ลำดับ" = เลขแถวตามที่เรียงอยู่ (หมวด → ชื่อ) นับต่อเนื่องข้ามหน้า — รายการคลังไม่มี sort_order ให้แก้เอง
  const rowOffset = (pagination.props.page - 1) * pagination.props.pageSize

  // ตัวเลือกของฟอร์ม โหลดใหม่ทุกครั้งที่เปิดฟอร์ม (เผื่อมีการแก้หมวด/หน่วยระหว่างที่หน้านี้เปิดค้าง)
  const loadOptions = useCallback(async () => {
    setOptions(null)
    try {
      setOptions(await itemsApi.options())
    } catch {
      notify.error('โหลดตัวเลือกหมวด/หน่วยนับไม่สำเร็จ ลองรีเฟรชหน้า')
    }
  }, [])

  const openForm = (state: Exclude<FormState, null>) => {
    void loadOptions()
    setFormState(state)
  }

  const handleFormSubmit = async (input: ItemInput) => {
    try {
      if (formState?.mode === 'edit') {
        await itemsApi.update(formState.item.id, input)
      } else {
        await itemsApi.create(input)
      }
    } catch (err) {
      throw new Error(itemErrorMessage(err))
    }
    await refetch()
    notify.saved(formState?.mode === 'edit', 'เพิ่มรายการคลังแล้ว', 'แก้ไขรายการคลังแล้ว')
  }

  const handleDelete = async () => {
    if (!pendingDelete) return
    setIsDeleting(true)
    try {
      await itemsApi.remove(pendingDelete.id)
      await refetch()
      notify.deleted('ลบรายการคลังแล้ว')
      setPendingDelete(null)
    } catch (err) {
      notify.error(itemErrorMessage(err))
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        fallbackTitle="ตั้งค่ารายการคลัง"
        description="รายการที่เก็บในคลัง (ยา เวชภัณฑ์ วัสดุแล็บ) พร้อมหน่วยฐานและหน่วยแปลง จำนวนทุกช่องนับเป็นหน่วยฐาน"
      />

      <SearchForm
        id="item-search"
        ariaLabel="ค้นหารายการคลัง"
        fields={[
          { name: 'name', label: 'ชื่อรายการ' },
          { name: 'category', label: 'หมวด', type: 'combobox', options: categoryOptions, allLabel: 'ทุกหมวด' },
          STATUS_SEARCH_FIELD,
        ]}
        onSearch={(values) => {
          setCriteria(values)
          pagination.reset()
        }}
      />

      <ListToolbar
        ready={status === 'ready'}
        total={items.length}
        shown={visibleItems.length}
        filtered={hasCriteria(criteria)}
      >
        {canCreate && <AddButton onClick={() => openForm({ mode: 'create' })}>เพิ่มรายการ</AddButton>}
      </ListToolbar>

      <Card>
        <CardContent className="flex flex-col gap-3 px-0 sm:px-2">
          {status === 'loading' && <ListState kind="loading" />}
          {status === 'error' && <ListState kind="error">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</ListState>}
          {status === 'ready' && items.length === 0 && (
            <ListState kind="empty">ยังไม่มีรายการคลัง กด &quot;+ เพิ่มรายการ&quot; เพื่อเริ่มต้น</ListState>
          )}
          {status === 'ready' && items.length > 0 && visibleItems.length === 0 && (
            <ListState kind="noMatch">
              ไม่พบรายการที่ตรงกับเงื่อนไข กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด
            </ListState>
          )}
          {status === 'ready' && visibleItems.length > 0 && (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>ลำดับ</TableHead>
                  <TableHead>ชื่อรายการ</TableHead>
                  <TableHead>หมวด</TableHead>
                  <TableHead>หน่วย</TableHead>
                  <TableHead>การติดตาม</TableHead>
                  <TableHead className="text-right">จุดสั่งซื้อ</TableHead>
                  <TableHead>สถานะ</TableHead>
                  {(canUpdate || canDelete) && <TableHead className="text-right">จัดการ</TableHead>}
                </TableRow>
              </TableHeader>
              <TableBody>
                {pagination.pagedItems.map((item, index) => {
                  const conversions = conversionText(item)
                  return (
                    <TableRow key={item.id}>
                      <TableCell className="text-muted-foreground">{rowOffset + index + 1}</TableCell>
                      <TableCell className="font-medium">{item.name}</TableCell>
                      <TableCell className="text-muted-foreground">{item.categoryName}</TableCell>
                      <TableCell className="text-muted-foreground">
                        <div>{item.baseUnitName}</div>
                        {conversions && <div className="text-xs">{conversions}</div>}
                      </TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-1">
                          {item.trackLot && <Badge variant="outline">ล็อต</Badge>}
                          {item.trackExpiry && <Badge variant="outline">วันหมดอายุ</Badge>}
                          {item.patientLinkable && <Badge variant="outline">ผูกผู้ป่วยได้</Badge>}
                          {!item.trackLot && !item.patientLinkable && (
                            <span className="text-muted-foreground">—</span>
                          )}
                        </div>
                      </TableCell>
                      <TableCell className="text-right text-muted-foreground">
                        {item.reorderPoint === null
                          ? '—'
                          : `${item.reorderPoint.toLocaleString('th-TH')} ${item.baseUnitName}`}
                      </TableCell>
                      <TableCell>
                        <Badge variant={item.isActive ? 'success' : 'outline'}>
                          {item.isActive ? 'ใช้งานอยู่' : 'ปิดใช้งาน'}
                        </Badge>
                      </TableCell>
                      {(canUpdate || canDelete) && (
                        <TableCell className="text-right whitespace-nowrap">
                          {canUpdate && <EditButton onClick={() => openForm({ mode: 'edit', item })} />}
                          {canDelete && <DeleteButton onClick={() => setPendingDelete(item)} />}
                        </TableCell>
                      )}
                    </TableRow>
                  )
                })}
              </TableBody>
            </Table>
          )}
          {status === 'ready' && visibleItems.length > 0 && <TablePagination {...pagination.props} />}
        </CardContent>
      </Card>

      <ItemFormDialog
        open={formState !== null}
        onOpenChange={(open) => {
          if (!open) setFormState(null)
        }}
        mode={formState?.mode ?? 'create'}
        initialValue={formState?.mode === 'edit' ? formState.item : undefined}
        options={options}
        onSubmit={handleFormSubmit}
      />

      <AlertDialog
        open={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>ยืนยันลบรายการคลัง</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการลบรายการ &quot;{pendingDelete?.name}&quot; ใช่ไหม? หน่วยแปลงของรายการนี้จะถูกลบไปด้วย
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>ยกเลิก</AlertDialogCancel>
            <AlertDialogAction variant="danger" onClick={() => void handleDelete()} disabled={isDeleting}>
              {isDeleting ? 'กำลังลบ...' : 'ลบ'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
