import { tick } from 'svelte'
import { get, type Readable, type Writable } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import { connectNetworkUri, isMountUri } from '@/features/network'
import { entryKind } from '../services/files.service'
import { createSelectionMemory } from '../model/selectionMemory'
import type { Entry } from '../model/types'
import type { CurrentView } from './useContextActions'

type ViewMode = 'list' | 'grid'

type LoadOpts = {
  recordHistory?: boolean
  silent?: boolean
}

type PendingNav = {
  path: string
  opts?: LoadOpts
  gen: number
}

type Deps = {
  current: Readable<string>
  loading: Readable<boolean>
  filteredEntries: Readable<Entry[]>
  selected: Writable<Set<string>>
  anchorIndex: Writable<number | null>
  caretIndex: Writable<number | null>
  rowHeight: Readable<number>
  gridTotalHeight: Readable<number>
  getViewMode: () => ViewMode
  getRowsEl: () => HTMLDivElement | null
  getHeaderEl: () => HTMLDivElement | null
  getGridEl: () => HTMLDivElement | null
  getGridCols: () => number
  getGridRowHeight: () => number
  getGridGap: () => number
  resetScrollPosition: () => void
  loadRaw: (path?: string, opts?: LoadOpts) => Promise<void>
  loadRecentRaw: (recordHistory?: boolean, applySort?: boolean) => Promise<void>
  loadStarredRaw: (recordHistory?: boolean) => Promise<void>
  loadNetworkRaw: (recordHistory?: boolean, opts?: { forceRefresh?: boolean }) => Promise<void>
  loadTrashRaw: (recordHistory?: boolean) => Promise<void>
  goBackRaw: () => Promise<void>
  goForwardRaw: () => Promise<void>
  open: (entry: Entry) => void | Promise<void>
  loadPartitions: (opts?: { forceNetworkRefresh?: boolean }) => Promise<void>
  showToast: (msg: string, durationMs?: number) => void
  setPathInput: (value: string) => void
}

export const useExplorerNavigation = (deps: Deps) => {
  const selectionMemory = createSelectionMemory()

  let pendingNav: PendingNav | null = null
  let navGeneration = 0

  const viewFromPath = (value: string): CurrentView =>
    value === 'Recent'
      ? 'recent'
      : value === 'Starred'
        ? 'starred'
        : value === 'Network'
          ? 'network'
          : value.startsWith('Trash')
            ? 'trash'
            : 'dir'

  const clearSelection = () => {
    deps.selected.set(new Set())
    deps.anchorIndex.set(null)
    deps.caretIndex.set(null)
  }

  const captureSelectionSnapshot = () => {
    const curr = get(deps.current)
    if (!curr || viewFromPath(curr) !== 'dir') return
    selectionMemory.capture(
      curr,
      get(deps.filteredEntries),
      get(deps.selected),
      get(deps.anchorIndex),
      get(deps.caretIndex),
    )
  }

  const restoreSelectionForCurrent = () => {
    const curr = get(deps.current)
    const view = curr ? viewFromPath(curr) : 'dir'
    if (!curr || view !== 'dir') {
      clearSelection()
      return
    }
    const restored = selectionMemory.restore(curr, get(deps.filteredEntries))
    if (restored) {
      deps.selected.set(restored.selected)
      deps.anchorIndex.set(restored.anchorIndex)
      deps.caretIndex.set(restored.caretIndex)
    } else {
      clearSelection()
    }
  }

  const centerSelectionIfAny = async () => {
    const list = get(deps.filteredEntries)
    const sel = get(deps.selected)
    if (sel.size === 0 || list.length === 0) return
    const idx =
      get(deps.caretIndex) ??
      get(deps.anchorIndex) ??
      list.findIndex((entry) => sel.has(entry.path))
    if (idx === null || idx < 0) return

    await tick()
    requestAnimationFrame(() => {
      if (deps.getViewMode() === 'list') {
        const rowsEl = deps.getRowsEl()
        if (!rowsEl) return
        const headerH = deps.getHeaderEl()?.offsetHeight ?? 0
        const viewport = rowsEl.clientHeight - headerH
        const rowH = get(deps.rowHeight)
        const target = headerH + Math.max(0, idx * rowH - (viewport - rowH) / 2)
        const maxScroll = Math.max(0, rowsEl.scrollHeight - rowsEl.clientHeight)
        rowsEl.scrollTo({ top: Math.min(target, maxScroll), behavior: 'auto' })
      } else {
        const gridEl = deps.getGridEl()
        const cols = deps.getGridCols()
        if (!gridEl || cols <= 0) return
        const stride = deps.getGridRowHeight() + deps.getGridGap()
        const row = Math.floor(idx / Math.max(1, cols))
        const viewport = gridEl.clientHeight
        const target = Math.max(0, row * stride - (viewport - stride) / 2)
        const maxScroll = Math.max(
          get(deps.gridTotalHeight) - viewport,
          gridEl.scrollHeight - gridEl.clientHeight,
          0,
        )
        gridEl.scrollTo({ top: Math.min(target, maxScroll), behavior: 'auto' })
      }
    })
  }

  const loadDir = async (
    path?: string,
    opts: LoadOpts = {},
    navOpts: { resetScroll?: boolean } = {},
  ) => {
    const shouldResetScroll = navOpts.resetScroll ?? !opts.silent
    if (shouldResetScroll) {
      deps.resetScrollPosition()
    }
    navGeneration += 1
    pendingNav = null
    captureSelectionSnapshot()
    await deps.loadRaw(path, opts)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const loadRecent = async (
    recordHistory = true,
    applySort = false,
    navOpts: { resetScroll?: boolean } = {},
  ) => {
    if (navOpts.resetScroll ?? true) {
      deps.resetScrollPosition()
    }
    captureSelectionSnapshot()
    await deps.loadRecentRaw(recordHistory, applySort)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const loadStarred = async (recordHistory = true, navOpts: { resetScroll?: boolean } = {}) => {
    if (navOpts.resetScroll ?? true) {
      deps.resetScrollPosition()
    }
    captureSelectionSnapshot()
    await deps.loadStarredRaw(recordHistory)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const loadNetwork = async (
    recordHistory = true,
    navOpts: { resetScroll?: boolean; forceRefresh?: boolean } = {},
  ) => {
    if (navOpts.resetScroll ?? true) {
      deps.resetScrollPosition()
    }
    captureSelectionSnapshot()
    await deps.loadNetworkRaw(recordHistory, { forceRefresh: navOpts.forceRefresh === true })
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const loadTrash = async (recordHistory = true, navOpts: { resetScroll?: boolean } = {}) => {
    if (navOpts.resetScroll ?? true) {
      deps.resetScrollPosition()
    }
    captureSelectionSnapshot()
    await deps.loadTrashRaw(recordHistory)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const goBack = async () => {
    captureSelectionSnapshot()
    const before = get(deps.current)
    await deps.goBackRaw()
    if (get(deps.current) !== before) {
      deps.resetScrollPosition()
    }
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const goForward = async () => {
    captureSelectionSnapshot()
    const before = get(deps.current)
    await deps.goForwardRaw()
    if (get(deps.current) !== before) {
      deps.resetScrollPosition()
    }
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const openPathAsFile = (path: string) => {
    const name = path.split(/[\\/]+/).filter((segment) => segment.length > 0).pop() ?? path
    void deps.open({
      name,
      path,
      kind: 'file',
      iconId: 0,
    })
  }

  const loadDirIfIdle = async (path: string, opts: LoadOpts = {}) => {
    if (get(deps.loading)) {
      if (pendingNav && pendingNav.path === path) {
        return
      }
      pendingNav = { path, opts, gen: navGeneration + 1 }
      return
    }
    await loadDir(path, opts)
  }

  const openPartition = async (path: string) => {
    if (await isMountUri(path)) {
      try {
        const result = await connectNetworkUri(path)
        if (result.kind === 'unsupported') {
          deps.showToast('Unsupported network protocol')
          return
        }
        if (result.kind === 'mountable') {
          await deps.loadPartitions({ forceNetworkRefresh: true })
          if (result.mountedPath) {
            await loadDirIfIdle(result.mountedPath)
          } else {
            deps.showToast('Mounted, but no mount path found')
          }
        }
      } catch (err) {
        deps.showToast(`Connect failed: ${getErrorMessage(err)}`)
      }
      return
    }

    await loadDirIfIdle(path)
  }

  const goToPath = async (path: string) => {
    const trimmed = path.trim()
    if (!trimmed) return

    if (await isMountUri(trimmed)) {
      await openPartition(trimmed)
      return
    }

    try {
      const kind = await entryKind(trimmed)
      if (kind === 'dir') {
        if (trimmed !== get(deps.current) && !get(deps.loading)) {
          await loadDir(trimmed)
        }
        return
      }

      openPathAsFile(trimmed)
      deps.setPathInput(get(deps.current))
    } catch {
      if (trimmed !== get(deps.current) && !get(deps.loading)) {
        await loadDir(trimmed)
      }
    }
  }

  const handlePlace = (label: string, path: string) => {
    captureSelectionSnapshot()
    if (label === 'Recent') {
      void loadRecent()
      return
    }
    if (label === 'Starred') {
      void loadStarred()
      return
    }
    if (label === 'Network') {
      void loadNetwork(true, { forceRefresh: true })
      return
    }
    if (label === 'Wastebasket') {
      void loadTrash()
      return
    }
    if (path) {
      void loadDir(path)
    } else {
      restoreSelectionForCurrent()
    }
  }

  const flushPendingNavigation = () => {
    if (get(deps.loading) || !pendingNav) return
    if (pendingNav.gen > navGeneration) {
      const next = pendingNav
      pendingNav = null
      navGeneration = next.gen
      void loadDir(next.path, next.opts ?? {})
      return
    }
    pendingNav = null
  }

  const dropPendingNavIfCurrent = () => {
    if (pendingNav && pendingNav.path === get(deps.current)) {
      pendingNav = null
    }
  }

  return {
    viewFromPath,
    loadDir,
    loadRecent,
    loadStarred,
    loadNetwork,
    loadTrash,
    goBack,
    goForward,
    goToPath,
    loadDirIfIdle,
    openPartition,
    handlePlace,
    flushPendingNavigation,
    dropPendingNavIfCurrent,
  }
}
