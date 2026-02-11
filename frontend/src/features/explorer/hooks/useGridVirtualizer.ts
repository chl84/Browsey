import { writable, type Writable } from 'svelte/store'
import type { Entry } from '../types'

type GridConfig = {
  cardWidth: number
  rowHeight: number
  gap: number
  overscan: number
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
  let scrollRaf: number | null = null

  const getGridCols = () => gridColsValue

  const getHorizontalPadding = (gridEl: HTMLDivElement) => {
    const styles = getComputedStyle(gridEl)
    const left = parseFloat(styles.paddingLeft) || 0
    const right = parseFloat(styles.paddingRight) || 0
    return left + right
  }

  const recomputeGrid = () => {
    if (getViewMode() !== 'grid') return
    const gridEl = getGridEl()
    if (!gridEl) return
    const list = getEntries()
    const width = Math.max(0, gridEl.clientWidth - getHorizontalPadding(gridEl))
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
    if (scrollRaf !== null) return
    scrollRaf = requestAnimationFrame(() => {
      scrollRaf = null
      recomputeGrid()
    })
  }

  const handleGridWheel = (event: WheelEvent) => {
    // Bruk native scroll; ingen custom behandling.
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
