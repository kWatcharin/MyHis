import { createElement } from 'react'
import type { ReactNode } from 'react'
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
} from '@dnd-kit/core'
import type { Announcements, DragEndEvent, UniqueIdentifier } from '@dnd-kit/core'
import { restrictToParentElement, restrictToVerticalAxis } from '@dnd-kit/modifiers'
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable'
import { CSS } from '@dnd-kit/utilities'
import { GripVertical } from 'lucide-react'

import { Badge } from '@/components/ui/badge'
import { resolveIcon } from '@/lib/icons'
import { cn } from '@/lib/utils'

import type { PageNode } from './pageNodes'
import type { Page } from './pagesApi'

const prefersReducedMotion = () =>
  typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches

const SCREEN_READER_INSTRUCTIONS = {
  draggable:
    'กด space หรือ enter เพื่อหยิบ ใช้ลูกศรขึ้นลงเพื่อย้าย กด space หรือ enter อีกครั้งเพื่อวาง หรือกด escape เพื่อยกเลิก',
}

// ข้อความที่ screen reader อ่านระหว่างลาก (ค่าเริ่มต้นของ dnd-kit เป็นภาษาอังกฤษ)
function thaiAnnouncements(items: Page[]): Announcements {
  const label = (id: UniqueIdentifier) => items.find((page) => page.id === id)?.label ?? ''
  const position = (id: UniqueIdentifier) =>
    `ลำดับที่ ${items.findIndex((page) => page.id === id) + 1} จาก ${items.length}`
  return {
    onDragStart: ({ active }) => `หยิบ ${label(active.id)} แล้ว อยู่${position(active.id)}`,
    onDragOver: ({ active, over }) =>
      over ? `${label(active.id)} อยู่${position(over.id)}` : `${label(active.id)} อยู่นอกรายการ`,
    onDragEnd: ({ active, over }) =>
      over ? `วาง ${label(active.id)} ที่${position(over.id)}` : `วาง ${label(active.id)} ที่เดิม`,
    onDragCancel: ({ active }) => `ยกเลิกการย้าย ${label(active.id)} กลับไปที่เดิม`,
  }
}

interface SortableLevelProps {
  items: Page[]
  refId: string | null
  reorderable: boolean
  onReorder: (refId: string | null, ids: string[]) => void
  className?: string
  children: ReactNode
}

// เพจหนึ่งระดับที่ลากเรียงกันได้ — แต่ละระดับมี DndContext ของตัวเอง (ซ้อนกันได้) การลากจึงไม่ข้ามระดับโดยโครงสร้าง:
// เพจหลักเรียงกันเอง เพจย่อยเรียงในกลุ่มของตัวเอง ย้ายกลุ่มต้องทำในฟอร์มแก้ไข (สิทธิ์ของเพจหลักคุมทั้งกลุ่ม)
function SortableLevel({ items, refId, reorderable, onReorder, className, children }: SortableLevelProps) {
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 4 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  )
  const ids = items.map((page) => page.id)

  const handleDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return
    const from = ids.indexOf(String(active.id))
    const to = ids.indexOf(String(over.id))
    if (from === -1 || to === -1) return
    onReorder(refId, arrayMove(ids, from, to))
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      modifiers={refId === null ? [restrictToVerticalAxis] : [restrictToVerticalAxis, restrictToParentElement]}
      onDragEnd={handleDragEnd}
      accessibility={{
        announcements: thaiAnnouncements(items),
        screenReaderInstructions: SCREEN_READER_INSTRUCTIONS,
      }}
    >
      <SortableContext items={ids} strategy={verticalListSortingStrategy} disabled={!reorderable}>
        <ul className={className}>{children}</ul>
      </SortableContext>
    </DndContext>
  )
}

interface PageRowProps {
  page: Page
  // แสดงปุ่มจับลาก (มีสิทธิ์แก้ไข) — กดได้จริงเมื่อ reorderable
  showHandle: boolean
  reorderable: boolean
  subPageCount?: number
  actions: ReactNode
  children?: ReactNode
}

function PageRow({ page, showHandle, reorderable, subPageCount, actions, children }: PageRowProps) {
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } =
    useSortable({
      id: page.id,
      attributes: { roleDescription: 'รายการที่ลากจัดลำดับได้' },
      transition: prefersReducedMotion() ? null : undefined,
    })
  const isGroup = page.refId === null && page.route === null

  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      className={cn('relative', isDragging && 'z-10')}
    >
      <div
        className={cn(
          'flex flex-wrap items-center gap-x-3 gap-y-2 rounded-lg px-2 py-2.5 transition-[background-color,box-shadow]',
          isDragging && 'bg-card shadow-[0_8px_24px_-6px_oklch(0_0_0/0.25)] ring-1 ring-border',
        )}
      >
        <div className="flex min-w-0 flex-1 basis-56 items-center gap-3">
          {showHandle && (
            <button
              ref={setActivatorNodeRef}
              type="button"
              {...attributes}
              {...listeners}
              disabled={!reorderable}
              aria-label={`ลากเพื่อจัดลำดับ ${page.label}`}
              className={cn(
                'flex size-8 shrink-0 touch-none items-center justify-center rounded-md text-muted-foreground outline-none',
                'focus-visible:ring-[3px] focus-visible:ring-ring/50',
                reorderable
                  ? 'cursor-grab hover:bg-muted hover:text-foreground active:cursor-grabbing'
                  : 'cursor-not-allowed opacity-40',
                isDragging && 'cursor-grabbing bg-muted text-foreground',
              )}
            >
              <GripVertical className="size-4" />
            </button>
          )}
          <span className="flex size-8 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
            {/* resolveIcon คืน component ตัวเดิมของ lucide เสมอ (ไม่ได้สร้างใหม่ทุก render) */}
            {createElement(resolveIcon(page.icon), { className: 'size-4' })}
          </span>
          <div className="flex min-w-0 flex-col gap-1">
            <span className="truncate font-medium">{page.label}</span>
            <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm text-muted-foreground">
              {isGroup ? (
                <>
                  <Badge variant="secondary">หัวข้อกลุ่ม</Badge>
                  <span>{subPageCount ? `${subPageCount} เพจย่อย` : 'ยังไม่มีเพจย่อย'}</span>
                </>
              ) : (
                <span className="font-mono">{page.route}</span>
              )}
              <span aria-hidden className="h-3.5 w-px bg-border" />
              {page.permissions.length === 0 ? (
                <span>สาธารณะ</span>
              ) : (
                page.permissions.map((permission) => (
                  <Badge key={permission} variant="outline">
                    {permission}
                  </Badge>
                ))
              )}
            </div>
          </div>
        </div>
        <div className="ml-auto flex shrink-0 gap-1">{actions}</div>
      </div>
      {children}
    </li>
  )
}

interface PageTreeProps {
  nodes: PageNode[]
  // ปุ่มจับลากมีเมื่อแก้ไขเพจได้ — ลากได้จริงเมื่อ reorderable (ไม่ได้ค้นหาอยู่ ไม่ได้กำลังบันทึก)
  showHandles: boolean
  reorderable: boolean
  onReorder: (refId: string | null, ids: string[]) => void
  renderActions: (page: Page) => ReactNode
}

// รายการเพจแบบต้นไม้: เพจหลักตามลำดับเมนู แต่ละหัวข้อกลุ่มมีเพจย่อยซ้อนอยู่ใต้ (ลากกลุ่มแล้วเพจย่อยติดไปด้วย)
export function PageTree({ nodes, showHandles, reorderable, onReorder, renderActions }: PageTreeProps) {
  return (
    <SortableLevel
      items={nodes.map((node) => node.page)}
      refId={null}
      reorderable={reorderable}
      onReorder={onReorder}
      className="flex flex-col divide-y divide-border"
    >
      {nodes.map(({ page, children, subPageCount }) => (
        <PageRow
          key={page.id}
          page={page}
          showHandle={showHandles}
          reorderable={reorderable}
          subPageCount={subPageCount}
          actions={renderActions(page)}
        >
          {children.length > 0 && (
            <SortableLevel
              items={children}
              refId={page.id}
              reorderable={reorderable}
              onReorder={onReorder}
              className="mb-2 ml-6 flex flex-col border-l border-border pl-2"
            >
              {children.map((child) => (
                <PageRow
                  key={child.id}
                  page={child}
                  showHandle={showHandles}
                  reorderable={reorderable}
                  actions={renderActions(child)}
                />
              ))}
            </SortableLevel>
          )}
        </PageRow>
      ))}
    </SortableLevel>
  )
}
