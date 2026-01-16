import { get, writable } from 'svelte/store'
import { clampIndex, clearSelection, selectAllPaths, selectRange } from '../selection'
import type { Entry } from '../types'

export const rowHeight = 32
const overscan = 8
const wheelScale = 0.7

export type ListState = ReturnType<typeof createListState>

export const createListState = () => {
  const selected = writable<Set<string>>(clearSelection())
  const anchorIndex = writable<number | null>(null)
  const caretIndex = writable<number | null>(null)
  const viewportHeight = writable(0)
  const scrollTop = writable(0)
  const rowsEl = writable<HTMLDivElement | null>(null)
  const headerEl = writable<HTMLDivElement | null>(null)
  const totalHeight = writable(0)
  const visibleEntries = writable<Entry[]>([])
  const start = writable(0)
  const offsetY = writable(0)

  let wheelRaf: number | null = null
  let pendingDeltaY = 0

  const headerHeight = () => get(headerEl)?.offsetHeight ?? 0

  const updateViewportHeight = () => {
    const containerHeight = get(rowsEl)?.clientHeight ?? 0
    const next = Math.max(0, containerHeight - headerHeight())
    if (next !== get(viewportHeight)) {
      viewportHeight.set(next)
    }
  }

  const handleResize = () => {
    if (typeof window === 'undefined') return
    updateViewportHeight()
  }

  const focusRow = (index: number) => {
    const el = get(rowsEl)
    if (!el) return
    const row = el.querySelector<HTMLButtonElement>(`.row[data-index="${index}"]`)
    row?.focus()
  }

  const handleRowsScroll = () => {
    const el = get(rowsEl)
    if (!el) return
    const effectiveTop = Math.max(0, el.scrollTop - headerHeight())
    scrollTop.set(effectiveTop)
  }

  let pendingDeltaX = 0
  const handleWheel = (event: WheelEvent) => {
    const el = get(rowsEl)
    if (!el) return
    pendingDeltaX += event.deltaX * wheelScale
    pendingDeltaY += event.deltaY * wheelScale
    if (wheelRaf !== null) return
    wheelRaf = requestAnimationFrame(() => {
      el.scrollLeft += pendingDeltaX
      el.scrollTop += pendingDeltaY
      pendingDeltaX = 0
      pendingDeltaY = 0
      wheelRaf = null
    })
  }

  const handleRowsKeydown = (filteredEntries: Entry[]) => (event: KeyboardEvent) => {
    const key = event.key.toLowerCase()
    const selectedSet = get(selected)
    if (event.ctrlKey && key === 'a') {
      event.preventDefault()
      event.stopPropagation()
      selected.set(selectAllPaths(filteredEntries))
      anchorIndex.set(0)
      caretIndex.set(filteredEntries.length > 0 ? filteredEntries.length - 1 : null)
    } else if (key === 'escape') {
      event.preventDefault()
      event.stopPropagation()
      selected.set(clearSelection())
      anchorIndex.set(null)
      caretIndex.set(null)
    } else if ((key === 'arrowdown' || key === 'arrowup') && filteredEntries.length > 0) {
      event.preventDefault()
      event.stopPropagation()
      const delta = key === 'arrowdown' ? 1 : -1
      const currentIdx = get(caretIndex) ?? get(anchorIndex) ?? (delta > 0 ? 0 : filteredEntries.length - 1)
      const next = clampIndex(currentIdx + delta, filteredEntries)

      if (event.shiftKey) {
        const anchor = get(anchorIndex) ?? currentIdx
        const rangeSet = selectRange(filteredEntries, anchor, next)
        selected.set(rangeSet)
        anchorIndex.set(anchor)
        caretIndex.set(next)
        focusRow(next)
      } else {
        selected.set(new Set([filteredEntries[next].path]))
        anchorIndex.set(next)
        caretIndex.set(next)
        focusRow(next)
      }
      if (get(caretIndex) !== null) {
        ensureRowVisible(get(caretIndex)!)
      }
    }
  }

  const handleRowsClick = (event: MouseEvent) => {
    if (event.target === get(rowsEl) && get(selected).size > 0) {
      selected.set(clearSelection())
      anchorIndex.set(null)
      caretIndex.set(null)
    }
  }

  const handleRowClick = (filteredEntries: Entry[]) => (entry: Entry, absoluteIndex: number, event: MouseEvent) => {
    event.stopPropagation()
    const isToggle = event.ctrlKey || event.metaKey
    const isRange = event.shiftKey && get(anchorIndex) !== null

    if (isRange && get(anchorIndex) !== null) {
      const rangeSet = selectRange(filteredEntries, get(anchorIndex)!, absoluteIndex)
      if (isToggle) {
        const merged = new Set(get(selected))
        rangeSet.forEach((p) => merged.add(p))
        selected.set(merged)
      } else {
        selected.set(rangeSet)
      }
      caretIndex.set(absoluteIndex)
    } else if (isToggle) {
      const next = new Set(get(selected))
      if (next.has(entry.path)) {
        next.delete(entry.path)
      } else {
        next.add(entry.path)
      }
      selected.set(next)
      anchorIndex.set(absoluteIndex)
      caretIndex.set(absoluteIndex)
    } else {
      selected.set(new Set([entry.path]))
      anchorIndex.set(absoluteIndex)
      caretIndex.set(absoluteIndex)
    }
  }

  const resetScrollPosition = () => {
    scrollTop.set(0)
    get(rowsEl)?.scrollTo({ top: 0 })
  }

  const ensureRowVisible = (index: number) => {
    const el = get(rowsEl)
    if (!el) return
    const headerOffset = headerHeight()
    const viewport = get(viewportHeight)
    const currentTop = get(scrollTop)
    const currentBottom = currentTop + viewport
    const rowTop = index * rowHeight
    const rowBottom = rowTop + rowHeight
    let nextScroll: number | null = null

    if (rowTop < currentTop) {
      nextScroll = headerOffset + rowTop
    } else if (rowBottom > currentBottom) {
      nextScroll = headerOffset + rowBottom - viewport
    }

    if (nextScroll !== null) {
      el.scrollTo({ top: nextScroll })
    }
  }

  const recompute = (filteredEntries: Entry[]) => {
    const total = filteredEntries.length * rowHeight
    totalHeight.set(total)
    const view = get(viewportHeight)
    const scrolled = get(scrollTop)
    const visibleCount = Math.ceil((view || 0) / rowHeight) + overscan * 2
    const startIdx = Math.max(0, Math.floor(scrolled / rowHeight) - overscan)
    const endIdx = Math.min(filteredEntries.length, startIdx + visibleCount)
    const slice = filteredEntries.slice(startIdx, endIdx)
    start.set(startIdx)
    offsetY.set(startIdx * rowHeight)
    visibleEntries.set(slice)
  }

  return {
    rowHeight,
    overscan,
    wheelScale,
    selected,
    anchorIndex,
    caretIndex,
    viewportHeight,
    scrollTop,
    rowsEl,
    headerEl,
    totalHeight,
    visibleEntries,
    start,
    offsetY,
    updateViewportHeight,
    handleResize,
    handleRowsScroll,
    handleWheel,
    handleRowsKeydown,
    handleRowsClick,
    handleRowClick,
    resetScrollPosition,
    recompute,
  }
}
