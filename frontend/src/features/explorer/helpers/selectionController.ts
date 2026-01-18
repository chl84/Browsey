import type { Entry } from '../types'

export type SelectionState = {
  selected: Set<string>
  anchor: number | null
  caret: number | null
}

export const applyClickSelection = (
  entries: Entry[],
  absoluteIndex: number,
  event: MouseEvent,
  prev: SelectionState,
): SelectionState => {
  const isToggle = event.ctrlKey || event.metaKey
  const isRange = event.shiftKey && prev.anchor !== null

  if (isRange && prev.anchor !== null) {
    const lo = Math.min(prev.anchor, absoluteIndex)
    const hi = Math.max(prev.anchor, absoluteIndex)
    const rangeSet = new Set<string>()
    for (let i = lo; i <= hi; i++) {
      const entry = entries[i]
      if (entry) rangeSet.add(entry.path)
    }
    if (isToggle) {
      const merged = new Set(prev.selected)
      rangeSet.forEach((p) => merged.add(p))
      return { selected: merged, anchor: prev.anchor, caret: absoluteIndex }
    }
    return { selected: rangeSet, anchor: prev.anchor, caret: absoluteIndex }
  }

  if (isToggle) {
    const next = new Set(prev.selected)
    const path = entries[absoluteIndex]?.path
    if (!path) return prev
    if (next.has(path)) {
      next.delete(path)
    } else {
      next.add(path)
    }
    return { selected: next, anchor: absoluteIndex, caret: absoluteIndex }
  }

  const only = entries[absoluteIndex]?.path
  return only ? { selected: new Set([only]), anchor: absoluteIndex, caret: absoluteIndex } : prev
}

export const clearSelection = (): SelectionState => ({
  selected: new Set(),
  anchor: null,
  caret: null,
})
