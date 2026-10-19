import {
  autocompletion,
  closeBrackets,
  closeBracketsKeymap,
  completionKeymap,
} from '@codemirror/autocomplete'
import type { Completion, CompletionContext, CompletionResult } from '@codemirror/autocomplete'
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
import { PostgreSQL, sql } from '@codemirror/lang-sql'
import { HighlightStyle, bracketMatching, syntaxHighlighting } from '@codemirror/language'
import { Compartment, EditorState, Prec } from '@codemirror/state'
import {
  EditorView,
  drawSelection,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  placeholder,
} from '@codemirror/view'
import { tags } from '@lezer/highlight'
import { useEffect, useImperativeHandle, useRef } from 'react'
import type { Ref } from 'react'

// ตัวแก้ไข SQL (CodeMirror 6) — auto complete ชื่อตาราง/คอลัมน์จาก /sql/schema (เฉพาะที่ role อ่านได้) + คำสงวนของ
// PostgreSQL + ตัวช่วย :hn('') / :nap_no('') สีทั้งหมดมาจาก token ของธีม (index.css) จึงตามทั้ง 3 ธีม
// เนื้อหาใน editor อยู่ในหน่วยความจำของหน้าเท่านั้น (ไม่จำลง localStorage)

export interface SqlEditorHandle {
  // ข้อความที่จะรัน: ส่วนที่เลือกไว้ (ถ้ามี) ไม่งั้นทั้งหมด + ตำแหน่งเริ่มของข้อความนั้น (ไว้ชี้ตำแหน่ง error)
  runnable: () => { text: string; offset: number }
  insert: (text: string) => void
  setText: (text: string) => void
  // ย้ายเคอร์เซอร์ไปตำแหน่ง (นับตัวอักษรจาก 0) และเลื่อนให้เห็น
  focusAt: (offset: number) => void
}

interface SqlEditorProps {
  ref?: Ref<SqlEditorHandle>
  initialValue: string
  // ตาราง → คอลัมน์ สำหรับ auto complete
  schema: Record<string, string[]>
  onRun: () => void
  ariaLabel: string
}

const theme = EditorView.theme({
  '&': {
    fontSize: '13px',
    backgroundColor: 'var(--card)',
    color: 'var(--card-foreground)',
  },
  '&.cm-focused': { outline: 'none' },
  // ใช้ font-mono ของกล่องที่ครอบ (index.css มีฟอนต์ไทยต่อท้าย) แทน monospace ตั้งต้นของ CodeMirror
  '.cm-scroller': { fontFamily: 'inherit', lineHeight: '1.65', minHeight: '12rem' },
  '.cm-content': { caretColor: 'var(--primary)', padding: '10px 0' },
  '.cm-gutters': {
    backgroundColor: 'color-mix(in oklch, var(--muted) 60%, transparent)',
    color: 'var(--muted-foreground)',
    border: 'none',
    borderRight: '1px solid var(--border)',
  },
  '.cm-activeLine': { backgroundColor: 'color-mix(in oklch, var(--primary) 6%, transparent)' },
  '.cm-activeLineGutter': {
    backgroundColor: 'color-mix(in oklch, var(--primary) 12%, transparent)',
    color: 'var(--foreground)',
  },
  '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground': {
    backgroundColor: 'color-mix(in oklch, var(--primary) 28%, transparent)',
  },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--primary)' },
  '.cm-placeholder': { color: 'var(--muted-foreground)' },
  '.cm-matchingBracket': {
    backgroundColor: 'color-mix(in oklch, var(--primary) 20%, transparent)',
    outline: 'none',
  },
  '.cm-tooltip': {
    backgroundColor: 'var(--popover)',
    color: 'var(--popover-foreground)',
    border: '1px solid var(--border)',
    borderRadius: '8px',
    overflow: 'hidden',
  },
  '.cm-tooltip-autocomplete > ul': { fontFamily: 'inherit', maxHeight: '16rem' },
  '.cm-tooltip-autocomplete > ul > li': { padding: '3px 10px' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    backgroundColor: 'var(--accent)',
    color: 'var(--accent-foreground)',
  },
  '.cm-completionDetail': { color: 'var(--muted-foreground)', fontStyle: 'normal', marginLeft: '0.75em' },
})

// ทุกสีผ่าน contrast ≥ 4.5:1 บน card ทั้ง 3 ธีม (token *-ink / muted-foreground ใน index.css)
const highlight = HighlightStyle.define([
  { tag: [tags.keyword, tags.operatorKeyword, tags.modifier], color: 'var(--info-ink)', fontWeight: '600' },
  { tag: [tags.string, tags.special(tags.string)], color: 'var(--success-ink)' },
  { tag: [tags.number, tags.bool, tags.null], color: 'var(--warning-ink)' },
  { tag: [tags.lineComment, tags.blockComment, tags.comment], color: 'var(--muted-foreground)', fontStyle: 'italic' },
  { tag: [tags.typeName, tags.standard(tags.name)], color: 'var(--danger-ink)' },
  { tag: [tags.operator, tags.punctuation, tags.paren, tags.squareBracket], color: 'var(--muted-foreground)' },
])

// ตัวช่วยค้นคอลัมน์ที่เข้ารหัส (backend แทนเป็น blind index ก่อนรัน) — เลือกแล้วเคอร์เซอร์อยู่ในเครื่องหมาย ' '
function helper(name: string, detail: string): Completion {
  return {
    label: name,
    detail,
    type: 'function',
    apply: (view, _completion, from, to) => {
      const text = `${name}('')`
      view.dispatch({
        changes: { from, to, insert: text },
        selection: { anchor: from + name.length + 2 },
      })
    },
  }
}

const HELPERS = [
  helper(':hn', "('HN') เทียบกับ hn_idx"),
  helper(':nap_no', "('เลขประจำตัว') เทียบกับ nap_no_idx"),
]

function helperCompletions(context: CompletionContext): CompletionResult | null {
  const word = context.matchBefore(/:[a-z_]*/)
  if (!word) return null
  // `::` คือ cast ไม่ใช่ตัวช่วย
  if (word.from > 0 && context.state.sliceDoc(word.from - 1, word.from) === ':') return null
  return { from: word.from, options: HELPERS, validFor: /^:[a-z_]*$/ }
}

function language(schema: Record<string, string[]>) {
  return [
    sql({ dialect: PostgreSQL, schema, upperCaseKeywords: false }),
    PostgreSQL.language.data.of({ autocomplete: helperCompletions }),
  ]
}

export function SqlEditor({ ref, initialValue, schema, onRun, ariaLabel }: SqlEditorProps) {
  const host = useRef<HTMLDivElement>(null)
  const view = useRef<EditorView | null>(null)
  const languageSlot = useRef(new Compartment())
  // ค่าตอนสร้าง editor ครั้งแรก + callback ล่าสุด (keymap ผูกครั้งเดียว อ่านจาก ref เสมอ)
  const initial = useRef({ value: initialValue, schema, ariaLabel })
  const runRef = useRef(onRun)

  useEffect(() => {
    runRef.current = onRun
  }, [onRun])

  useEffect(() => {
    if (!host.current) return
    const { value, schema: firstSchema, ariaLabel: label } = initial.current
    const editor = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          highlightActiveLineGutter(),
          highlightActiveLine(),
          drawSelection(),
          history(),
          bracketMatching(),
          closeBrackets(),
          autocompletion({ icons: false }),
          Prec.highest(
            keymap.of([
              {
                key: 'Mod-Enter',
                run: () => {
                  runRef.current()
                  return true
                },
              },
            ]),
          ),
          keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...historyKeymap, ...completionKeymap]),
          languageSlot.current.of(language(firstSchema)),
          syntaxHighlighting(highlight),
          theme,
          EditorView.lineWrapping,
          placeholder('select * from patient limit 10'),
          EditorView.contentAttributes.of({ 'aria-label': label }),
        ],
      }),
    })
    view.current = editor
    return () => {
      editor.destroy()
      view.current = null
    }
  }, [])

  // schema มาทีหลัง editor (โหลดจาก API) — เปลี่ยนเฉพาะส่วนภาษา ไม่สร้าง editor ใหม่ (ข้อความที่พิมพ์ไม่หาย)
  useEffect(() => {
    view.current?.dispatch({ effects: languageSlot.current.reconfigure(language(schema)) })
  }, [schema])

  useImperativeHandle(
    ref,
    () => ({
      runnable: () => {
        const state = view.current?.state
        if (!state) return { text: '', offset: 0 }
        const selection = state.selection.main
        return selection.empty
          ? { text: state.doc.toString(), offset: 0 }
          : { text: state.sliceDoc(selection.from, selection.to), offset: selection.from }
      },
      insert: (text) => {
        const editor = view.current
        if (!editor) return
        editor.dispatch(editor.state.replaceSelection(text))
        editor.focus()
      },
      setText: (text) => {
        const editor = view.current
        if (!editor) return
        editor.dispatch({
          changes: { from: 0, to: editor.state.doc.length, insert: text },
          selection: { anchor: text.length },
        })
        editor.focus()
      },
      focusAt: (offset) => {
        const editor = view.current
        if (!editor) return
        const anchor = Math.max(0, Math.min(offset, editor.state.doc.length))
        editor.dispatch({ selection: { anchor }, scrollIntoView: true })
        editor.focus()
      },
    }),
    [],
  )

  return <div ref={host} className="font-mono" />
}
