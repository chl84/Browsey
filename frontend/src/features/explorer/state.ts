import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { derived, get, writable } from 'svelte/store'
import type {
  Column,
  Entry,
  Listing,
  Location,
  Partition,
  SortDirection,
  SortField,
  DefaultSortField,
  Density,
} from './types'
import { isUnderMount, normalizePath, parentPath } from './utils'
import { openEntry } from './services/files'
import { listDir, listRecent, listStarred, listTrash, watchDir, listMounts, searchStream } from './services/listing'
import { storeColumnWidths, loadSavedColumnWidths } from './services/layout'
import {
  loadShowHidden,
  storeShowHidden,
  loadHiddenFilesLast,
  storeHiddenFilesLast,
  loadFoldersFirst,
  storeFoldersFirst,
  loadStartDir,
  storeStartDir,
  loadConfirmDelete,
  storeConfirmDelete,
  loadSortField,
  storeSortField,
  loadSortDirection,
  storeSortDirection,
  loadArchiveName,
  storeArchiveName,
  loadArchiveLevel,
  storeArchiveLevel,
  loadOpenDestAfterExtract,
  storeOpenDestAfterExtract,
  loadDensity,
  storeDensity,
} from './services/settings'
import { toggleStar as toggleStarService } from './services/star'
import { getBookmarks } from './services/bookmarks'

const FILTER_DEBOUNCE_MS = 40

type ExplorerCallbacks = {
  onEntriesChanged?: () => void
  onCurrentChange?: (path: string) => void
}

const withNameLower = (entry: Entry): Entry => ({
  ...entry,
  nameLower: entry.nameLower ?? entry.name.toLowerCase(),
})

const mapNameLower = (list: Entry[]) => list.map(withNameLower)

const defaultColumns: Column[] = [
  { key: 'name', label: 'Name', sort: 'name', width: 320, min: 220, align: 'left' },
  { key: 'type', label: 'Type', sort: 'type', width: 120, min: 80 },
  { key: 'modified', label: 'Modified', sort: 'modified', width: 90, min: 80 },
  { key: 'size', label: 'Size', sort: 'size', width: 90, min: 70, align: 'right' },
  { key: 'star', label: '', sort: 'starred', width: 25, min: 25, resizable: false, sortable: false },
]

const sameLocation = (a?: Location, b?: Location) => {
  if (!a || !b) return false
  if (a.type !== b.type) return false
  if (a.type === 'dir' && b.type === 'dir') {
    return a.path === b.path
  }
  return true
}

export const createExplorerState = (callbacks: ExplorerCallbacks = {}) => {
  const cols = writable<Column[]>(defaultColumns)
  const gridTemplate = derived(cols, ($cols) => $cols.map((c) => `${Math.max(c.width, c.min)}px`).join(' '))

  const current = writable('')
  const entries = writable<Entry[]>([])
  const loading = writable(false)
  const error = writable('')
  const filter = writable('')
  const searchMode = writable(false)
  const searchActive = writable(false)
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
  const bookmarks = writable<{ label: string; path: string }[]>([])
  const partitions = writable<Partition[]>([])
  const history = writable<Location[]>([])
  const historyIndex = writable(-1)

  const applyFoldersFirst = (list: Entry[], foldersFirstOn: boolean) => {
    if (!foldersFirstOn) return list
    const dirs: Entry[] = []
    const rest: Entry[] = []
    for (const e of list) {
      if (e.kind === 'dir') {
        dirs.push(e)
      } else {
        rest.push(e)
      }
    }
    return [...dirs, ...rest]
  }

  const visibleEntries = derived(
    [entries, showHidden, hiddenFilesLast, foldersFirst],
    ([$entries, $showHidden, $hiddenLast, $foldersFirst]) => {
      const base = $showHidden
        ? $entries
        : $entries.filter((e) => !(e.hidden === true || e.name.startsWith('.')))
      if ($showHidden && $hiddenLast) {
        const hiddenList: Entry[] = []
        const visibleList: Entry[] = []
        for (const e of base) {
          if (e.hidden === true || e.name.startsWith('.')) {
            hiddenList.push(e)
          } else {
            visibleList.push(e)
          }
        }
        return [
          ...applyFoldersFirst(visibleList, $foldersFirst),
          ...applyFoldersFirst(hiddenList, $foldersFirst),
        ]
      }
      return applyFoldersFirst(base, $foldersFirst)
    },
  )

  let lastNeedle = ''
  let lastResult: Entry[] = []
  let lastVisibleRef: Entry[] = []

  const filteredEntries = derived([visibleEntries, filter], ([$visible, $filter], set) => {
    let timer: ReturnType<typeof setTimeout> | undefined
    const needle = $filter.trim().toLowerCase()

    const compute = () => {
      if (needle.length === 0) {
        lastNeedle = ''
        lastVisibleRef = $visible
        lastResult = $visible
        set($visible)
        return
      }

      if (needle === lastNeedle && $visible === lastVisibleRef) {
        set(lastResult)
        return
      }

      const result = $visible.filter((e) => (e.nameLower ?? e.name.toLowerCase()).includes(needle))
      lastNeedle = needle
      lastVisibleRef = $visible
      lastResult = result
      set(result)
    }

    const shouldDebounce = needle.length > 0 && needle !== lastNeedle
    if (shouldDebounce) {
      timer = setTimeout(compute, FILTER_DEBOUNCE_MS)
    } else {
      compute()
    }

    return () => {
      if (timer) {
        clearTimeout(timer)
      }
    }
  }, [] as Entry[])

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
    error.set('')
    searchActive.set(false)
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
      error.set(err instanceof Error ? err.message : String(err))
    } finally {
      if (!silent) {
        loading.set(false)
      }
    }
  }

  const loadRecent = async (recordHistory = true, applySort = false) => {
    loading.set(true)
    error.set('')
    searchActive.set(false)
    try {
      const sortArg = applySort ? sortPayload() : null
      const result = await listRecent(sortArg)
      current.set('Recent')
      entries.set(mapNameLower(result))
      callbacks.onEntriesChanged?.()
      callbacks.onCurrentChange?.('Recent')
      if (recordHistory) {
        pushHistory({ type: 'recent' })
      }
    } catch (err) {
      error.set(err instanceof Error ? err.message : String(err))
    } finally {
      loading.set(false)
    }
  }

  const loadStarred = async (recordHistory = true) => {
    loading.set(true)
    error.set('')
    searchActive.set(false)
    try {
      const result = await listStarred(sortPayload())
      current.set('Starred')
      entries.set(mapNameLower(result))
      callbacks.onEntriesChanged?.()
      callbacks.onCurrentChange?.('Starred')
      if (recordHistory) {
        pushHistory({ type: 'starred' })
      }
    } catch (err) {
      error.set(err instanceof Error ? err.message : String(err))
    } finally {
      loading.set(false)
    }
  }

  const loadTrash = async (recordHistory = true) => {
    loading.set(true)
    error.set('')
    searchActive.set(false)
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
      error.set(err instanceof Error ? err.message : String(err))
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

  const setSortFieldPref = async (field: DefaultSortField) => {
    if (get(sortField) === field) return
    sortField.set(field)
    sortFieldPref.set(field)
    void storeSortField(field)
    await refreshForSort()
  }

  const setSortDirectionPref = async (dir: SortDirection) => {
    if (get(sortDirection) === dir) return
    sortDirection.set(dir)
    sortDirectionPref.set(dir)
    void storeSortDirection(dir)
    await refreshForSort()
  }

  const setArchiveNamePref = (value: string) => {
    const trimmed = value.trim().replace(/\.zip$/i, '')
    if (trimmed.length === 0) {
      // Allow empty in UI; keep last persisted value until user provides one.
      archiveName.set('')
      return
    }
    archiveName.set(trimmed)
    void storeArchiveName(trimmed)
  }

  const setArchiveLevelPref = (value: number) => {
    const lvl = Math.min(Math.max(Math.round(value), 0), 9)
    archiveLevel.set(lvl)
    void storeArchiveLevel(lvl)
  }

  const refreshForSort = async () => {
    const isSearchActive = get(searchActive)
    const isSearchMode = get(searchMode)
    if (isSearchActive && isSearchMode) {
      const list = [...get(entries)]
      const spec = sortPayload()
      const dir = spec.direction === 'asc' ? 1 : -1
      const kindRank = (k: string) => (k === 'dir' ? 0 : k === 'file' ? 1 : 2)
      list.sort((a, b) => {
        const cmp = (() => {
          switch (spec.field) {
            case 'name':
              return a.name.localeCompare(b.name, undefined, { sensitivity: 'base' })
            case 'type':
              if (a.kind !== b.kind) return kindRank(a.kind) - kindRank(b.kind)
              return (a.ext ?? '').localeCompare(b.ext ?? '', undefined, { sensitivity: 'base' })
            case 'modified':
              return (a.modified ?? '').localeCompare(b.modified ?? '')
            case 'size':
              return (a.size ?? 0) - (b.size ?? 0)
            case 'starred':
              return Number(b.starred ?? false) - Number(a.starred ?? false)
            default:
              return 0
          }
        })()
        return dir * cmp
      })
      entries.set(mapNameLower(list))
      return
    }
    const where = get(current)
    if (where === 'Recent') {
      await loadRecent(false, true)
    } else if (where === 'Starred') {
      await loadStarred(false)
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
      error.set(err instanceof Error ? err.message : String(err))
    }
  }

  const toggleShowHidden = () => {
    showHidden.update((v) => {
      const next = !v
      void storeShowHidden(next)
      return next
    })
  }

  const toggleHiddenFilesLast = () => {
    hiddenFilesLast.update((v) => {
      const next = !v
      void storeHiddenFilesLast(next)
      return next
    })
  }

  const toggleFoldersFirst = () => {
    foldersFirst.update((v) => {
      const next = !v
      void storeFoldersFirst(next)
      return next
    })
  }

  const goUp = () => {
    searchActive.set(false)
    return load(parentPath(get(current)))
  }

  const goHome = () => {
    searchActive.set(false)
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

  const handlePlace = (label: string, path: string) => {
    if (label === 'Recent') {
      void loadRecent()
      return
    }
    if (label === 'Starred') {
      void loadStarred()
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
    error.set('')
    if (needle.length === 0) {
      searchActive.set(false)
      await load(get(current), { recordHistory: false })
      loading.set(false)
      return
    }
    filter.set(needle)
    searchActive.set(true)
    entries.set([])
    callbacks.onEntriesChanged?.()

    const stop = await listen<{ entries: Entry[]; done: boolean; error?: string }>(progressEvent, (evt) => {
      if (evt.payload.error) {
        error.set(evt.payload.error)
      }
      if (evt.payload.done) {
        entries.set(mapNameLower(evt.payload.entries ?? []))
        callbacks.onEntriesChanged?.()
        loading.set(false)
        return
      }
      if (evt.payload.entries && evt.payload.entries.length > 0) {
        entries.update((list) => [...list, ...mapNameLower(evt.payload.entries ?? [])])
        callbacks.onEntriesChanged?.()
      }
    })

    searchStream({
      path: get(current),
      query: needle,
      sort: sortPayload(),
      progressEvent,
    }).catch((err) => {
      error.set(err instanceof Error ? err.message : String(err))
      loading.set(false)
      void stop()
    })

    return async () => {
      await stop()
    }
  }

  const toggleMode = async (checked: boolean) => {
    if (get(searchMode) === checked) {
      return
    }
    searchMode.set(checked)
    if (!checked) {
      filter.set('')
      searchActive.set(false)
      const curr = get(current)
      if (curr === 'Recent') {
        await loadRecent(false)
      } else if (curr === 'Starred') {
        await loadStarred(false)
      } else if (curr.startsWith('Trash')) {
        await loadTrash(false)
      } else {
        await load(curr, { recordHistory: false })
      }
    }
  }

  let lastMountPaths: string[] = []
  const loadPartitions = async () => {
    try {
      const result = await listMounts()
      partitions.set(result)
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

  const loadShowHiddenPref = async () => {
    try {
      const saved = await loadShowHidden()
      if (typeof saved === 'boolean') {
        showHidden.set(saved)
      }
    } catch (err) {
      console.error('Failed to load showHidden setting', err)
    }
  }

  const loadHiddenFilesLastPref = async () => {
    try {
      const saved = await loadHiddenFilesLast()
      if (typeof saved === 'boolean') {
        hiddenFilesLast.set(saved)
      }
    } catch (err) {
      console.error('Failed to load hiddenFilesLast setting', err)
    }
  }

  const loadStartDirPref = async () => {
    try {
      const saved = await loadStartDir()
      if (typeof saved === 'string' && saved.trim().length > 0) {
        startDirPref.set(saved)
      }
    } catch (err) {
      console.error('Failed to load startDir setting', err)
    }
  }

  const setStartDirPref = (value: string) => {
    const next = value.trim()
    startDirPref.set(next || null)
    void storeStartDir(next)
  }

  const loadConfirmDeletePref = async () => {
    try {
      const saved = await loadConfirmDelete()
      if (typeof saved === 'boolean') {
        confirmDelete.set(saved)
      }
    } catch (err) {
      console.error('Failed to load confirmDelete setting', err)
    }
  }

  const toggleConfirmDelete = () => {
    confirmDelete.update((v) => {
      const next = !v
      void storeConfirmDelete(next)
      return next
    })
  }

  const loadSortPref = async () => {
    try {
      const savedField = await loadSortField()
      if (savedField === 'name' || savedField === 'type' || savedField === 'modified' || savedField === 'size') {
        sortField.set(savedField)
        sortFieldPref.set(savedField)
      } else if (savedField !== null) {
        await storeSortField('name')
        sortField.set('name')
        sortFieldPref.set('name')
      }
      const savedDir = await loadSortDirection()
      if (savedDir === 'asc' || savedDir === 'desc') {
        sortDirection.set(savedDir)
        sortDirectionPref.set(savedDir)
      }
    } catch (err) {
      console.error('Failed to load sort settings', err)
    }
  }

  const loadArchiveNamePref = async () => {
    try {
      const saved = await loadArchiveName()
      if (typeof saved === 'string' && saved.trim().length > 0) {
        archiveName.set(saved.trim())
      }
    } catch (err) {
      console.error('Failed to load archiveName setting', err)
    }
  }

  const loadArchiveLevelPref = async () => {
    try {
      const saved = await loadArchiveLevel()
      if (typeof saved === 'number' && saved >= 0 && saved <= 9) {
        archiveLevel.set(saved)
      }
    } catch (err) {
      console.error('Failed to load archiveLevel setting', err)
    }
  }

  const loadOpenDestAfterExtractPref = async () => {
    try {
      const saved = await loadOpenDestAfterExtract()
      if (typeof saved === 'boolean') {
        openDestAfterExtract.set(saved)
      }
    } catch (err) {
      console.error('Failed to load openDestAfterExtract setting', err)
    }
  }

  const loadDensityPref = async () => {
    try {
      const saved = await loadDensity()
      if (saved === 'cozy' || saved === 'compact') {
        density.set(saved)
      }
    } catch (err) {
      console.error('Failed to load density setting', err)
    }
  }

  const setDensityPref = (value: Density) => {
    density.set(value)
    void storeDensity(value)
  }

  const toggleOpenDestAfterExtract = () => {
    openDestAfterExtract.update((v) => {
      const next = !v
      void storeOpenDestAfterExtract(next)
      return next
    })
  }

  const loadFoldersFirstPref = async () => {
    try {
      const saved = await loadFoldersFirst()
      if (typeof saved === 'boolean') {
        foldersFirst.set(saved)
      }
    } catch (err) {
      console.error('Failed to load foldersFirst setting', err)
    }
  }

  return {
    cols,
    gridTemplate,
    current,
    entries,
    loading,
    error,
    filter,
    searchMode,
    searchActive,
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
    bookmarks,
    partitions,
    showHidden,
    visibleEntries,
    filteredEntries,
    density,
    load,
    loadRecent,
    loadStarred,
    loadTrash,
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
  }
}
