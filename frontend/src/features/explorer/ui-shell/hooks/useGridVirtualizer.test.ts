import { afterEach, describe, expect, it, vi } from 'vitest'
import { get, writable } from 'svelte/store'
import { useGridVirtualizer } from './useGridVirtualizer'
import type { Entry } from '../../model/types'

const setup = (count = 1000) => {
  const entries = Array.from({ length: count }, (_, i) => ({ path: `/mock/${i}`, name: `${i}`, kind: 'file', iconId: 1 } as Entry))
  const styles = document.createElement('div').style
  styles.padding = '20px 20px 32px'
  const readStyle = vi.fn(() => styles)
  vi.stubGlobal('getComputedStyle', readStyle)
  const grid = { clientWidth: 500, clientHeight: 400, scrollTop: 0 } as HTMLDivElement
  const visibleEntries = writable<Entry[]>([])
  const start = writable(0), offsetY = writable(0), totalHeight = writable(0)
  const config = { cardWidth: 100, rowHeight: 100, gap: 0, overscan: 1 }
  const virtualizer = useGridVirtualizer({ getEntries: () => entries, getViewMode: () => 'grid', getGridEl: () => grid,
    visibleEntries, start, offsetY, totalHeight, config })
  return { virtualizer, entries, grid, visibleEntries, start, totalHeight, config, readStyle }
}

describe('anchored grid zoom reflow', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('publishes the final anchored window in one calculation without moving the old DOM', () => {
    const { virtualizer, grid, visibleEntries, start, totalHeight, readStyle } = setup()
    const publications: Entry[][] = []
    const unsubscribe = visibleEntries.subscribe(value => publications.push(value))
    const resolve = vi.fn(() => 1000)
    expect(virtualizer.recomputeGrid(resolve)).toBe(1000)
    expect(resolve).toHaveBeenCalledExactlyOnceWith(4, 400)
    expect(readStyle).toHaveBeenCalledExactlyOnceWith(grid)
    expect(grid.scrollTop).toBe(0)
    expect(get(start)).toBe(36)
    expect(get(totalHeight)).toBe(25000)
    expect(get(visibleEntries)[0].path).toBe('/mock/36')
    expect(publications).toHaveLength(2) // Initial store value and final window only.
    unsubscribe()
  })

  it('clamps the anchor against the new content height when zooming out at the bottom', () => {
    const { virtualizer, grid, start, visibleEntries, totalHeight, config } = setup(100)
    grid.scrollTop = 9000
    config.cardWidth = 200
    expect(virtualizer.recomputeGrid(() => 100000)).toBe(4652)
    expect(get(totalHeight)).toBe(5000)
    expect(get(start)).toBe(90)
    expect(get(visibleEntries).at(-1)?.path).toBe('/mock/99')
    config.cardWidth = 100
    expect(virtualizer.recomputeGrid(() => 100000)).toBe(2152)
    expect(get(totalHeight)).toBe(2500)
    expect(get(visibleEntries).at(-1)?.path).toBe('/mock/99')
  })

  it('clamps negative anchors and handles listings shorter than the viewport', () => {
    const { virtualizer, start, visibleEntries } = setup(2)
    expect(virtualizer.recomputeGrid(() => -100)).toBe(0)
    expect(virtualizer.recomputeGrid(() => 100)).toBe(0)
    expect(get(start)).toBe(0)
    expect(get(visibleEntries)).toHaveLength(2)
  })

  it('keeps ordinary scrolling virtualized without an anchor callback', () => {
    const { virtualizer, grid, visibleEntries, start } = setup(100000)
    grid.scrollTop = 1000
    expect(virtualizer.recomputeGrid()).toBe(1000)
    expect(get(start)).toBe(36)
    expect(get(visibleEntries)).toHaveLength(24)
  })
})
