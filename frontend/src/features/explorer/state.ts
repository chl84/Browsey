import { getErrorMessage } from '@/shared/lib/error'
import { get } from 'svelte/store'
import type { Entry, Location, SortField } from './model/types'
import { isUnderMount, normalizePath, parentPath } from './utils'
import { openEntry } from './services/files.service'
import {
  listDir,
  listRecent,
  listStarred,
  listTrash,
  watchDir,
  listMounts,
} from './services/listing.service'
import { cancelTask } from './services/activity.service'
import { storeColumnWidths, loadSavedColumnWidths } from './services/layout.service'
import { toggleStar as toggleStarService } from './services/star.service'
import { getBookmarks } from './services/bookmarks.service'
import { listNetworkEntries } from '../network'
import { emptyListingFacets, mapNameLower, sameLocation } from './state/helpers'
import type { ExplorerCallbacks } from './state/helpers'
import { createSearchSession } from './state/createSearchSession'
import { createSortRefreshDispatcher } from './state/createSortRefreshDispatcher'
import { createFilteringSlice } from './state/filteringSlice'
import { patchEntryStarred, removeEntryByPath } from './state/entryMutations'
import { createPreferenceSlice } from './state/preferencesSlice'
import { sortExplorerEntriesInMemory } from './state/searchSort'
import { createExplorerStores } from './state/stores'

export const createExplorerState = (callbacks: ExplorerCallbacks = {}) => {
  // Store composition root; keep the returned object shape as the stable public API.
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
    bookmarks,
    partitions,
    history,
    historyIndex,
  } = createExplorerStores()

  // Filtering/faceting slice derived from base stores.
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

  // Search streaming coordination and cancellation guards.
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

  const isCloudDirectoryPath = (where: string) => where.startsWith('rclone://')

  // Navigation/history and listing load flows.
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
      entries.set(sortExplorerEntriesInMemory(networkEntries, sortPayload()))
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

  // Sort refresh routing across search/library/network/directory views.
  const refreshForSort = createSortRefreshDispatcher({
    hasActiveSearchSortTarget: () => get(searchMode) && get(filter).trim().length > 0,
    sortActiveSearchEntries: () => {
      entries.set(sortExplorerEntriesInMemory(get(entries), sortPayload()))
    },
    getCurrentWhere: () => get(current),
    loadRecentForSort: () => loadRecent(false, true),
    loadStarredForSort: () => loadStarred(false),
    loadNetworkForSort: () => loadNetwork(false),
    loadTrashForSort: () => loadTrash(false),
    loadDirectoryForSort: async (where) => {
      if (isCloudDirectoryPath(where)) {
        entries.set(sortExplorerEntriesInMemory(get(entries), sortPayload()))
        callbacks.onEntriesChanged?.()
        return
      }
      await load(where, { recordHistory: false })
    },
  })

  const open = (entry: Entry) => {
    if (entry.kind === 'dir') {
      void load(entry.path)
    } else {
      if (callbacks.onOpenEntry) {
        void callbacks.onOpenEntry(entry)
      } else {
        void openEntry(entry)
      }
    }
  }

  const toggleStar = async (entry: Entry) => {
    try {
      const newState = await toggleStarService(entry.path)
      const where = get(current)
      if (where === 'Starred' && !newState) {
        entries.update((list) => removeEntryByPath(list, entry.path))
      } else {
        entries.update((list) => patchEntryStarred(list, entry.path, newState))
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

  // Search mode and streaming orchestration (high-churn hotspot).
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

  const runSearch = createSearchSession({
    entries,
    loading,
    error,
    filter,
    searchRunning,
    columnFacets,
    current,
    clearFacetCache,
    emptyListingFacets,
    onEntriesChanged: callbacks.onEntriesChanged,
    getErrorMessage,
    loadCurrentForEmptyNeedle: () => load(get(current), { recordHistory: false }),
    sortPayload,
    invalidateSearchRun,
    getSearchRunId: () => searchRunId,
    getCancelActiveSearch: () => cancelActiveSearch,
    setCancelActiveSearch: (fn) => {
      cancelActiveSearch = fn
    },
    getActiveSearchCancelId: () => activeSearchCancelId,
    setActiveSearchCancelId: (id) => {
      activeSearchCancelId = id
    },
  })

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
          entries.set(sortExplorerEntriesInMemory(networkEntries, sortPayload()))
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

  // Preferences slice wiring and final state API assembly.
  const {
    setSortFieldPref,
    setSortDirectionPref,
    setArchiveNamePref,
    setArchiveLevelPref,
    toggleShowHidden,
    toggleHiddenFilesLast,
    toggleHighContrast,
    toggleFoldersFirst,
    setStartDirPref,
    loadShowHiddenPref,
    loadHiddenFilesLastPref,
    loadHighContrastPref,
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
      highContrast,
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
    highContrast,
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
    toggleHighContrast,
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
    loadHighContrastPref,
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
