import { useState } from 'react'
import { GripVertical } from 'lucide-react'

import { AddButton, DeleteButton, EditButton } from '@/components/layout/ActionButtons'
import { ListState } from '@/components/layout/ListState'
import { ListToolbar } from '@/components/layout/ListToolbar'
import { PageHeader } from '@/components/layout/PageHeader'
import { SearchForm } from '@/components/layout/SearchForm'
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
import { Card, CardContent } from '@/components/ui/card'
import { usePagePermissions } from '@/features/role-preview/usePagePermissions'
import { useAuth } from '@/features/auth/AuthContext'
import { useRolePreview } from '@/features/role-preview/RolePreviewContext'
import { ApiError } from '@/lib/api'
import { notify } from '@/lib/notify'
import { hasCriteria, textIncludes } from '@/lib/search'
import type { SearchValues } from '@/lib/search'

import type { PageFormValues } from './PageFormDialog'
import { PageFormDialog } from './PageFormDialog'
import { PageTree } from './PageTree'
import { buildTree, reorderLevel } from './pageNodes'
import type { PageNode } from './pageNodes'
import type { CreatePageInput, Page, UpdatePageInput } from './pagesApi'
import { pagesApi } from './pagesApi'
import { useAdminList } from '../useAdminList'

// ข้อความ error ที่รู้จักจาก backend (backend/src/users/service/pages.rs) แปลเป็นไทย
const PAGE_ERRORS: Record<string, string> = {
  'route already taken': 'route นี้มีเพจอื่นใช้อยู่แล้ว',
  'main page not found': 'ไม่พบเพจหลักที่เลือก (อาจถูกลบไปแล้ว) ลองรีเฟรชหน้า',
  'main page must be a top-level page without a route':
    'เพจหลักที่เลือกต้องเป็นหัวข้อกลุ่ม (เพจชั้นบนที่ไม่มี route)',
  'page cannot be its own main page': 'เลือกตัวเองเป็นเพจหลักไม่ได้',
  'sub page requires a route': 'เพจย่อยต้องมี route',
}

function pageErrorMessage(err: unknown, action: 'save' | 'delete'): string {
  if (err instanceof ApiError && err.message === 'page has sub pages') {
    return action === 'delete'
      ? 'ลบไม่ได้ เพจนี้ยังมีเพจย่อยอยู่ ย้ายเพจย่อยไปเพจหลักอื่นหรือลบออกก่อน'
      : 'เพจนี้มีเพจย่อยอยู่ ย้ายไปอยู่ใต้เพจหลักอื่นหรือใส่ route ไม่ได้ ย้ายเพจย่อยออกก่อน'
  }
  if (err instanceof ApiError && err.message in PAGE_ERRORS) {
    return PAGE_ERRORS[err.message]
  }
  return 'เกิดข้อผิดพลาด ลองใหม่อีกครั้ง'
}

const STALE_ORDER = 'page order out of date'

// จัดลำดับไม่สำเร็จ — 409 = รายการบนจอเก่ากว่าใน DB (มีคนแก้เพจจากที่อื่น)
function reorderErrorMessage(err: unknown): string {
  if (err instanceof ApiError && err.message === STALE_ORDER) {
    return 'รายการเพจถูกแก้จากที่อื่น โหลดรายการล่าสุดให้แล้ว ลองลากอีกครั้ง'
  }
  return 'บันทึกลำดับไม่สำเร็จ คืนลำดับเดิมให้แล้ว ลองอีกครั้ง'
}

// ค่าในฟอร์ม → body ของ POST/PATCH /pages: route ว่างไม่ส่ง (สร้าง = หัวข้อกลุ่ม, แก้ = ไม่แตะ) และ
// refId ว่างส่ง null (= เพจหลัก) ทั้งตอนสร้างและแก้ ย้ายเพจย่อยขึ้นเป็นเพจหลักได้ — ไม่ส่งลำดับ (จัดด้วยการลากวาง)
function toPageInput(values: PageFormValues): CreatePageInput & UpdatePageInput {
  return {
    label: values.label,
    icon: values.icon,
    route: values.route === '' ? undefined : values.route,
    permissions: values.permissions,
    refId: values.refId === '' ? null : values.refId,
  }
}

type FormState = { mode: 'create' } | { mode: 'edit'; page: Page } | null

// ต่อ API จริงเต็ม CRUD (backend/src/users/{handlers,service}.rs) — create/update คืนแค่ status
// ไม่มี body เลย refetch() ทั้ง list ของหน้านี้เองและ catalog ของ RoleSwitcher หลังทุก mutation
export function PagesSettingsPage() {
  const { canCreate, canUpdate, canDelete } = usePagePermissions('page')
  const { items: fetchedPages, status, refetch } = useAdminList(pagesApi.list)
  const { refetchCatalog } = useRolePreview()
  // หลังแก้ role/เพจ ต้องโหลดสิทธิ์ของผู้ใช้เองและเมนูซ้ายใหม่ด้วย (ไม่ใช่แค่ตารางในหน้านี้)
  const { refresh: refreshAuth } = useAuth()

  const [formState, setFormState] = useState<FormState>(null)
  const [pendingDelete, setPendingDelete] = useState<Page | null>(null)
  const [isDeleting, setIsDeleting] = useState(false)
  // ลำดับที่เพิ่งลากวาง — แสดงทันทีระหว่างบันทึก (optimistic) แล้วล้างเมื่อโหลดรายการใหม่เสร็จหรือบันทึกไม่สำเร็จ
  const [reordered, setReordered] = useState<Page[] | null>(null)
  const [isSavingOrder, setIsSavingOrder] = useState(false)
  const pages = reordered ?? fetchedPages
  // เงื่อนไขค้นหาที่ใช้กรองจริง (เปลี่ยนเมื่อกดปุ่ม "ค้นหา") — ช่องที่กรอกต้องตรงทุกช่อง
  const [criteria, setCriteria] = useState<SearchValues>({})
  const filtered = hasCriteria(criteria)
  const labelById = new Map(pages.map((page) => [page.id, page.label]))
  // ชื่อกลุ่มของแถว: เพจย่อย = ชื่อเพจหลักของมัน, เพจหลัก = ชื่อตัวเอง (ค้น "Lab" ได้ทั้งหัวข้อและเพจในกลุ่ม)
  const groupName = (page: Page) => (page.refId ? labelById.get(page.refId) : page.label)
  const matches = (page: Page) =>
    textIncludes(page.label, criteria.label ?? '') &&
    textIncludes(groupName(page), criteria.mainPage ?? '') &&
    textIncludes(page.route, criteria.route ?? '')
  const visiblePages = pages.filter(matches)

  // ต้นไม้ที่แสดง: ตอนค้นหา เพจหลักโผล่ถ้าตัวมันหรือเพจย่อยอย่างน้อยหนึ่งหน้าตรง และแสดงเฉพาะเพจย่อยที่ตรง
  const nodes: PageNode[] = buildTree(pages).flatMap((node) => {
    if (!filtered) return [node]
    const children = node.children.filter(matches)
    return matches(node.page) || children.length > 0 ? [{ ...node, children }] : []
  })
  // ลากได้เมื่อเห็นรายการครบ (ตอนค้นหาที่เห็นไม่ใช่ทั้งระดับ) และไม่มีการบันทึกลำดับค้างอยู่
  const reorderable = canUpdate && !filtered && !isSavingOrder

  // ตัวเลือก "เพจหลัก" ในฟอร์ม = เพจหลักที่เป็นหัวข้อกลุ่ม (เมนู 2 ชั้น)
  const groupPages = pages.filter((page) => page.refId === null && page.route === null)

  // ลากวางแล้วบันทึกทันที: แสดงลำดับใหม่ก่อน → PUT /pages/order → โหลดรายการ/เมนูซ้าย/สิทธิ์ใหม่เหมือน mutation อื่น
  // (โหลดแบบ silent ไม่ให้ list กระพริบเป็น loading) ไม่สำเร็จ = คืนลำดับเดิม, 409 = โหลดรายการล่าสุดให้
  // ข้อความ "ไม่สำเร็จ" ผูกกับ PUT เท่านั้น — โหลดซ้ำทีหลังพลาดไม่ได้แปลว่าลำดับไม่ถูกบันทึก
  const handleReorder = async (refId: string | null, ids: string[]) => {
    setReordered(reorderLevel(pages, refId, ids))
    setIsSavingOrder(true)
    try {
      await pagesApi.reorder(refId, ids)
    } catch (err) {
      setReordered(null)
      setIsSavingOrder(false)
      notify.error(reorderErrorMessage(err))
      if (err instanceof ApiError && err.message === STALE_ORDER) {
        await refetch({ silent: true })
      }
      return
    }
    notify.updated('บันทึกลำดับเพจแล้ว')
    await refetch({ silent: true })
    setReordered(null)
    setIsSavingOrder(false)
    refetchCatalog()
    await refreshAuth()
  }

  const handleFormSubmit = async (values: PageFormValues) => {
    try {
      if (formState?.mode === 'edit') {
        await pagesApi.update(formState.page.id, toPageInput(values))
      } else {
        await pagesApi.create(toPageInput(values))
      }
    } catch (err) {
      throw new Error(pageErrorMessage(err, 'save'))
    }
    await refetch()
    refetchCatalog()
    await refreshAuth()
    notify.saved(formState?.mode === 'edit', 'เพิ่มเพจแล้ว', 'แก้ไขเพจแล้ว')
  }

  const handleDelete = async () => {
    if (!pendingDelete) return
    setIsDeleting(true)
    try {
      await pagesApi.remove(pendingDelete.id)
      await refetch()
      refetchCatalog()
      await refreshAuth()
      notify.deleted('ลบเพจแล้ว')
      setPendingDelete(null)
    } catch (err) {
      notify.error(pageErrorMessage(err, 'delete'))
    } finally {
      setIsDeleting(false)
    }
  }

  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        fallbackTitle="การตั้งค่าเพจ"
        description="สร้าง แก้ไข ลบเพจ และกำหนดว่าใครเห็นเพจไหนในเมนู"
      />

      <SearchForm
        id="page-search"
        ariaLabel="ค้นหาเพจ"
        fields={[
          { name: 'label', label: 'ชื่อเพจ' },
          { name: 'mainPage', label: 'เพจหลัก' },
          { name: 'route', label: 'route', placeholder: 'เช่น /settings' },
        ]}
        onSearch={setCriteria}
      />

      <ListToolbar
        ready={status === 'ready'}
        total={pages.length}
        shown={visiblePages.length}
        filtered={hasCriteria(criteria)}
      >
        {canCreate && <AddButton onClick={() => setFormState({ mode: 'create' })}>เพิ่มเพจ</AddButton>}
      </ListToolbar>

      {status === 'ready' && canUpdate && pages.length > 0 && (
        <p className="text-sm text-muted-foreground" aria-live="polite">
          {isSavingOrder ? (
            'กำลังบันทึกลำดับ...'
          ) : filtered ? (
            'กด "ล้างข้อมูล" ในช่องค้นหาก่อน ถึงจะลากจัดลำดับได้'
          ) : (
            <>
              ลาก <GripVertical aria-hidden className="inline size-4 align-[-0.2em]" /> เพื่อจัดลำดับเมนู เพจย่อยเรียงได้ในกลุ่มของตัวเอง
              ย้ายไปกลุ่มอื่นที่ปุ่มแก้ไข
            </>
          )}
        </p>
      )}

      <Card>
        <CardContent className="px-2 sm:px-4">
          {status === 'loading' && (
            <ListState kind="loading" />
          )}
          {status === 'error' && (
            <ListState kind="error">โหลดข้อมูลไม่สำเร็จ ลองรีเฟรชหน้า</ListState>
          )}
          {status === 'ready' && pages.length > 0 && nodes.length === 0 && (
            <ListState kind="noMatch">
              ไม่พบเพจที่ตรงกับเงื่อนไข กด &quot;ล้างข้อมูล&quot; เพื่อดูทั้งหมด
            </ListState>
          )}
          {status === 'ready' && nodes.length > 0 && (
            <PageTree
              nodes={nodes}
              showHandles={canUpdate}
              reorderable={reorderable}
              onReorder={(refId, ids) => void handleReorder(refId, ids)}
              renderActions={(page) => (
                <>
                  {canUpdate && <EditButton onClick={() => setFormState({ mode: 'edit', page })} />}
                  {canDelete && <DeleteButton onClick={() => setPendingDelete(page)} />}
                </>
              )}
            />
          )}
        </CardContent>
      </Card>

      <PageFormDialog
        open={formState !== null}
        onOpenChange={(open) => {
          if (!open) setFormState(null)
        }}
        mode={formState?.mode ?? 'create'}
        initialValue={formState?.mode === 'edit' ? formState.page : undefined}
        onSubmit={handleFormSubmit}
        groupPages={groupPages}
      />

      <AlertDialog
        open={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null)
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>ยืนยันลบเพจ</AlertDialogTitle>
            <AlertDialogDescription>
              ต้องการลบเพจ &quot;{pendingDelete?.label}&quot; ใช่ไหม? การลบนี้ย้อนกลับไม่ได้
              ถ้าเป็นเพจที่อยู่ในเมนูอยู่ตอนนี้ จะหายจากเมนูของทุกคนทันที
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>ยกเลิก</AlertDialogCancel>
            <AlertDialogAction
              variant="danger"
              onClick={() => void handleDelete()}
              disabled={isDeleting}
            >
              {isDeleting ? 'กำลังลบ...' : 'ลบ'}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  )
}
