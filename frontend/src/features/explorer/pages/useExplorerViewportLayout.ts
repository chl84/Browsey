type ViewMode = 'list' | 'grid'

type DensityMetrics = {
  rowHeight: number
  gridGap: number
  gridCardWidth: number
  gridRowHeight: number
}

type Params = {
  getViewMode: () => ViewMode
  setSidebarCollapsed: (collapsed: boolean) => void
  listResize: () => void
  recomputeGrid: () => void
  setDensityMetrics: (metrics: DensityMetrics) => void
  recreateViewAnchor: (metrics: Pick<DensityMetrics, 'rowHeight' | 'gridGap' | 'gridRowHeight'>) => void
  getFilteredEntries: () => unknown[]
  getVisibleEntries: () => unknown[]
  getGridTotalHeight: () => number
  getTotalHeight: () => number
  getRowsEl: () => HTMLDivElement | null
  getGridEl: () => HTMLDivElement | null
  updateViewportHeight: () => void
  recomputeList: (entries: unknown[]) => void
  sidebarCollapseWidthPx?: number
}

export const useExplorerViewportLayout = (params: Params) => {
  const sidebarCollapseWidthPx = params.sidebarCollapseWidthPx ?? 700

  const readCssNumber = (name: string, fallback: number) => {
    if (typeof document === 'undefined') return fallback
    const raw = getComputedStyle(document.body).getPropertyValue(name)
    const parsed = parseFloat(raw)
    return Number.isFinite(parsed) ? parsed : fallback
  }

  const applyDensityClass = (density: string) => {
    if (typeof document === 'undefined') return
    document.body.classList.remove('density-cozy', 'density-compact')
    document.body.classList.add(`density-${density}`)
  }

  const applyDensityMetrics = () => {
    const nextRowHeight = readCssNumber('--row-height', 32)
    const nextGridGap = readCssNumber('--grid-gap', 6)
    const nextGridCardWidth = readCssNumber('--grid-card-width', 120)
    const nextGridRowHeight = readCssNumber('--grid-row-height', 126)

    params.setDensityMetrics({
      rowHeight: nextRowHeight,
      gridGap: nextGridGap,
      gridCardWidth: nextGridCardWidth,
      gridRowHeight: nextGridRowHeight,
    })

    params.recreateViewAnchor({
      rowHeight: nextRowHeight,
      gridRowHeight: nextGridRowHeight,
      gridGap: nextGridGap,
    })

    if (params.getViewMode() === 'grid') {
      params.recomputeGrid()
      const entriesList = params.getFilteredEntries()
      const visible = params.getVisibleEntries()
      const gridEl = params.getGridEl()
      if (entriesList.length > 0 && visible.length === 0 && gridEl) {
        const maxTop = Math.max(
          0,
          params.getGridTotalHeight() - gridEl.clientHeight,
          gridEl.scrollHeight - gridEl.clientHeight,
        )
        gridEl.scrollTop = Math.min(gridEl.scrollTop, maxTop)
        params.recomputeGrid()
      }
      return
    }

    const rowsEl = params.getRowsEl()
    const entriesList = params.getFilteredEntries()
    if (rowsEl) {
      const viewport = rowsEl.clientHeight
      const maxTop = Math.max(0, params.getTotalHeight() - viewport)
      if (rowsEl.scrollTop > maxTop) {
        rowsEl.scrollTop = maxTop
      }
    }
    params.updateViewportHeight()
    params.recomputeList(entriesList)
  }

  const handleResize = () => {
    if (typeof window === 'undefined') return
    params.setSidebarCollapsed(window.innerWidth < sidebarCollapseWidthPx)
    params.listResize()
    if (params.getViewMode() === 'grid') {
      params.recomputeGrid()
    }
  }

  return {
    applyDensityClass,
    applyDensityMetrics,
    handleResize,
  }
}
