import type { Entry } from '../model/types'

type StoredSelection = {
  paths: Set<string>
  anchorPath: string | null
  caretPath: string | null
}

type RestoreResult = {
  selected: Set<string>
  anchorIndex: number | null
  caretIndex: number | null
}

/**
 * Per-folder memory for selection/anchor/caret so we can navigate back without losing selection.
 */
export const createSelectionMemory = () => {
  const MAX_SELECTION_SNAPSHOTS = 10
  const memory = new Map<string, StoredSelection>()

  const capture = (
    path: string,
    filteredEntries: Entry[],
    selected: Set<string>,
    anchorIndex: number | null,
    caretIndex: number | null,
  ) => {
    if (!path) return
    const pathsInList = new Set(filteredEntries.map((e) => e.path))
    const filteredSelection = new Set([...selected].filter((p) => pathsInList.has(p)))
    const anchorPath = anchorIndex !== null ? filteredEntries[anchorIndex]?.path ?? null : null
    const caretPath = caretIndex !== null ? filteredEntries[caretIndex]?.path ?? null : null

    // Update LRU order: remove first if present, then insert again.
    if (memory.has(path)) {
      memory.delete(path)
    }
    memory.set(path, {
      paths: filteredSelection,
      anchorPath,
      caretPath,
    })

    while (memory.size > MAX_SELECTION_SNAPSHOTS) {
      const oldest = memory.keys().next().value
      if (oldest !== undefined) {
        memory.delete(oldest)
      } else {
        break
      }
    }
  }

  const restore = (path: string, filteredEntries: Entry[]): RestoreResult | null => {
    const stored = memory.get(path)
    if (!stored) return null

    const indexByPath = new Map(filteredEntries.map((e, i) => [e.path, i]))
    const selected = new Set([...stored.paths].filter((p) => indexByPath.has(p)))
    if (selected.size === 0) return null

    const firstSelected = indexByPath.get([...selected][0]) ?? null
    const anchorIndex =
      stored.anchorPath && indexByPath.has(stored.anchorPath)
        ? indexByPath.get(stored.anchorPath)!
        : firstSelected
    const caretIndex =
      stored.caretPath && indexByPath.has(stored.caretPath)
        ? indexByPath.get(stored.caretPath)!
        : anchorIndex

    return {
      selected,
      anchorIndex,
      caretIndex,
    }
  }

  return { capture, restore }
}
