import { get, type Writable } from 'svelte/store'
import type { Entry } from '../model/types'
import { moveCaret } from '../helpers/navigationController'

type Deps = {
  getFilteredEntries: () => Entry[]
  selected: Writable<Set<string>>
  anchorIndex: Writable<number | null>
  caretIndex: Writable<number | null>
  getGridCols: () => number
  ensureGridVisible: (index: number) => void
  handleOpenEntry: (entry: Entry) => void | Promise<void>
}

/**
 * Keyboard navigation for grid mode.
 * Expects caller to gate on viewMode === 'grid' before invoking.
 */
export const createGridKeyboardHandler = ({
  getFilteredEntries,
  selected,
  anchorIndex,
  caretIndex,
  getGridCols,
  ensureGridVisible,
  handleOpenEntry,
}: Deps) => {
  return (event: KeyboardEvent) => {
    const list = getFilteredEntries()
    if (list.length === 0) return

    const key = event.key.toLowerCase()
    const selectedSet = get(selected)

    if (key === 'escape') {
      event.preventDefault()
      event.stopPropagation()
      selected.set(new Set())
      anchorIndex.set(null)
      caretIndex.set(null)
      return
    }

    if (key === 'enter') {
      event.preventDefault()
      event.stopPropagation()
      const idx =
        get(caretIndex) ??
        get(anchorIndex) ??
        list.findIndex((entry) => selectedSet.has(entry.path))
      if (idx !== null && idx >= 0) {
        const entry = list[idx]
        if (entry) {
          void handleOpenEntry(entry)
        }
      }
      return
    }

    let current = get(caretIndex) ?? get(anchorIndex)
    if (current === null) {
      current = key === 'arrowleft' || key === 'arrowup' ? list.length : -1
    }

    const rowDelta = Math.max(1, getGridCols())
    let next: number | null = null
    if (key === 'arrowright') next = moveCaret({ count: list.length, current, delta: 1 })
    else if (key === 'arrowleft') next = moveCaret({ count: list.length, current, delta: -1 })
    else if (key === 'arrowdown') next = moveCaret({ count: list.length, current, delta: rowDelta })
    else if (key === 'arrowup') next = moveCaret({ count: list.length, current, delta: -rowDelta })
    else if (key === 'home') next = moveCaret({ count: list.length, current, toStart: true })
    else if (key === 'end') next = moveCaret({ count: list.length, current, toEnd: true })
    else return

    if (next === null) return
    event.preventDefault()
    event.stopPropagation()

    if (event.shiftKey) {
      const anchor = get(anchorIndex) ?? current ?? next
      const lo = Math.min(anchor, next)
      const hi = Math.max(anchor, next)
      const range = new Set<string>()
      for (let i = lo; i <= hi; i++) {
        const path = list[i]?.path
        if (path) range.add(path)
      }
      selected.set(range)
      anchorIndex.set(anchor)
      caretIndex.set(next)
    } else {
      const path = list[next]?.path
      if (path) {
        selected.set(new Set([path]))
        anchorIndex.set(next)
        caretIndex.set(next)
      }
    }
    ensureGridVisible(next)
  }
}
