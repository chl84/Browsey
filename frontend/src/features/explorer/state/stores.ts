import { derived, writable } from 'svelte/store'
import type { DefaultSortField, Density, Entry, Location, Partition, SortDirection, SortField } from '../model/types'
import { defaultColumns } from './helpers'

export const createExplorerStores = () => {
  const cols = writable(defaultColumns)
  const gridTemplate = derived(cols, ($cols) => $cols.map((c) => `${Math.max(c.width, c.min)}px`).join(' '))

  const current = writable('')
  const entries = writable<Entry[]>([])
  const loading = writable(false)
  const error = writable('')
  const filter = writable('')
  const searchMode = writable(false)
  const searchRunning = writable(false)
  const showHidden = writable(true)
  const hiddenFilesLast = writable(false)
  const foldersFirst = writable(true)
  const startDirPref = writable<string | null>(null)
  const confirmDelete = writable(true)
  const sortFieldPref = writable<DefaultSortField>('name')
  const sortDirectionPref = writable<SortDirection>('asc')
  const sortField = writable<SortField>('name')
  const sortDirection = writable<SortDirection>('asc')
  const density = writable<Density>('cozy')
  const archiveName = writable<string>('Archive')
  const archiveLevel = writable<number>(6)
  const openDestAfterExtract = writable<boolean>(false)
  const videoThumbs = writable<boolean>(true)
  const hardwareAcceleration = writable<boolean>(true)
  const ffmpegPath = writable<string>('')
  const thumbCacheMb = writable<number>(300)
  const mountsPollMs = writable<number>(8000)
  const doubleClickMs = writable<number>(300)
  const bookmarks = writable<{ label: string; path: string }[]>([])
  const partitions = writable<Partition[]>([])
  const history = writable<Location[]>([])
  const historyIndex = writable(-1)

  return {
    cols,
    gridTemplate,
    current,
    entries,
    loading,
    error,
    filter,
    searchMode,
    searchRunning,
    showHidden,
    hiddenFilesLast,
    foldersFirst,
    startDirPref,
    confirmDelete,
    sortFieldPref,
    sortDirectionPref,
    sortField,
    sortDirection,
    density,
    archiveName,
    archiveLevel,
    openDestAfterExtract,
    videoThumbs,
    hardwareAcceleration,
    ffmpegPath,
    thumbCacheMb,
    mountsPollMs,
    doubleClickMs,
    bookmarks,
    partitions,
    history,
    historyIndex,
  }
}

export type ExplorerStores = ReturnType<typeof createExplorerStores>
