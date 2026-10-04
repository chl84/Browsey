import { afterEach, describe, expect, it, vi } from 'vitest'
import { useExplorerViewportLayout } from './useExplorerViewportLayout'

const setup = (viewMode: 'list' | 'grid' = 'grid') => {
  const styles = document.createElement('div').style
  for (const [name, value] of Object.entries({
    '--row-height': '32px', '--grid-gap': '8px', '--grid-thumb-size': '90px',
    '--grid-card-width': '120px', '--grid-row-height': '126px',
  })) styles.setProperty(name, value)
  const readStyle = vi.fn(() => styles)
  vi.stubGlobal('getComputedStyle', readStyle)
  const grid = { clientHeight: 400, scrollHeight: 1000, scrollTop: 0 } as HTMLDivElement
  const rows = { clientHeight: 400, scrollTop: 900 } as HTMLDivElement
  const params = {
    getViewMode: () => viewMode,
    getGridThumbSize: () => 128,
    setSidebarCollapsed: vi.fn(), listResize: vi.fn(), recomputeGrid: vi.fn(),
    setDensityMetrics: vi.fn(), recreateViewAnchor: vi.fn(),
    getFilteredEntries: () => [{}], getVisibleEntries: () => [{}],
    getGridTotalHeight: () => 1000, getTotalHeight: () => 1000,
    getRowsEl: () => rows, getGridEl: () => grid,
    updateViewportHeight: vi.fn(), recomputeList: vi.fn(),
  }
  return { layout: useExplorerViewportLayout(params), params, readStyle, styles, rows }
}

describe('viewport density metrics', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('reads the density styles once and lets zoom own the final reflow', () => {
    const { layout, params, readStyle } = setup()
    layout.applyDensityMetrics({ recompute: false })
    expect(readStyle).toHaveBeenCalledExactlyOnceWith(document.body)
    expect(params.setDensityMetrics).toHaveBeenCalledExactlyOnceWith({
      rowHeight: 32, gridGap: 8, gridCardWidth: 158, gridRowHeight: 164,
    })
    expect(params.recreateViewAnchor).toHaveBeenCalledExactlyOnceWith({ rowHeight: 32, gridGap: 8, gridRowHeight: 164 })
    expect(params.recomputeGrid).not.toHaveBeenCalled()
    expect(params.recomputeList).not.toHaveBeenCalled()
    expect(params.updateViewportHeight).not.toHaveBeenCalled()
  })

  it('still recomputes ordinary density changes in grid mode', () => {
    const { layout, params } = setup()
    layout.applyDensityMetrics()
    expect(params.recomputeGrid).toHaveBeenCalledTimes(1)
  })

  it('still clamps and recomputes ordinary list density changes', () => {
    const { layout, params, rows } = setup('list')
    layout.applyDensityMetrics()
    expect(rows.scrollTop).toBe(600)
    expect(params.updateViewportHeight).toHaveBeenCalledTimes(1)
    expect(params.recomputeList).toHaveBeenCalledExactlyOnceWith([{}])
    expect(params.recomputeGrid).not.toHaveBeenCalled()
  })

  it('uses current compact metrics while retaining the requested thumbnail size', () => {
    const { layout, params, styles } = setup()
    styles.setProperty('--grid-gap', '6px')
    styles.setProperty('--grid-thumb-size', '72px')
    styles.setProperty('--grid-card-width', '100px')
    styles.setProperty('--grid-row-height', '108px')
    layout.applyDensityMetrics({ recompute: false })
    expect(params.setDensityMetrics).toHaveBeenCalledExactlyOnceWith({
      rowHeight: 32, gridGap: 6, gridCardWidth: 156, gridRowHeight: 164,
    })
  })
})
