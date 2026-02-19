export type EntryLike = { path: string }

export const selectAllPaths = (entries: EntryLike[]) => {
  const next = new Set<string>()
  for (const entry of entries) {
    next.add(entry.path)
  }
  return next
}

export const clearSelection = () => new Set<string>()

export const selectRange = (entries: EntryLike[], start: number, end: number) => {
  const next = new Set<string>()
  const lo = Math.max(0, Math.min(start, end))
  const hi = Math.min(entries.length - 1, Math.max(start, end))
  for (let i = lo; i <= hi; i++) {
    next.add(entries[i].path)
  }
  return next
}

export const clampIndex = (idx: number, entries: EntryLike[]) => {
  if (entries.length === 0) return -1
  return Math.max(0, Math.min(idx, entries.length - 1))
}
