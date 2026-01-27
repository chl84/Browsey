import type { Entry } from './types'

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
 * Per-mappe minne for utvalg/anker/caret slik at vi kan gå tilbake uten å miste markering.
 */
export const createSelectionMemory = () => {
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

    memory.set(path, {
      paths: filteredSelection,
      anchorPath,
      caretPath,
    })
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
