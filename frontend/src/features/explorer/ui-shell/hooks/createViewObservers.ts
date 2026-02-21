export const createViewObservers = () => {
  let rowsObserver: ResizeObserver | null = null
  let gridObserver: ResizeObserver | null = null
  let rowsRaf: number | null = null
  let gridRaf: number | null = null

  const disconnectRows = () => {
    rowsObserver?.disconnect()
    rowsObserver = null
    if (rowsRaf !== null) {
      cancelAnimationFrame(rowsRaf)
      rowsRaf = null
    }
  }

  const disconnectGrid = () => {
    gridObserver?.disconnect()
    gridObserver = null
    if (gridRaf !== null) {
      cancelAnimationFrame(gridRaf)
      gridRaf = null
    }
  }

  const setupRows = (rowsEl: HTMLDivElement | null, onResize: () => void) => {
    if (!rowsEl || typeof ResizeObserver === 'undefined') return
    disconnectRows()
    rowsObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (entry.contentRect.height > 0) {
          if (rowsRaf === null) {
            rowsRaf = requestAnimationFrame(() => {
              rowsRaf = null
              onResize()
            })
          }
        }
      }
    })
    rowsObserver.observe(rowsEl)
  }

  const setupGrid = (gridEl: HTMLDivElement | null, viewMode: 'list' | 'grid', onResize: () => void) => {
    if (!gridEl || viewMode !== 'grid' || typeof ResizeObserver === 'undefined') return
    disconnectGrid()
    gridObserver = new ResizeObserver(() => {
      if (gridRaf === null) {
        gridRaf = requestAnimationFrame(() => {
          gridRaf = null
          onResize()
        })
      }
    })
    gridObserver.observe(gridEl)
  }

  const cleanup = () => {
    disconnectRows()
    disconnectGrid()
  }

  return {
    setupRows,
    setupGrid,
    disconnectGrid,
    cleanup,
  }
}
