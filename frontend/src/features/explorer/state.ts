import { getErrorMessage } from '@/shared/lib/error'
import { listen } from '@tauri-apps/api/event'
import { get } from 'svelte/store'
import type { Entry, ListingFacets, Location, SortDirection, SortField } from './model/types'
import { isUnderMount, normalizePath, parentPath } from './utils'
import { openEntry } from './services/files.service'
import {
  listDir,
  listRecent,
  listStarred,
  listTrash,
  watchDir,
  listMounts,
  searchStream,
} from './services/listing.service'
import { cancelTask } from './services/activity.service'
import { storeColumnWidths, loadSavedColumnWidths } from './services/layout.service'
import { toggleStar as toggleStarService } from './services/star.service'
import { getBookmarks } from './services/bookmarks.service'
import { listNetworkEntries } from '../network'
import { emptyListingFacets, mapNameLower, sameLocation } from './state/helpers'
import type { ExplorerCallbacks } from './state/helpers'
import { createFilteringSlice } from './state/filteringSlice'
import { createPreferenceSlice } from './state/preferencesSlice'
import { createExplorerStores } from './state/stores'

export const createExplorerState = (callbacks: ExplorerCallbacks = {}) => {
  const {
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
  } = createExplorerStores()

  const {
    visibleEntries,
    filteredEntries,
    columnFilters,
    columnFacets,
    columnFacetsLoading,
    invalidateFacetCache,
    clearFacetCache,
    ensureColumnFacets,
    resetColumnFilter,
    toggleColumnFilter,
  } = createFilteringSlice({
    entries,
    showHidden,
    hiddenFilesLast,
    foldersFirst,
    filter,
    searchMode,
    current,
  })

  // Search streaming coordination
  let searchRunId = 0
  let cancelActiveSearch: (() => void) | null = null
  let activeSearchCancelId: string | null = null
  const invalidateSearchRun = () => {
    const cancelId = activeSearchCancelId
    activeSearchCancelId = null
    if (cancelId) {
      void cancelTask(cancelId).catch(() => {})
    }
    searchRunId += 1
    cancelActiveSearch?.()
    cancelActiveSearch = null
  }

  const sortPayload = () => ({
    field: get(sortField),
    direction: get(sortDirection),
  })

  const pushHistory = (loc: Location) => {
    const list = get(history)
    const idx = get(historyIndex)
    const last = list[idx]
    if (sameLocation(last, loc)) return
    const next = [...list.slice(0, idx + 1), loc]
    history.set(next)
    historyIndex.set(next.length - 1)
  }

  const load = async (path?: string, opts: { recordHistory?: boolean; silent?: boolean } = {}) => {
    const { recordHistory = true, silent = false } = opts
    if (!silent) {
      loading.set(true)
    }
    clearFacetCache()
    error.set('')
    invalidateSearchRun()
    searchRunning.set(false)
    try {
      const result = await listDir(path, sortPayload())
      current.set(result.current)
      entries.set(mapNameLower(result.entries))
      callbacks.onEntriesChanged?.()
      callbacks.onCurrentChange?.(result.current)
      if (recordHistory) {
        pushHistory({ type: 'dir', path: result.current })
      }
      await watchDir(result.current)
    } catch (err) {
      error.set(getErrorMessage(err))
    } finally {
      if (!silent) {
        loading.set(false)
      }
    }
  }

  const loadRecent = async (recordHistory = true, applySort = false) => {
    loading.set(true)
    clearFacetCache()
    error.set('')
    invalidateSearchRun()
    searchRunning.set(false)
    try {
      const sortArg = applySort ? sortPayload() : null
      const result = await listRecent(sortArg)
      current.set(result.current)
      entries.set(mapNameLower(result.entries))
      callbacks.onEntriesChanged?.()
      callbacks.onCurrentChange?.(result.current)
      if (recordHistory) {
        pushHistory({ type: 'recent' })
      }
    } catch (err) {
      error.set(getErrorMessage(err))
    } finally {
      loading.set(false)
    }
  }

  const loadStarred = async (recordHistory = true) => {
    loading.set(true)
    clearFacetCache()
    error.set('')
    invalidateSearchRun()
    searchRunning.set(false)
    try {
      const result = await listStarred(sortPayload())
      current.set(result.current)
      entries.set(mapNameLower(result.entries))
      callbacks.onEntriesChanged?.()
      callbacks.onCurrentChange?.(result.current)
      if (recordHistory) {
        pushHistory({ type: 'starred' })
      }
    } catch (err) {
      error.set(getErrorMessage(err))
    } finally {
      loading.set(false)
    }
  }

  const loadNetwork = async (
    recordHistory = true,
    options: { forceRefresh?: boolean } = {},
  ) => {
    loading.set(true)
    clearFacetCache()
    error.set('')
    invalidateSearchRun()
    searchRunning.set(false)
    try {
      const mounts = await listMounts()
      partitions.set(mounts)
      const networkEntries = await listNetworkEntries(options.forceRefresh === true)
      current.set('Network')
      entries.set(sortSearchEntries(networkEntries, sortPayload()))
      callbacks.onEntriesChanged?.()
      callbacks.onCurrentChange?.('Network')
      if (recordHistory) {
        pushHistory({ type: 'network' })
      }
    } catch (err) {
      error.set(getErrorMessage(err))
    } finally {
      loading.set(false)
    }
  }

  const loadTrash = async (recordHistory = true) => {
    loading.set(true)
    clearFacetCache()
    error.set('')
    invalidateSearchRun()
    searchRunning.set(false)
    try {
      const result = await listTrash(sortPayload())
      current.set('Trash')
      entries.set(mapNameLower(result.entries))
      callbacks.onEntriesChanged?.()
      callbacks.onCurrentChange?.('Wastebasket')
      if (recordHistory) {
        pushHistory({ type: 'trash' })
      }
    } catch (err) {
      error.set(getErrorMessage(err))
    } finally {
      loading.set(false)
    }
  }

  const navigateTo = async (loc: Location, recordHistory = false) => {
    switch (loc.type) {
      case 'dir':
        await load(loc.path, { recordHistory })
        break
      case 'recent':
        await loadRecent(recordHistory)
        break
      case 'starred':
        await loadStarred(recordHistory)
        break
      case 'network':
        await loadNetwork(recordHistory)
        break
      case 'trash':
        await loadTrash(recordHistory)
        break
    }
  }

  const goBack = async () => {
    const idx = get(historyIndex)
    if (idx <= 0) return
    historyIndex.set(idx - 1)
    const loc = get(history)[idx - 1]
    await navigateTo(loc, false)
  }

  const goForward = async () => {
    const list = get(history)
    const idx = get(historyIndex)
    if (idx < 0 || idx >= list.length - 1) return
    historyIndex.set(idx + 1)
    await navigateTo(list[idx + 1], false)
  }

  const changeSort = async (field: SortField) => {
    const currentField = get(sortField)
    const currentDir = get(sortDirection)
    if (currentField === field) {
      sortDirection.set(currentDir === 'asc' ? 'desc' : 'asc')
    } else {
      sortField.set(field)
      sortDirection.set('asc')
    }
    await refreshForSort()
  }

  const sortSearchEntries = (
    list: Entry[],
    spec: { field: SortField; direction: SortDirection } = sortPayload(),
  ) => {
    const dir = spec.direction === 'asc' ? 1 : -1
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
    ) => {
      const rankCmp = a.kindRank - b.kindRank
      if (rankCmp !== 0) return rankCmp
      const numericCmp = compareOptionalNumber(a.numeric, b.numeric)
      if (numericCmp !== 0) return dir * numericCmp
      return dir * compareString(a.nameKey, b.nameKey)
    }
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
            return compareSizeField(a.sizeKey, b.sizeKey)
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

  const refreshForSort = async () => {
    const isSearchMode = get(searchMode)
    const hasSearchQuery = get(filter).trim().length > 0
    if (isSearchMode && hasSearchQuery) {
      entries.set(sortSearchEntries(get(entries), sortPayload()))
      return
    }
    const where = get(current)
    if (where === 'Recent') {
      await loadRecent(false, true)
    } else if (where === 'Starred') {
      await loadStarred(false)
    } else if (where === 'Network') {
      await loadNetwork(false)
    } else if (where === 'Trash') {
      await loadTrash(false)
    } else {
      await load(where, { recordHistory: false })
    }
  }

  const open = (entry: Entry) => {
    if (entry.kind === 'dir') {
      void load(entry.path)
    } else {
      void openEntry(entry)
    }
  }

  const toggleStar = async (entry: Entry) => {
    try {
      const newState = await toggleStarService(entry.path)
      const where = get(current)
      if (where === 'Starred' && !newState) {
        entries.update((list) => list.filter((e) => e.path !== entry.path))
      } else {
        entries.update((list) =>
          list.map((e) => (e.path === entry.path ? { ...e, starred: newState } : e))
        )
      }
    } catch (err) {
      error.set(getErrorMessage(err))
    }
  }

  const goUp = () => {
    searchRunning.set(false)
    return load(parentPath(get(current)))
  }

  const goHome = () => {
    searchRunning.set(false)
    const pref = (get(startDirPref) ?? '').trim()
    return load(pref || undefined)
  }

  const goToPath = (path: string) => {
    const trimmed = path.trim()
    if (!trimmed) return
    if (trimmed !== get(current)) {
      void load(trimmed)
    }
  }

  const cancelSearch = () => {
    const hadActiveSearch = activeSearchCancelId !== null || get(searchRunning)
    invalidateSearchRun()
    searchRunning.set(false)
    if (hadActiveSearch) {
      loading.set(false)
    }
  }

  const handlePlace = (label: string, path: string) => {
    if (label === 'Recent') {
      void loadRecent()
      return
    }
    if (label === 'Starred') {
      void loadStarred()
      return
    }
    if (label === 'Network') {
      void loadNetwork()
      return
    }
    if (label === 'Wastebasket') {
      void loadTrash()
      return
    }
    if (path) {
      void load(path)
    }
  }

  const runSearch = async (needleRaw: string) => {
    const needle = needleRaw.trim()
    const progressEvent = `search-progress-${Math.random().toString(16).slice(2)}`
    loading.set(true)
    clearFacetCache()
    error.set('')

    invalidateSearchRun()
    const runId = searchRunId

    let buffer: Entry[] = []
    let raf: number | null = null
    let stop: (() => void) | null = null
    let cleaned = false

    const cleanup = () => {
      if (cleaned) return
      cleaned = true
      if (raf !== null) {
        cancelAnimationFrame(raf)
        raf = null
      }
      buffer = []
      if (stop) {
        stop()
        stop = null
      }
      if (cancelActiveSearch === cleanup) {
        cancelActiveSearch = null
      }
      if (activeSearchCancelId === progressEvent) {
        activeSearchCancelId = null
      }
    }

    cancelActiveSearch = cleanup
    activeSearchCancelId = progressEvent

    if (needle.length === 0) {
      searchRunning.set(false)
      await load(get(current), { recordHistory: false })
      if (runId === searchRunId) {
        loading.set(false)
      }
      cleanup()
      return cleanup
    }
    filter.set(needle)
    searchRunning.set(true)
    entries.set([])
    callbacks.onEntriesChanged?.()

    const flushBuffer = () => {
      if (runId !== searchRunId) {
        cleanup()
        return
      }
      if (buffer.length === 0) {
        raf = null
        return
      }
      entries.update((list) => [...list, ...buffer])
      callbacks.onEntriesChanged?.()
      buffer = []
      raf = null
    }

    const scheduleFlush = () => {
      if (raf !== null) return
      raf = requestAnimationFrame(flushBuffer)
    }

    let unlisten: () => void
    try {
      unlisten = await listen<{
        entries: Entry[]
        done: boolean
        error?: string
        facets?: ListingFacets
      }>(progressEvent, (evt) => {
        if (runId !== searchRunId) {
          cleanup()
          return
        }
        if (evt.payload.error) {
          error.set(evt.payload.error)
        }
        if (evt.payload.done) {
          if (raf !== null) {
            cancelAnimationFrame(raf)
            raf = null
          }
          const doneEntries =
            evt.payload.entries && evt.payload.entries.length > 0
              ? mapNameLower(evt.payload.entries)
              : null
          const finalEntries = doneEntries ?? [...get(entries), ...buffer]
          buffer = []
          // Keep streamed order on completion; sorting remains a manual UI action.
          entries.set(finalEntries)
          if (evt.payload.facets) {
            columnFacets.set(evt.payload.facets)
          }
          callbacks.onEntriesChanged?.()
          searchRunning.set(false)
          loading.set(false)
          cleanup()
          return
        }
        if (evt.payload.entries && evt.payload.entries.length > 0) {
          buffer.push(...mapNameLower(evt.payload.entries))
          scheduleFlush()
        }
      })
    } catch (err) {
      if (runId === searchRunId) {
        error.set(getErrorMessage(err))
        searchRunning.set(false)
        loading.set(false)
        columnFacets.set(emptyListingFacets())
      }
      cleanup()
      return cleanup
    }
    stop = unlisten
    if (cleaned) {
      stop()
      stop = null
      return cleanup
    }

    searchStream({
      path: get(current),
      query: needle,
      sort: sortPayload(),
      progressEvent,
    }).catch((err) => {
      if (runId !== searchRunId) {
        cleanup()
        return
      }
      error.set(getErrorMessage(err))
      searchRunning.set(false)
      loading.set(false)
      columnFacets.set(emptyListingFacets())
      cleanup()
    })

    return cleanup
  }

  const toggleMode = async (
    checked: boolean,
    options: {
      reloadOnDisable?: boolean
    } = {},
  ) => {
    const { reloadOnDisable = true } = options
    if (get(searchMode) === checked) {
      return
    }
    searchMode.set(checked)
    if (!checked) {
      cancelSearch()
      filter.set('')
      if (!reloadOnDisable) {
        return
      }
      const curr = get(current)
      if (curr === 'Recent') {
        await loadRecent(false)
      } else if (curr === 'Starred') {
        await loadStarred(false)
      } else if (curr === 'Network') {
        await loadNetwork(false)
      } else if (curr.startsWith('Trash')) {
        await loadTrash(false)
      } else {
        await load(curr, { recordHistory: false })
      }
    }
  }

  let lastMountPaths: string[] = []
  const loadPartitions = async (options: { forceNetworkRefresh?: boolean } = {}) => {
    const { forceNetworkRefresh = false } = options
    try {
      const result = await listMounts()
      partitions.set(result)
      if (get(current) === 'Network') {
        try {
          const networkEntries = await listNetworkEntries(forceNetworkRefresh)
          entries.set(sortSearchEntries(networkEntries, sortPayload()))
          callbacks.onEntriesChanged?.()
        } catch (err) {
          console.error('Failed to list network entries', err)
        }
      }
      const nextPaths = result.map((p) => normalizePath(p.path))
      const removedMount = lastMountPaths.find((p) => !nextPaths.includes(p))
      lastMountPaths = nextPaths

      if (removedMount && isUnderMount(get(current), removedMount)) {
        error.set('Volume disconnected; returning to Home')
        void load(undefined)
      }
    } catch (err) {
      console.error('Failed to load mounts', err)
    }
  }

  const loadBookmarks = async () => {
    try {
      const rows = await getBookmarks()
      bookmarks.set(rows)
    } catch (err) {
      console.error('Failed to load bookmarks', err)
    }
  }

  const persistWidths = async () => {
    try {
      await storeColumnWidths(get(cols).map((c) => c.width))
    } catch (err) {
      console.error('Failed to store widths', err)
    }
  }

  const loadSavedWidths = async () => {
    try {
      const saved = await loadSavedColumnWidths()
      if (saved && Array.isArray(saved)) {
        cols.update((list) =>
          list.map((c, i) => {
            if (c.resizable === false) {
              return { ...c, width: Math.max(c.min, c.width) }
            }
            return saved[i] !== undefined ? { ...c, width: Math.max(c.min, saved[i]) } : c
          })
        )
      }
    } catch (err) {
      console.error('Failed to load widths', err)
    }
  }

  const {
    setSortFieldPref,
    setSortDirectionPref,
    setArchiveNamePref,
    setArchiveLevelPref,
    toggleShowHidden,
    toggleHiddenFilesLast,
    toggleFoldersFirst,
    setStartDirPref,
    loadShowHiddenPref,
    loadHiddenFilesLastPref,
    loadStartDirPref,
    loadConfirmDeletePref,
    toggleConfirmDelete,
    loadSortPref,
    loadArchiveNamePref,
    loadArchiveLevelPref,
    loadOpenDestAfterExtractPref,
    loadVideoThumbsPref,
    loadHardwareAccelerationPref,
    loadFfmpegPathPref,
    loadThumbCachePref,
    loadDensityPref,
    setDensityPref,
    toggleOpenDestAfterExtract,
    toggleVideoThumbs,
    setHardwareAccelerationPref,
    setFfmpegPathPref,
    setThumbCachePref,
    setMountsPollPref,
    setDoubleClickMsPref,
    loadMountsPollPref,
    loadDoubleClickMsPref,
    loadFoldersFirstPref,
  } = createPreferenceSlice(
    {
      showHidden,
      hiddenFilesLast,
      foldersFirst,
      startDirPref,
      confirmDelete,
      sortField,
      sortDirection,
      sortFieldPref,
      sortDirectionPref,
      archiveName,
      archiveLevel,
      openDestAfterExtract,
      videoThumbs,
      hardwareAcceleration,
      ffmpegPath,
      thumbCacheMb,
      mountsPollMs,
      doubleClickMs,
      density,
    },
    {
      clearFacetCache,
      refreshForSort,
    },
  )

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
    hiddenFilesLast,
    foldersFirst,
    confirmDelete,
    startDirPref,
    sortField,
    sortDirection,
    sortFieldPref,
    sortDirectionPref,
    archiveName,
    archiveLevel,
    openDestAfterExtract,
    videoThumbs,
    hardwareAcceleration,
    ffmpegPath,
    thumbCacheMb,
    doubleClickMs,
    bookmarks,
    partitions,
    showHidden,
    columnFilters,
    columnFacets,
    columnFacetsLoading,
    invalidateFacetCache,
    visibleEntries,
    filteredEntries,
    density,
    load,
    mountsPollMs,
    loadRecent,
    loadStarred,
    loadNetwork,
    loadTrash,
    cancelSearch,
    runSearch,
    toggleMode,
    changeSort,
    toggleShowHidden,
    toggleHiddenFilesLast,
    toggleFoldersFirst,
    toggleConfirmDelete,
    setStartDirPref,
    refreshForSort,
    open,
    toggleStar,
    handlePlace,
    goUp,
    goHome,
    goToPath,
    goBack,
    goForward,
    loadBookmarks,
    loadPartitions,
    loadShowHiddenPref,
    loadHiddenFilesLastPref,
    loadStartDirPref,
    loadConfirmDeletePref,
    loadSortPref,
    loadArchiveNamePref,
    loadArchiveLevelPref,
    loadOpenDestAfterExtractPref,
    loadVideoThumbsPref,
    loadHardwareAccelerationPref,
    loadFfmpegPathPref,
    loadThumbCachePref,
    loadFoldersFirstPref,
    loadDensityPref,
    loadSavedWidths,
    persistWidths,
    setSortFieldPref,
    setSortDirectionPref,
    setDensityPref,
    setArchiveNamePref,
    setArchiveLevelPref,
    toggleOpenDestAfterExtract,
    toggleVideoThumbs,
    setHardwareAccelerationPref,
    setFfmpegPathPref,
    setThumbCachePref,
    setDoubleClickMsPref,
    setMountsPollPref,
    loadDoubleClickMsPref,
    loadMountsPollPref,
    toggleColumnFilter,
    resetColumnFilter,
    ensureColumnFacets,
  }
}
