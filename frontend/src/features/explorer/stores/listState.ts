import { get, writable } from 'svelte/store'
import { clampIndex, clearSelection, selectRange } from '../selection'
import type { Entry } from '../types'
import { applyClickSelection } from '../helpers/selectionController'
import { isScrollbarClick } from '../helpers/scrollbar'

const defaultRowHeight = 32
const overscan = 16

export type ListState = ReturnType<typeof createListState>

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

  let scrollRaf: number | null = null
  let pendingScrollTop = 0

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
    pendingScrollTop = Math.max(0, el.scrollTop - headerHeight())
    if (scrollRaf !== null) return
    scrollRaf = requestAnimationFrame(() => {
      scrollRaf = null
      scrollTop.set(pendingScrollTop)
    })
  }

  const handleWheel = (_event: WheelEvent) => {
    // Bruk nettleserens native scroll og momentum; ingen custom handling.
  }

  const handleRowsKeydown = (filteredEntries: Entry[]) => (event: KeyboardEvent) => {
    const key = event.key.toLowerCase()
    const selectedSet = get(selected)
    if (key === 'escape') {
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
    const visibleCount = Math.max(1, Math.ceil((view || 0) / rh) + overscan * 2)
    const rawStartIdx = Math.max(0, Math.floor(scrolled / rh) - overscan)
    const maxStartIdx = Math.max(0, filteredEntries.length - visibleCount)
    const startIdx = Math.min(rawStartIdx, maxStartIdx)
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
