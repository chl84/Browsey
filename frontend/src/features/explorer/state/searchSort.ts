import type { Entry, SortDirection, SortField } from '../model/types'
import { mapNameLower } from './helpers'

const compareString = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0)

const kindRank = (k: string) => (k === 'dir' ? 0 : k === 'file' ? 1 : 2)

const sizeSortKindRank = (k: Entry['kind']) => {
  switch (k) {
    case 'file':
      return 0
    case 'link':
      return 1
    case 'dir':
      return 3
    default:
      return 2
  }
}

const compareOptionalNumber = (a: number | null | undefined, b: number | null | undefined) => {
  const aHas = typeof a === 'number'
  const bHas = typeof b === 'number'
  if (aHas && bHas) return (a as number) - (b as number)
  if (aHas && !bHas) return -1
  if (!aHas && bHas) return 1
  return 0
}

const compareSizeField = (
  a: { kindRank: number; numeric: number | null | undefined; nameKey: string },
  b: { kindRank: number; numeric: number | null | undefined; nameKey: string },
  dir: number,
) => {
  const rankCmp = a.kindRank - b.kindRank
  if (rankCmp !== 0) return rankCmp
  const numericCmp = compareOptionalNumber(a.numeric, b.numeric)
  if (numericCmp !== 0) return dir * numericCmp
  return dir * compareString(a.nameKey, b.nameKey)
}

export const sortExplorerEntriesInMemory = (
  list: Entry[],
  spec: { field: SortField; direction: SortDirection },
) => {
  const dir = spec.direction === 'asc' ? 1 : -1
  const decorated = list.map((entry, index) => {
    const nameKey = entry.nameLower ?? entry.name.toLowerCase()
    return {
      entry,
      index,
      nameKey,
      typeKindRank: kindRank(entry.kind),
      typeExtKey: (entry.ext ?? '').toLowerCase(),
      modifiedKey: entry.modified ?? '',
      sizeKey: {
        kindRank: sizeSortKindRank(entry.kind),
        numeric: entry.kind === 'dir' ? entry.items : entry.size,
        nameKey,
      },
    }
  })
  decorated.sort((a, b) => {
    const cmp = (() => {
      switch (spec.field) {
        case 'name':
          return compareString(a.nameKey, b.nameKey)
        case 'type': {
          const kindCmp = a.typeKindRank - b.typeKindRank
          if (kindCmp !== 0) return kindCmp
          const extCmp = compareString(a.typeExtKey, b.typeExtKey)
          if (extCmp !== 0) return extCmp
          return compareString(a.nameKey, b.nameKey)
        }
        case 'modified': {
          const modifiedCmp = compareString(a.modifiedKey, b.modifiedKey)
          if (modifiedCmp !== 0) return modifiedCmp
          return compareString(a.nameKey, b.nameKey)
        }
        case 'size':
          return compareSizeField(a.sizeKey, b.sizeKey, dir)
        default:
          return 0
      }
    })()
    if (cmp !== 0) {
      return spec.field === 'size' ? cmp : dir * cmp
    }
    return a.index - b.index
  })
  return mapNameLower(decorated.map((item) => item.entry))
}
