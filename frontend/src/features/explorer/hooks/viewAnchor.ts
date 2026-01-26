import { get } from 'svelte/store'
import type { Readable } from 'svelte/store'
import type { Entry } from '../types'

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

  const capture = ({ viewMode, rowsEl, headerEl, gridEl, gridCols }: Refs & { viewMode: ViewMode }) => {
    const list = get(filteredEntries)
    if (list.length === 0) {
      anchorPath = null
      return
    }

    if (viewMode === 'list') {
      const headerHeight = headerEl?.offsetHeight ?? 0
      const viewport = Math.max(0, (rowsEl?.clientHeight ?? 0) - headerHeight)
      const midOffset = Math.max(0, (rowsEl?.scrollTop ?? 0) - headerHeight + viewport / 2)
      const idx = Math.min(list.length - 1, Math.floor(midOffset / rowHeight))
      anchorPath = list[idx]?.path ?? null
      return
    }

    if (!gridEl) {
      anchorPath = null
      return
    }

    const rowStride = gridRowHeight + gridGap
    const viewport = gridEl.clientHeight
    const midOffset = (gridEl.scrollTop ?? 0) + viewport / 2
    const row = Math.max(0, Math.floor(midOffset / rowStride))
    const idx = Math.min(list.length - 1, row * Math.max(1, gridCols))
    anchorPath = list[idx]?.path ?? null
  }

  const scroll = ({ viewMode, rowsEl, headerEl, gridEl, gridCols }: Refs & { viewMode: ViewMode }) => {
    if (!anchorPath) return
    const list = get(filteredEntries)
    const anchor = anchorPath
    anchorPath = null
    const idx = list.findIndex((e) => e.path === anchor)

    if (idx < 0) {
      if (viewMode === 'list') {
        rowsEl?.scrollTo({ top: 0 })
      } else {
        gridEl?.scrollTo({ top: 0 })
      }
      return
    }

    if (viewMode === 'list') {
      const headerHeight = headerEl?.offsetHeight ?? 0
      const viewport = Math.max(0, (rowsEl?.clientHeight ?? 0) - headerHeight)
      const target = headerHeight + idx * rowHeight - Math.max(0, viewport / 2 - rowHeight / 2)
      rowsEl?.scrollTo({ top: Math.max(0, target), behavior: 'auto' })
      return
    }

    const rowStride = gridRowHeight + gridGap
    const row = Math.floor(idx / Math.max(1, gridCols))
    const viewport = gridEl?.clientHeight ?? 0
    const target = row * rowStride - Math.max(0, viewport / 2 - rowStride / 2)
    gridEl?.scrollTo({ top: Math.max(0, target), behavior: 'auto' })
  }

  return { capture, scroll }
}
