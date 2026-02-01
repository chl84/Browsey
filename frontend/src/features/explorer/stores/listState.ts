import { get, writable } from 'svelte/store'
import { clampIndex, clearSelection, selectAllPaths, selectRange } from '../selection'
import type { Entry } from '../types'
import { applyClickSelection } from '../helpers/selectionController'

const defaultRowHeight = 32
const overscan = 16
const wheelScale = 0.7

export type ListState = ReturnType<typeof createListState>

const isScrollbarClick = (event: MouseEvent, el: HTMLDivElement | null) => {
  if (!el) return false
  const rect = el.getBoundingClientRect()
  const scrollbarX = el.offsetWidth - el.clientWidth
  const scrollbarY = el.offsetHeight - el.clientHeight
  if (scrollbarX > 0) {
    const x = event.clientX - rect.left
    if (x >= el.clientWidth) return true
  }
  if (scrollbarY > 0) {
    const y = event.clientY - rect.top
    if (y >= el.clientHeight) return true
  }
  return false
}

export const createListState = (initialRowHeight: number = defaultRowHeight) => {
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

  const rowHeight = writable(initialRowHeight)

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
      const fallback = delta > 0 ? -1 : filteredEntries.length
      const currentIdx = get(caretIndex) ?? get(anchorIndex) ?? fallback
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
    const el = get(rowsEl)
    if (!el) return
    if (isScrollbarClick(event, el)) return
    if (event.target === el && get(selected).size > 0) {
      selected.set(clearSelection())
      anchorIndex.set(null)
      caretIndex.set(null)
    }
  }

  const handleRowClick = (filteredEntries: Entry[]) => (entry: Entry, absoluteIndex: number, event: MouseEvent) => {
    event.stopPropagation()
    const next = applyClickSelection(filteredEntries, absoluteIndex, event, {
      selected: get(selected),
      anchor: get(anchorIndex),
      caret: get(caretIndex),
    })
    selected.set(next.selected)
    anchorIndex.set(next.anchor)
    caretIndex.set(next.caret)
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
    const rh = get(rowHeight)
    const rowTop = index * rh
    const rowBottom = rowTop + rh
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
    const rh = get(rowHeight)
    const total = filteredEntries.length * rh
    totalHeight.set(total)
    const view = get(viewportHeight)
    const scrolled = get(scrollTop)
    const visibleCount = Math.ceil((view || 0) / rh) + overscan * 2
    const startIdx = Math.max(0, Math.floor(scrolled / rh) - overscan)
    const endIdx = Math.min(filteredEntries.length, startIdx + visibleCount)
    const slice = filteredEntries.slice(startIdx, endIdx)
    start.set(startIdx)
    offsetY.set(startIdx * rh)
    visibleEntries.set(slice)
  }

  const setRowHeight = (value: number) => {
    if (!Number.isFinite(value) || value <= 0) return
    rowHeight.set(value)
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
    setRowHeight,
  }
}
