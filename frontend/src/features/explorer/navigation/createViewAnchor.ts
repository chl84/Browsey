import { get } from 'svelte/store'
import type { Readable } from 'svelte/store'
import type { Entry } from '../model/types'

type ViewMode = 'list' | 'grid'

type Refs = {
  rowsEl: HTMLDivElement | null
  headerEl: HTMLDivElement | null
  gridEl: HTMLDivElement | null
  gridCols: number
}

type Options = {
  filteredEntries: Readable<Entry[]>
  rowHeight: number
  gridRowHeight: number
  gridGap: number
}

export const createViewSwitchAnchor = ({ filteredEntries, rowHeight, gridRowHeight, gridGap }: Options) => {
  let anchorPath: string | null = null
  let anchorIndex = -1

  const capture = ({ viewMode, rowsEl, gridEl, gridCols }: Refs & { viewMode: ViewMode }) => {
    const list = get(filteredEntries)
    if (list.length === 0) {
      anchorPath = null
      anchorIndex = -1
      return
    }

    if (viewMode === 'list') {
      const viewport = Math.max(0, rowsEl?.clientHeight ?? 0)
      const midOffset = Math.max(0, (rowsEl?.scrollTop ?? 0) + viewport / 2)
      const idx = Math.min(list.length - 1, Math.floor(midOffset / rowHeight))
      anchorIndex = idx
      anchorPath = list[idx]?.path ?? null
      return
    }

    if (!gridEl) {
      anchorPath = null
      anchorIndex = -1
      return
    }

    const rowStride = gridRowHeight + gridGap
    const viewport = gridEl.clientHeight
    const midOffset = (gridEl.scrollTop ?? 0) + viewport / 2
    const row = Math.max(0, Math.floor(midOffset / rowStride))
    const idx = Math.min(list.length - 1, row * Math.max(1, gridCols))
    anchorIndex = idx
    anchorPath = list[idx]?.path ?? null
  }

  const takeScrollTarget = ({ viewMode, gridCols, viewport }: { viewMode: ViewMode; gridCols: number; viewport: number }) => {
    if (!anchorPath) return null
    const list = get(filteredEntries)
    const anchor = anchorPath
    anchorPath = null
    // The entry order does not change during zoom. Avoid scanning a large
    // listing, but still resolve by path if sorting/filtering changed it.
    const idx = list[anchorIndex]?.path === anchor ? anchorIndex : list.findIndex((e) => e.path === anchor)
    anchorIndex = -1
    if (idx < 0) return 0

    if (viewMode === 'list') {
      const target = idx * rowHeight - Math.max(0, viewport / 2 - rowHeight / 2)
      return Math.max(0, target)
    }

    const rowStride = gridRowHeight + gridGap
    const row = Math.floor(idx / Math.max(1, gridCols))
    const target = row * rowStride - Math.max(0, viewport / 2 - rowStride / 2)
    return Math.max(0, target)
  }

  const scroll = ({ viewMode, rowsEl, gridEl, gridCols }: Refs & { viewMode: ViewMode }) => {
    const element = viewMode === 'list' ? rowsEl : gridEl
    const target = takeScrollTarget({ viewMode, gridCols, viewport: Math.max(0, element?.clientHeight ?? 0) })
    if (target !== null) element?.scrollTo({ top: target, behavior: 'auto' })
  }

  const setMetrics = (metrics: Pick<Options, 'rowHeight' | 'gridRowHeight' | 'gridGap'>) => {
    rowHeight = metrics.rowHeight
    gridRowHeight = metrics.gridRowHeight
    gridGap = metrics.gridGap
  }
  return { capture, scroll, takeScrollTarget, setMetrics }
}
