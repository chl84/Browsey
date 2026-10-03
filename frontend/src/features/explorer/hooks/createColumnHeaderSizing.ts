import { get, type Writable } from 'svelte/store'
import type { Column } from '../model/types'

const cssNumber = (value: string) => parseFloat(value) || 0
const outerWidth = (element: HTMLElement | null) => {
  if (!element) return 0
  const style = getComputedStyle(element)
  return element.getBoundingClientRect().width + cssNumber(style.marginLeft) + cssNumber(style.marginRight)
}

/** Keep both dragging and restored widths above the actual header's intrinsic size. */
export const createColumnHeaderSizing = (cols: Writable<Column[]>) => {
  const baseMinimums = new Map(get(cols).map(col => [col.key, col.min]))
  let observedHeader: HTMLElement | null = null
  let resizeObserver: ResizeObserver | null = null
  let styleObserver: MutationObserver | null = null
  let frame: number | null = null

  const measure = (header = observedHeader) => {
    if (!header || header.getBoundingClientRect().width === 0) return
    const cells = [...header.querySelectorAll<HTMLElement>('.header-cell')]
    let changed = false
    const next = get(cols).map((col, index) => {
      const cell = cells[index]
      const label = cell?.querySelector<HTMLElement>('.header-label')
      if (!cell || !label) return col
      const button = cell.querySelector<HTMLElement>('.header-btn')!
      const controls = cell.querySelector<HTMLElement>('.header-controls')!
      const buttonStyle = getComputedStyle(button)
      const filter = cell.querySelector<HTMLElement>('.filter-btn')
      const grip = cell.querySelector<HTMLElement>('.column-resizer')
      const required = label.scrollWidth
        + outerWidth(cell.querySelector<HTMLElement>('.sort-icon')) + cssNumber(buttonStyle.columnGap)
        + cssNumber(buttonStyle.paddingLeft) + cssNumber(buttonStyle.paddingRight)
        // The filter's auto margin is leftover space, not intrinsic width.
        + (filter?.getBoundingClientRect().width ?? 0) + (filter ? cssNumber(getComputedStyle(controls).columnGap) : 0)
        + outerWidth(grip) + (grip ? cssNumber(getComputedStyle(cell).columnGap) : 0)
      // Round up with a small allowance for fractional glyph metrics. Base
      // minima protect row content; measured minima protect the whole header.
      const min = Math.max(baseMinimums.get(col.key) ?? col.min, Math.ceil(required) + 1)
      const width = Math.max(col.width, min)
      if (min === col.min && width === col.width) return col
      changed = true
      return { ...col, min, width }
    })
    if (changed) cols.set(next)
  }

  const schedule = () => {
    if (!observedHeader || frame !== null) return
    frame = requestAnimationFrame(() => { frame = null; measure() })
  }
  const disconnect = () => {
    resizeObserver?.disconnect()
    styleObserver?.disconnect()
    resizeObserver = null
    styleObserver = null
    document.fonts?.removeEventListener('loadingdone', schedule)
    if (frame !== null) cancelAnimationFrame(frame)
    frame = null
    observedHeader = null
  }
  const observe = (header: HTMLElement | null) => {
    if (header === observedHeader) return
    disconnect()
    observedHeader = header
    if (!header) return
    resizeObserver = new ResizeObserver(schedule)
    resizeObserver.observe(header)
    header.querySelectorAll('.header-label, .sort-icon, .filter-btn, .column-resizer')
      .forEach(element => resizeObserver!.observe(element))
    styleObserver = new MutationObserver(schedule)
    styleObserver.observe(document.body, { attributes: true, attributeFilter: ['class', 'style'] })
    styleObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['class', 'style'] })
    document.fonts?.addEventListener('loadingdone', schedule)
    void document.fonts?.ready.then(schedule)
    schedule()
  }
  return { measure, observe, cleanup: disconnect }
}
