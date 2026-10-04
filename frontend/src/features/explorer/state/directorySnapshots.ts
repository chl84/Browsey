import type { Entry, Listing } from '../model/types'
import { withNameLower } from './helpers'

const sameFields = (a: object, b: object) => {
  const left = a as Record<string, unknown>
  const right = b as Record<string, unknown>
  const keys = Object.keys(left)
  return keys.length === Object.keys(right).length && keys.every(key => left[key] === right[key])
}

const reuseEntry = (previous: Entry | undefined, next: Entry): Entry => {
  if (!previous) return next
  const keys = Object.keys(next) as Array<keyof Entry>
  const equal = keys.length === Object.keys(previous).length && keys.every(key => key === 'capabilities'
    ? previous.capabilities === next.capabilities || Boolean(previous.capabilities && next.capabilities && sameFields(previous.capabilities, next.capabilities))
    : previous[key] === next[key])
  return equal ? previous : next
}

/** Keep known metadata while the backend refreshes a placeholder. Never retain deleted paths. */
export const reconcileDirectorySnapshot = (
  previous: Entry[], listing: Listing, preserveOrder: boolean, earlyMetadata: ReadonlyMap<string, Entry> = new Map(),
): Entry[] => {
  const existing = new Map(previous.map(entry => [entry.path, entry]))
  const pending = new Set(listing.pendingMetadataPaths ?? [])
  const incoming = new Map<string, Entry>()
  for (const entry of listing.entries) {
    const old = existing.get(entry.path)
    const early = pending.has(entry.path) ? earlyMetadata.get(entry.path) : undefined
    const next = early
      ? { ...entry, ...early, metadataPending: false }
      : pending.has(entry.path)
        ? { ...(old ?? entry), name: entry.name, nameLower: entry.name.toLowerCase(), starred: entry.starred, metadataPending: true }
        : { ...entry, metadataPending: false }
    incoming.set(entry.path, reuseEntry(old, withNameLower(next)))
  }
  const result: Entry[] = []
  if (preserveOrder) {
    for (const old of previous) {
      const next = incoming.get(old.path)
      if (next) {
        result.push(next)
        incoming.delete(old.path)
      }
    }
  }
  // Existing entries keep their slots; new entries arrive together at the end.
  result.push(...incoming.values())
  return result.length === previous.length && result.every((entry, i) => entry === previous[i]) ? previous : result
}

export const applyDirectoryMetadata = (list: Entry[], updates: ReadonlyMap<string, Entry>): Entry[] => {
  const result = list.map(entry => {
    const update = updates.get(entry.path)
    return update ? reuseEntry(entry, withNameLower({ ...entry, ...update, metadataPending: false })) : entry
  })
  return result.every((entry, i) => entry === list[i]) ? list : result
}
