import { describe, expect, it } from 'vitest'
import { get, writable } from 'svelte/store'
import type { Column } from '../model/types'
import { createColumnHeaderSizing } from './createColumnHeaderSizing'
import { createColumnResize } from './createColumnResize'

const fixture = (width = 90) => {
  const cols = writable<Column[]>([{ key: 'modified', label: 'Modified', sort: 'modified', width, min: 80 }])
  const header = document.createElement('div')
  header.innerHTML = `<div class="header-cell" style="column-gap:6px">
    <div class="header-controls" style="column-gap:5px">
      <button class="header-btn" style="column-gap:6px;padding:0">
        <span class="header-label">Modified</span><span class="sort-icon">▲</span>
      </button><button class="filter-btn" style="margin-left:999px"></button>
    </div><span class="column-resizer" style="margin-left:2px"></span>
  </div>`
  const mockWidth = (element: Element, value: number) => {
    element.getBoundingClientRect = () => ({ width: value }) as DOMRect
  }
  mockWidth(header, 800)
  mockWidth(header.querySelector('.sort-icon')!, 11)
  mockWidth(header.querySelector('.filter-btn')!, 17)
  mockWidth(header.querySelector('.column-resizer')!, 10)
  const label = header.querySelector('.header-label')!
  let textWidth = 75
  Object.defineProperty(label, 'scrollWidth', { get: () => textWidth })
  const sizing = createColumnHeaderSizing(cols)
  return { cols, header, sizing, setTextWidth: (width: number) => { textWidth = width } }
}

describe('intrinsic column header minimums', () => {
  it('includes label, sort/filter targets, spacing and the resize handle in its minimum', () => {
    const { cols, header, sizing } = fixture()
    sizing.measure(header)
    expect(get(cols)[0]).toMatchObject({ min: 133, width: 133 })
    // A second measurement is stable and does not trigger redundant rendering.
    const previous = get(cols)
    sizing.measure(header)
    expect(get(cols)).toBe(previous)
  })

  it('keeps user widths and allows minimums to shrink after font/density changes', () => {
    const { cols, header, sizing, setTextWidth } = fixture(300)
    sizing.measure(header)
    setTextWidth(90)
    sizing.measure(header)
    expect(get(cols)[0]).toMatchObject({ min: 148, width: 300 })
    setTextWidth(40)
    sizing.measure(header)
    expect(get(cols)[0]).toMatchObject({ min: 98, width: 300 })
    setTextWidth(5)
    sizing.measure(header)
    expect(get(cols)[0].min).toBe(80)
  })

  it('does not measure hidden or absent headers', () => {
    const { cols, header, sizing } = fixture()
    header.getBoundingClientRect = () => ({ width: 0 }) as DOMRect
    sizing.measure(header)
    sizing.measure(null)
    expect(get(cols)[0]).toMatchObject({ min: 80, width: 90 })
  })

  it('enforces the measured minimum during dragging, even in a smaller viewport', () => {
    const { cols, header, sizing } = fixture()
    sizing.measure(header)
    const resize = createColumnResize(cols, async () => {}, () => 100)
    resize.startResize(0, new MouseEvent('pointerdown', { clientX: 200 }) as PointerEvent)
    try {
      window.dispatchEvent(new MouseEvent('pointermove', { clientX: -500 }))
      expect(get(cols)[0].width).toBe(133)
    } finally {
      resize.handleResizeEnd()
    }
  })
})
