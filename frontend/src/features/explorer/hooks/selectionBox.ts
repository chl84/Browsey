import { writable } from 'svelte/store'
import type { Entry } from '../types'
import { clampIndex, selectRange } from '../selection'

type Rect = { x: number; y: number; width: number; height: number }

type Context = {
  rowsEl: HTMLDivElement
  headerEl: HTMLDivElement | null
  entries: Entry[]
  rowHeight: number
  onSelect: (paths: Set<string>, anchor: number | null, caret: number | null) => void
  onEnd?: (didDrag: boolean) => void
}

export const createSelectionBox = () => {
  const active = writable(false)
  const rect = writable<Rect>({ x: 0, y: 0, width: 0, height: 0 })

  let startX = 0
  let startY = 0
  let ctx: Context | null = null
  let didDrag = false

  const cleanup = () => {
    window.removeEventListener('mousemove', handleMove)
    window.removeEventListener('mouseup', handleUp)
  }

  const handleMove = (event: MouseEvent) => {
    if (!ctx) return
    didDrag = true
    const rowsRect = ctx.rowsEl.getBoundingClientRect()
    const headerHeight = ctx.headerEl?.offsetHeight ?? 0
    const scrollY = ctx.rowsEl.scrollTop
    const contentHeight = ctx.entries.length * ctx.rowHeight
    const maxX = rowsRect.width
    const maxY = rowsRect.height

    const rawX1 = Math.min(startX, event.clientX) - rowsRect.left
    const rawY1 = Math.min(startY, event.clientY) - rowsRect.top
    const rawX2 = Math.max(startX, event.clientX) - rowsRect.left
    const rawY2 = Math.max(startY, event.clientY) - rowsRect.top

    const x1 = Math.max(0, Math.min(rawX1, maxX))
    const x2 = Math.max(0, Math.min(rawX2, maxX))
    const y1Local = Math.max(0, Math.min(rawY1, maxY))
    const y2Local = Math.max(0, Math.min(rawY2, maxY))
    const y1 = y1Local + scrollY
    const y2 = y2Local + scrollY

    rect.set({
      x: x1,
      y: y1,
      width: Math.max(0, x2 - x1),
      height: Math.max(0, y2 - y1),
    })

    const yStartContent = y1 - headerHeight
    const yEndContent = y2 - headerHeight
    if (yStartContent > contentHeight || yEndContent < 0) {
      ctx.onSelect(new Set(), null, null)
      return
    }

    const lo = clampIndex(Math.floor(Math.max(0, yStartContent) / ctx.rowHeight), ctx.entries)
    const hi = clampIndex(Math.floor(Math.max(0, yEndContent) / ctx.rowHeight), ctx.entries)

    const rangeSet = selectRange(ctx.entries, lo, hi)
    ctx.onSelect(rangeSet, lo, hi)
  }

  const handleUp = () => {
    active.set(false)
    cleanup()
    ctx?.onEnd?.(didDrag)
    ctx = null
  }

  const start = (event: MouseEvent, context: Context) => {
    if (event.button !== 0) return
    ctx = context
    didDrag = false
    startX = event.clientX
    startY = event.clientY
    rect.set({ x: 0, y: 0, width: 0, height: 0 })
    active.set(true)
    window.addEventListener('mousemove', handleMove)
    window.addEventListener('mouseup', handleUp, { once: true })
  }

  return {
    active,
    rect,
    start,
  }
}
