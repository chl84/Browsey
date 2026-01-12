export type Entry = {
  name: string
  path: string
  kind: 'dir' | 'file' | 'link'
  ext?: string | null
  size?: number | null
  items?: number | null
  modified?: string | null
  accessed?: string | null
  created?: string | null
  starred?: boolean
  icon?: string
}

export type Listing = {
  current: string
  entries: Entry[]
}

export type SortField = 'name' | 'type' | 'modified' | 'size' | 'starred'
export type SortDirection = 'asc' | 'desc'

export type Location =
  | { type: 'dir'; path: string }
  | { type: 'recent' }
  | { type: 'starred' }
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
