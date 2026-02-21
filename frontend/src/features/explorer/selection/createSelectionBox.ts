import { writable } from 'svelte/store'
import type { Entry } from '../model/types'
import { clampIndex, selectRange } from './selection'

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
const AUTOSCROLL_MIN_SPEED = 12 // px per frame (≈ 720 px/s)
const AUTOSCROLL_MAX_SPEED = 192 // px per frame (≈ 11.5 kpx/s)
const AUTOSCROLL_RAMP_FRAMES = 8 // ~130ms at 60fps

export const createSelectionBox = () => {
  const active = writable(false)
  const rect = writable<Rect>({ x: 0, y: 0, width: 0, height: 0 })

  let startContentX = 0
  let startContentY = 0
  let ctx: Context | null = null
  let didDrag = false
  let rafId: number | null = null
  let autoscrollSpeed = 0
  let autoscrollDir: -1 | 0 | 1 = 0
  let edgeFrames = 0
  let lastClientX = 0
  let lastClientY = 0
  let hasLast = false

  const stopAutoscroll = () => {
    if (rafId !== null) {
      cancelAnimationFrame(rafId)
      rafId = null
    }
    autoscrollSpeed = 0
    autoscrollDir = 0
    edgeFrames = 0
  }

  const scheduleAutoscroll = () => {
    if (rafId !== null) return
    rafId = requestAnimationFrame(() => {
      rafId = null
      if (!ctx || autoscrollSpeed === 0 || autoscrollDir === 0) return
      const el = ctx.rowsEl
      const maxScroll = Math.max(0, el.scrollHeight - el.clientHeight)
      const next = Math.max(0, Math.min(maxScroll, el.scrollTop + autoscrollSpeed * autoscrollDir))
      if (next === el.scrollTop) {
        stopAutoscroll()
        return
      }
      el.scrollTop = next
      if (hasLast) {
        updateSelection(lastClientX, lastClientY)
      }
      // Continue scrolling if mouse stays at edge
      scheduleAutoscroll()
    })
  }

  const cleanup = () => {
    window.removeEventListener('mousemove', handleMove)
    window.removeEventListener('mouseup', handleUp)
    stopAutoscroll()
  }

  const updateSelection = (clientX: number, clientY: number) => {
    if (!ctx) return
    const rowsRect = ctx.rowsEl.getBoundingClientRect()
    const headerHeight = ctx.headerEl?.offsetHeight ?? 0
    const scrollY = ctx.rowsEl.scrollTop
    const scrollX = ctx.rowsEl.scrollLeft
    const contentHeight = ctx.hitTest ? ctx.rowsEl.scrollHeight : ctx.entries.length * ctx.rowHeight
    const contentWidth = ctx.headerEl?.offsetWidth ?? ctx.rowsEl.scrollWidth
    const maxX = rowsRect.width
    const maxY = rowsRect.height

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
    autoscrollSpeed = 0
    autoscrollDir = 0
    const distanceTop = clientY - rowsRect.top
    const distanceBottom = rowsRect.bottom - clientY
    if (distanceTop < AUTOSCROLL_EDGE_PX) {
      autoscrollDir = -1
      const factor = (AUTOSCROLL_EDGE_PX - distanceTop) / AUTOSCROLL_EDGE_PX
      const eased = factor * factor
      const ramp = Math.min(1, edgeFrames / AUTOSCROLL_RAMP_FRAMES)
      autoscrollSpeed =
        AUTOSCROLL_MIN_SPEED +
        Math.min(
          AUTOSCROLL_MAX_SPEED - AUTOSCROLL_MIN_SPEED,
          eased * (AUTOSCROLL_MAX_SPEED - AUTOSCROLL_MIN_SPEED)
        ) *
          ramp
    } else if (distanceBottom < AUTOSCROLL_EDGE_PX) {
      autoscrollDir = 1
      const factor = (AUTOSCROLL_EDGE_PX - distanceBottom) / AUTOSCROLL_EDGE_PX
      const eased = factor * factor
      const ramp = Math.min(1, edgeFrames / AUTOSCROLL_RAMP_FRAMES)
      autoscrollSpeed =
        AUTOSCROLL_MIN_SPEED +
        Math.min(
          AUTOSCROLL_MAX_SPEED - AUTOSCROLL_MIN_SPEED,
          eased * (AUTOSCROLL_MAX_SPEED - AUTOSCROLL_MIN_SPEED)
        ) *
          ramp
    }
    if (autoscrollDir !== 0) {
      // hold counter for ramping up
      edgeFrames = Math.min(edgeFrames + 1, AUTOSCROLL_RAMP_FRAMES)
    } else {
      edgeFrames = 0
    }

    if (autoscrollSpeed !== 0 && autoscrollDir !== 0) {
      const el = ctx.rowsEl
      const maxScroll = Math.max(0, el.scrollHeight - el.clientHeight)
      if ((autoscrollDir < 0 && el.scrollTop <= 0) || (autoscrollDir > 0 && el.scrollTop >= maxScroll)) {
        autoscrollSpeed = 0
        autoscrollDir = 0
        edgeFrames = 0
      }
    }
    if (autoscrollSpeed !== 0 && autoscrollDir !== 0) {
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

  const handleMove = (event: MouseEvent) => {
    if (!ctx) return
    didDrag = true
    lastClientX = event.clientX
    lastClientY = event.clientY
    hasLast = true
    updateSelection(lastClientX, lastClientY)
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
