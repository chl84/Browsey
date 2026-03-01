import { derived, writable, type Writable } from 'svelte/store'
import type { DefaultSortField, Density, Entry, Location, Partition, SortDirection, SortField } from '../model/types'
import { defaultColumns } from './helpers'

const NAV_LOADING_DELAY_MS = 150
const NAV_LOADING_MIN_VISIBLE_MS = 180

const createDelayedLoadingStore = (): Writable<boolean> => {
  const visible = writable(false)
  let requested = false
  let visibleNow = false
  let visibleSince = 0
  let showTimer: ReturnType<typeof setTimeout> | null = null
  let hideTimer: ReturnType<typeof setTimeout> | null = null

  const clearShowTimer = () => {
    if (showTimer) {
      clearTimeout(showTimer)
      showTimer = null
    }
  }

  const clearHideTimer = () => {
    if (hideTimer) {
      clearTimeout(hideTimer)
      hideTimer = null
    }
  }

  const setVisible = (next: boolean) => {
    visibleNow = next
    if (next) {
      visibleSince = Date.now()
    } else {
      visibleSince = 0
    }
    visible.set(next)
  }

  const store: Writable<boolean> = {
    subscribe: visible.subscribe,
    set(next) {
      requested = next
      if (next) {
        clearHideTimer()
        if (visibleNow || showTimer) {
          return
        }
        showTimer = setTimeout(() => {
          showTimer = null
          if (!requested || visibleNow) {
            return
          }
          setVisible(true)
        }, NAV_LOADING_DELAY_MS)
        return
      }

      clearShowTimer()
      if (!visibleNow) {
        return
      }
      const elapsed = Date.now() - visibleSince
      const remaining = Math.max(0, NAV_LOADING_MIN_VISIBLE_MS - elapsed)
      clearHideTimer()
      if (remaining === 0) {
        setVisible(false)
        return
      }
      hideTimer = setTimeout(() => {
        hideTimer = null
        if (requested) {
          return
        }
        setVisible(false)
      }, remaining)
    },
    update(updater) {
      store.set(updater(visibleNow))
    },
  }
  return store
}

export const createExplorerStores = () => {
  const cols = writable(defaultColumns)
  const gridTemplate = derived(cols, ($cols) => $cols.map((c) => `${Math.max(c.width, c.min)}px`).join(' '))

  const current = writable('')
  const entries = writable<Entry[]>([])
  const loading = createDelayedLoadingStore()
  const error = writable('')
  const filter = writable('')
  const searchMode = writable(false)
  const searchRunning = writable(false)
  const showHidden = writable(true)
  const hiddenFilesLast = writable(false)
  const highContrast = writable(false)
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
  const hardwareAcceleration = writable<boolean>(false)
  const ffmpegPath = writable<string>('')
  const thumbCacheMb = writable<number>(300)
  const mountsPollMs = writable<number>(8000)
  const doubleClickMs = writable<number>(300)
  const scrollbarWidth = writable<number>(10)
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
    highContrast,
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
    scrollbarWidth,
    bookmarks,
    partitions,
    history,
    historyIndex,
  }
}

export type ExplorerStores = ReturnType<typeof createExplorerStores>
