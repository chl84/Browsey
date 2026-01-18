import { writable } from 'svelte/store'
import type { Entry } from '../types'
import { clampIndex, selectRange } from '../selection'

type Rect = { x: number; y: number; width: number; height: number }

type Context = {
  rowsEl: HTMLDivElement
  headerEl: HTMLDivElement | null
  entries: Entry[]
  rowHeight: number
  hitTest?: (rect: Rect) => { paths: Set<string>; anchor: number | null; caret: number | null }
  onSelect: (paths: Set<string>, anchor: number | null, caret: number | null) => void
  onEnd?: (didDrag: boolean) => void
}

const AUTOSCROLL_EDGE_PX = 48
const AUTOSCROLL_STEP = 32

export const createSelectionBox = () => {
  const active = writable(false)
  const rect = writable<Rect>({ x: 0, y: 0, width: 0, height: 0 })

  let startContentX = 0
  let startContentY = 0
  let ctx: Context | null = null
  let didDrag = false
  let rafId: number | null = null
  let pendingScroll = 0

  const stopAutoscroll = () => {
    if (rafId !== null) {
      cancelAnimationFrame(rafId)
      rafId = null
    }
    pendingScroll = 0
  }

  const scheduleAutoscroll = () => {
    if (rafId !== null) return
    rafId = requestAnimationFrame(() => {
      rafId = null
      if (!ctx || pendingScroll === 0) return
      const el = ctx.rowsEl
      const maxScroll = Math.max(0, el.scrollHeight - el.clientHeight)
      const next = Math.max(0, Math.min(maxScroll, el.scrollTop + pendingScroll))
      if (next === el.scrollTop) {
        pendingScroll = 0
        stopAutoscroll()
        return
      }
      el.scrollTop = next
      pendingScroll = 0
      // Continue scrolling if mouse stays at edge
      scheduleAutoscroll()
    })
  }

  const cleanup = () => {
    window.removeEventListener('mousemove', handleMove)
    window.removeEventListener('mouseup', handleUp)
    stopAutoscroll()
  }

  const handleMove = (event: MouseEvent) => {
    if (!ctx) return
    didDrag = true
    const rowsRect = ctx.rowsEl.getBoundingClientRect()
    const headerHeight = ctx.headerEl?.offsetHeight ?? 0
    const scrollY = ctx.rowsEl.scrollTop
    const scrollX = ctx.rowsEl.scrollLeft
    const contentHeight = ctx.hitTest ? ctx.rowsEl.scrollHeight : ctx.entries.length * ctx.rowHeight
    const contentWidth = ctx.headerEl?.offsetWidth ?? ctx.rowsEl.scrollWidth
    const maxX = rowsRect.width
    const maxY = rowsRect.height

    const clientX = event.clientX
    const clientY = event.clientY
    const currentContentX = clientX - rowsRect.left + scrollX
    const currentContentY = clientY - rowsRect.top + scrollY
    const rawX1 = Math.min(startContentX, currentContentX)
    const rawY1 = Math.min(startContentY, currentContentY)
    const rawX2 = Math.max(startContentX, currentContentX)
    const rawY2 = Math.max(startContentY, currentContentY)

    const maxContentX = Math.max(contentWidth, maxX + scrollX)
    const maxContentY = Math.max(contentHeight + headerHeight, maxY + scrollY)
    const x1 = Math.max(0, Math.min(rawX1, maxContentX))
    const x2 = Math.max(0, Math.min(rawX2, maxContentX))
    const y1 = Math.max(0, Math.min(rawY1, maxContentY))
    const y2 = Math.max(0, Math.min(rawY2, maxContentY))

    // Autoscroll near edges while dragging
    pendingScroll = 0
    const distanceTop = clientY - rowsRect.top
    const distanceBottom = rowsRect.bottom - clientY
    if (distanceTop < AUTOSCROLL_EDGE_PX) {
      pendingScroll = -AUTOSCROLL_STEP
    } else if (distanceBottom < AUTOSCROLL_EDGE_PX) {
      pendingScroll = AUTOSCROLL_STEP
    }
    if (pendingScroll !== 0) {
      const el = ctx.rowsEl
      const maxScroll = Math.max(0, el.scrollHeight - el.clientHeight)
      if ((pendingScroll < 0 && el.scrollTop <= 0) || (pendingScroll > 0 && el.scrollTop >= maxScroll)) {
        pendingScroll = 0
      }
    }
    if (pendingScroll !== 0) {
      scheduleAutoscroll()
    } else {
      stopAutoscroll()
    }

    const boxY1 = Math.max(scrollY + headerHeight, y1)
    const boxY2 = Math.max(boxY1, y2)
    const hitY1 = Math.max(headerHeight, y1)
    const hitY2 = Math.max(hitY1, y2)

    rect.set({
      x: x1,
      y: boxY1,
      width: Math.max(0, x2 - x1),
      height: Math.max(0, boxY2 - boxY1),
    })

    if (ctx.hitTest) {
      const result = ctx.hitTest({
        x: x1,
        y: hitY1,
        width: Math.max(0, x2 - x1),
        height: Math.max(0, hitY2 - hitY1),
      })
      ctx.onSelect(result.paths, result.anchor, result.caret)
      return
    }

    const intersectsX = Math.min(x1, x2) < contentWidth
    const yStartContent = y1 - headerHeight
    const yEndContent = y2 - headerHeight
    if (!intersectsX || yStartContent > contentHeight || yEndContent < 0) {
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
    stopAutoscroll()
  }

  const start = (event: MouseEvent, context: Context) => {
    if (event.button !== 0) return
    const rowsRect = context.rowsEl.getBoundingClientRect()
    const contentWidth = context.headerEl?.offsetWidth ?? context.rowsEl.scrollWidth
    const scrollX = context.rowsEl.scrollLeft
    const scrollY = context.rowsEl.scrollTop
    const x = event.clientX - rowsRect.left

    // If click is completely past the content area, clear selection but allow drag to start
    if (x > contentWidth) {
      context.onSelect(new Set(), null, null)
    }

    ctx = context
    didDrag = false
    startContentX = x + scrollX
    startContentY = event.clientY - rowsRect.top + scrollY
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
