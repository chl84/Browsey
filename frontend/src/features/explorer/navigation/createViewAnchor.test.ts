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
})
