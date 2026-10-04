import { describe, expect, it, vi } from 'vitest'
import { writable } from 'svelte/store'
import { createViewSwitchAnchor } from './createViewAnchor'
import type { Entry } from '../model/types'

describe('view anchor during zoom', () => {
  it('retains the anchor when grid metrics and column count change', () => {
    const entries = Array.from({ length: 100 }, (_, i) => ({ path: `/mock/${i}`, name: `${i}`, kind: 'file', iconId: 1 } as Entry))
    const anchor = createViewSwitchAnchor({ filteredEntries: writable(entries), rowHeight: 32, gridRowHeight: 100, gridGap: 0 })
    const grid = { scrollTop: 200, clientHeight: 400, scrollTo: vi.fn() } as unknown as HTMLDivElement
    anchor.capture({ viewMode: 'grid', gridEl: grid, rowsEl: null, headerEl: null, gridCols: 5 })
    anchor.setMetrics({ rowHeight: 32, gridRowHeight: 200, gridGap: 0 })
    anchor.scroll({ viewMode: 'grid', gridEl: grid, rowsEl: null, headerEl: null, gridCols: 3 })
    // Old centre was item 20; it is now on row 6.
    expect(grid.scrollTo).toHaveBeenCalledWith({ top: 1100, behavior: 'auto' })
  })

  it('resolves the target without scanning unchanged entries or touching the DOM', () => {
    const entries = Array.from({ length: 100 }, (_, i) => ({ path: `/mock/${i}` } as Entry))
    const scan = vi.spyOn(entries, 'findIndex')
    const anchor = createViewSwitchAnchor({ filteredEntries: writable(entries), rowHeight: 32, gridRowHeight: 100, gridGap: 0 })
    const grid = { scrollTop: 200, clientHeight: 400 } as HTMLDivElement
    anchor.capture({ viewMode: 'grid', gridEl: grid, rowsEl: null, headerEl: null, gridCols: 5 })
    anchor.setMetrics({ rowHeight: 32, gridRowHeight: 200, gridGap: 0 })
    expect(anchor.takeScrollTarget({ viewMode: 'grid', gridCols: 3, viewport: 400 })).toBe(1100)
    expect(scan).not.toHaveBeenCalled()
    expect(anchor.takeScrollTarget({ viewMode: 'grid', gridCols: 3, viewport: 400 })).toBeNull()
  })

  it('falls back to the path if sorting changes the captured index', () => {
    const entries = Array.from({ length: 100 }, (_, i) => ({ path: `/mock/${i}` } as Entry))
    const store = writable(entries)
    const anchor = createViewSwitchAnchor({ filteredEntries: store, rowHeight: 32, gridRowHeight: 100, gridGap: 0 })
    const grid = { scrollTop: 200, clientHeight: 400 } as HTMLDivElement
    anchor.capture({ viewMode: 'grid', gridEl: grid, rowsEl: null, headerEl: null, gridCols: 5 })
    store.set([...entries].reverse())
    // Captured item 20 moves to index 79, row 15 at five columns.
    expect(anchor.takeScrollTarget({ viewMode: 'grid', gridCols: 5, viewport: 400 })).toBe(1350)
  })
})
