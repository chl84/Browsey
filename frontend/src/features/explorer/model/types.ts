export type Entry = {
  name: string
  nameLower?: string
  path: string
  kind: 'dir' | 'file' | 'link'
  ext?: string | null
  size?: number | null
  items?: number | null
  modified?: string | null
  original_path?: string | null
  trash_id?: string | null
  accessed?: string | null
  created?: string | null
  starred?: boolean
  iconId: number
  hidden?: boolean
  network?: boolean
  readOnly?: boolean
  readDenied?: boolean
}

export type Listing = {
  current: string
  entries: Entry[]
}

export type SortField = 'name' | 'type' | 'modified' | 'size'
export type DefaultSortField = SortField
export type SortDirection = 'asc' | 'desc'
export type Density = 'cozy' | 'compact'

export type FilterOption = {
  id: string
  label: string
  description?: string
}

export type ListingFacets = {
  name: FilterOption[]
  type: FilterOption[]
  modified: FilterOption[]
  size: FilterOption[]
}

export type Location =
  | { type: 'dir'; path: string }
  | { type: 'recent' }
  | { type: 'starred' }
  | { type: 'network' }
  | { type: 'trash' }

export type Partition = {
  label: string
  path: string
  fs?: string
  removable?: boolean
}

export type Column = {
  key: string
  label: string
  sort: SortField
  width: number
  min: number
  align?: 'left' | 'right' | 'center'
  resizable?: boolean
  sortable?: boolean
}
