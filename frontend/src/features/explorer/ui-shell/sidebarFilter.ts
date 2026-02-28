import type { Partition } from '../model/types'

type SidebarEntry = { label: string; path: string }

type FilterParams = {
  query: string
  places: SidebarEntry[]
  bookmarks: SidebarEntry[]
  partitions: Partition[]
}

const normalize = (value: string) => value.trim().toLowerCase()

const matches = (query: string, ...values: Array<string | null | undefined>) => {
  if (!query) return true
  return values.some((value) => value?.toLowerCase().includes(query))
}

export const filterSidebarEntries = ({ query, places, bookmarks, partitions }: FilterParams) => {
  const normalized = normalize(query)
  return {
    places: places.filter((place) => matches(normalized, place.label, place.path)),
    bookmarks: bookmarks.filter((bookmark) => matches(normalized, bookmark.label, bookmark.path)),
    partitions: partitions.filter((partition) => matches(normalized, partition.label, partition.path)),
  }
}
