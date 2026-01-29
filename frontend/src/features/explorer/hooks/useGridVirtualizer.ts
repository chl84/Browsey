import { writable, type Writable } from 'svelte/store'
import type { Entry } from '../types'

type GridConfig = {
  cardWidth: number
  rowHeight: number
  gap: number
  padding: number
  overscan: number
  wheelScale: number
}

type Params = {
  getEntries: () => Entry[]
  getViewMode: () => 'list' | 'grid'
  getGridEl: () => HTMLDivElement | null
  start: Writable<number>
  offsetY: Writable<number>
  totalHeight: Writable<number>
  visibleEntries: Writable<Entry[]>
  config: GridConfig
}

export const useGridVirtualizer = ({
  getEntries,
  getViewMode,
  getGridEl,
  start,
  offsetY,
  totalHeight,
  visibleEntries,
  config,
}: Params) => {
  const gridStart = writable(0)
  const gridOffsetY = writable(0)
  const gridTotalHeight = writable(0)
  const gridColsStore = writable(1)
  let gridColsValue = 1

  let gridWheelRaf: number | null = null
  let gridPendingDeltaX = 0
  let gridPendingDeltaY = 0

  const getGridCols = () => gridColsValue

  const recomputeGrid = () => {
    if (getViewMode() !== 'grid') return
    const gridEl = getGridEl()
    if (!gridEl) return
    const list = getEntries()
    const width = Math.max(0, gridEl.clientWidth - config.padding * 2)
    gridColsValue = Math.max(1, Math.floor((width + config.gap) / (config.cardWidth + config.gap)))
    gridColsStore.set(gridColsValue)
    const rowStride = config.rowHeight + config.gap
    const totalRows = Math.ceil(list.length / gridColsValue)
    const scrollTop = gridEl.scrollTop
    const viewport = gridEl.clientHeight
    const startRow = Math.max(0, Math.floor(scrollTop / rowStride) - config.overscan)
    const endRow = Math.min(totalRows, Math.ceil((scrollTop + viewport) / rowStride) + config.overscan)
    const startIdx = startRow * gridColsValue
    const endIdx = Math.min(list.length, endRow * gridColsValue)
    gridStart.set(startIdx)
    gridOffsetY.set(startRow * rowStride)
    const totalH = totalRows * rowStride
    gridTotalHeight.set(totalH)
    visibleEntries.set(list.slice(startIdx, endIdx))
    start.set(startIdx)
    offsetY.set(startRow * rowStride)
    totalHeight.set(totalH)
  }

  const handleGridScroll = () => {
    if (getViewMode() !== 'grid') return
    recomputeGrid()
  }

  const handleGridWheel = (event: WheelEvent) => {
    const el = getGridEl()
    if (!el) return
    gridPendingDeltaX += event.deltaX * config.wheelScale
    gridPendingDeltaY += event.deltaY * config.wheelScale
    if (gridWheelRaf !== null) return
    gridWheelRaf = requestAnimationFrame(() => {
      el.scrollLeft += gridPendingDeltaX
      el.scrollTop += gridPendingDeltaY
      gridPendingDeltaX = 0
      gridPendingDeltaY = 0
      gridWheelRaf = null
    })
  }

  const ensureGridVisible = (index: number) => {
    const gridEl = getGridEl()
    const cols = getGridCols()
    if (!gridEl || cols <= 0) return
    const rowStride = config.rowHeight + config.gap
    const row = Math.floor(index / Math.max(1, cols))
    const top = row * rowStride
    const bottom = top + rowStride
    const currentTop = gridEl.scrollTop
    const currentBottom = currentTop + gridEl.clientHeight
    if (top < currentTop) {
      gridEl.scrollTo({ top })
    } else if (bottom > currentBottom) {
      gridEl.scrollTo({ top: bottom - gridEl.clientHeight })
    }
  }

  return {
    gridCols: gridColsStore,
    getGridCols,
    gridStart,
    gridOffsetY,
    gridTotalHeight,
    recomputeGrid,
    handleGridScroll,
    handleGridWheel,
    ensureGridVisible,
  }
}
