import { get, type Writable } from 'svelte/store'
import type { Column } from '../types'

export const createColumnResize = (cols: Writable<Column[]>, persist: () => Promise<void>) => {
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
    cols.update((list) =>
      list.map((c, i) =>
        i === resizeState!.index ? { ...c, width: Math.max(c.min, resizeState!.startWidth + delta) } : c
      )
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
