import type { Column, Entry, ListingFacets, Location } from '../model/types'

export const FILTER_DEBOUNCE_MS = 40

export type ColumnFilters = {
  name: Set<string>
  type: Set<string>
  modified: Set<string>
  size: Set<string>
}

export type ExplorerCallbacks = {
  onEntriesChanged?: () => void
  onCurrentChange?: (path: string) => void
}

export const emptyListingFacets = (): ListingFacets => ({
  name: [],
  type: [],
  modified: [],
  size: [],
})

export const withNameLower = (entry: Entry): Entry => ({
  ...entry,
  nameLower: entry.nameLower ?? entry.name.toLowerCase(),
})

export const mapNameLower = (list: Entry[]) => list.map(withNameLower)

export const defaultColumns: Column[] = [
  { key: 'name', label: 'Name', sort: 'name', width: 320, min: 220, align: 'left' },
  { key: 'type', label: 'Type', sort: 'type', width: 120, min: 80 },
  { key: 'modified', label: 'Modified', sort: 'modified', width: 90, min: 80 },
  { key: 'size', label: 'Size', sort: 'size', width: 90, min: 70, align: 'right' },
  { key: 'star', label: '', sort: 'name', width: 25, min: 25, resizable: false, sortable: false },
]

export const sameLocation = (a?: Location, b?: Location) => {
  if (!a || !b) return false
  if (a.type !== b.type) return false
  if (a.type === 'dir' && b.type === 'dir') {
    return a.path === b.path
  }
  return true
}
