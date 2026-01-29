import { get, type Writable } from 'svelte/store'
import type { Column } from '../types'

export const createColumnResize = (
  cols: Writable<Column[]>,
  persist: () => Promise<void>,
  getMaxWidth?: () => number | null,
) => {
  let resizeState: { index: number; startX: number; startWidth: number } | null = null

  const startResize = (index: number, event: PointerEvent) => {
    event.preventDefault()
    event.stopPropagation()
    const list = get(cols)
    resizeState = {
      index,
      startX: event.clientX,
      startWidth: list[index].width,
    }
    window.addEventListener('pointermove', handleResizeMove)
    window.addEventListener('pointerup', handleResizeEnd, { once: true })
  }

  const handleResizeMove = (event: PointerEvent) => {
    if (!resizeState) return
    const delta = event.clientX - resizeState.startX
    const list = get(cols)
    const maxWidth = getMaxWidth?.() ?? null

    const otherWidth = list.reduce((sum, col, idx) => (idx === resizeState!.index ? sum : sum + col.width), 0)
    const proposed = Math.max(list[resizeState.index].min, resizeState.startWidth + delta)
    const capped =
      maxWidth === null
        ? proposed
        : Math.min(proposed, Math.max(list[resizeState.index].min, maxWidth - otherWidth))

    cols.set(
      list.map((c, i) => (i === resizeState!.index ? { ...c, width: capped } : c)),
    )
  }

  const handleResizeEnd = () => {
    resizeState = null
    window.removeEventListener('pointermove', handleResizeMove)
    void persist()
  }

  return {
    startResize,
    handleResizeEnd,
  }
}
