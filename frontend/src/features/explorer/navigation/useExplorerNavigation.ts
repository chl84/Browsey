import { cloudLeafName } from '../cloudPaths'
import { tick } from 'svelte'
import { get, type Readable, type Writable } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import { connectNetworkUri, isMountUri, statCloudEntry } from '@/features/network'
import { entryKind } from '../services/files.service'
import { isMtpUri, isUnmountedUsb, mountUsbVolume } from '../services/drives.service'
import { createSelectionMemory } from '../selection/selectionMemory'
import type { Entry } from '../model/types'
import type { CurrentView } from '../context/createContextActions'

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

type PartitionTarget = { path: string | null; warning?: string | null; message?: string }

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
  const isCloudPath = (path: string) => path.startsWith('rclone://')

  let pendingNav: PendingNav | null = null
  let navGeneration = 0
  const beginNavigation = () => {
    pendingNav = null
    return ++navGeneration
  }

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

  const centerSelectionIfAny = async (generation: number) => {
    const list = get(deps.filteredEntries)
    const sel = get(deps.selected)
    const current = get(deps.current)
    if (sel.size === 0 || list.length === 0) return
    const idx =
      get(deps.caretIndex) ??
      get(deps.anchorIndex) ??
      list.findIndex((entry) => sel.has(entry.path))
    if (idx === null || idx < 0) return

    await tick()
    requestAnimationFrame(() => {
      if (generation !== navGeneration || current !== get(deps.current) || sel !== get(deps.selected)) return
      if (deps.getViewMode() === 'list') {
        const rowsEl = deps.getRowsEl()
        if (!rowsEl) return
        const viewport = rowsEl.clientHeight
        const rowH = get(deps.rowHeight)
        const target = Math.max(0, idx * rowH - (viewport - rowH) / 2)
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

  const finishNavigation = async (generation: number) => {
    if (generation !== navGeneration) return
    restoreSelectionForCurrent()
    await centerSelectionIfAny(generation)
  }

  const loadDirForRequest = async (
    generation: number,
    path?: string,
    opts: LoadOpts = {},
    navOpts: { resetScroll?: boolean } = {},
  ) => {
    const shouldResetScroll = navOpts.resetScroll ?? !opts.silent
    if (shouldResetScroll) {
      deps.resetScrollPosition()
    }
    captureSelectionSnapshot()
    await deps.loadRaw(path, opts)
    await finishNavigation(generation)
  }

  const loadDir = (path?: string, opts: LoadOpts = {}, navOpts: { resetScroll?: boolean } = {}) =>
    loadDirForRequest(beginNavigation(), path, opts, navOpts)

  const loadRecent = async (
    recordHistory = true,
    applySort = false,
    navOpts: { resetScroll?: boolean } = {},
  ) => {
    const generation = beginNavigation()
    if (navOpts.resetScroll ?? true) {
      deps.resetScrollPosition()
    }
    captureSelectionSnapshot()
    await deps.loadRecentRaw(recordHistory, applySort)
    await finishNavigation(generation)
  }

  const loadStarred = async (recordHistory = true, navOpts: { resetScroll?: boolean } = {}) => {
    const generation = beginNavigation()
    if (navOpts.resetScroll ?? true) {
      deps.resetScrollPosition()
    }
    captureSelectionSnapshot()
    await deps.loadStarredRaw(recordHistory)
    await finishNavigation(generation)
  }

  const loadNetwork = async (
    recordHistory = true,
    navOpts: { resetScroll?: boolean; forceRefresh?: boolean } = {},
  ) => {
    const generation = beginNavigation()
    if (navOpts.resetScroll ?? true) {
      deps.resetScrollPosition()
    }
    captureSelectionSnapshot()
    await deps.loadNetworkRaw(recordHistory, { forceRefresh: navOpts.forceRefresh === true })
    await finishNavigation(generation)
  }

  const loadTrash = async (recordHistory = true, navOpts: { resetScroll?: boolean } = {}) => {
    const generation = beginNavigation()
    if (navOpts.resetScroll ?? true) {
      deps.resetScrollPosition()
    }
    captureSelectionSnapshot()
    await deps.loadTrashRaw(recordHistory)
    await finishNavigation(generation)
  }

  const goBack = async () => {
    const generation = beginNavigation()
    captureSelectionSnapshot()
    const before = get(deps.current)
    await deps.goBackRaw()
    if (generation !== navGeneration) return
    if (get(deps.current) !== before) {
      deps.resetScrollPosition()
    }
    await finishNavigation(generation)
  }

  const goForward = async () => {
    const generation = beginNavigation()
    captureSelectionSnapshot()
    const before = get(deps.current)
    await deps.goForwardRaw()
    if (generation !== navGeneration) return
    if (get(deps.current) !== before) {
      deps.resetScrollPosition()
    }
    await finishNavigation(generation)
  }

  const openPathAsFile = (path: string) => {
    const name = cloudLeafName(path)
    void deps.open({
      name,
      path,
      kind: 'file',
      iconId: 0,
    })
  }

  const loadDirIfIdleForRequest = async (generation: number, path: string, opts: LoadOpts = {}) => {
    if (generation !== navGeneration) return
    if (get(deps.loading)) {
      pendingNav = { path, opts, gen: generation }
      return
    }
    await loadDirForRequest(generation, path, opts)
  }

  const loadDirIfIdle = (path: string, opts: LoadOpts = {}) => {
    if (pendingNav?.path === path && pendingNav.gen === navGeneration) return Promise.resolve()
    return loadDirIfIdleForRequest(beginNavigation(), path, opts)
  }

  const resolvePartition = async (path: string): Promise<PartitionTarget> => {
    if (isUnmountedUsb(path)) {
      const mountedPath = await mountUsbVolume(path)
      await deps.loadPartitions()
      return { path: mountedPath }
    }
    if (isCloudPath(path)) return { path }
    if (isMtpUri(path) || await isMountUri(path)) {
      const result = await connectNetworkUri(path)
      if (result.kind === 'unsupported') {
        return { path: null, warning: result.warning, message: 'Unsupported network protocol' }
      }
      if (result.kind === 'mountable') {
        await deps.loadPartitions({ forceNetworkRefresh: true })
        return { path: result.mountedPath, warning: result.warning,
          message: result.mountedPath ? undefined : 'Mounted, but no mount path found' }
      }
      return { path: null, warning: result.warning }
    }
    return { path }
  }

  const connectingPartitions = new Map<string, { generation: number; promise: Promise<PartitionTarget> }>()
  const openPartition = async (path: string) => {
    const existing = connectingPartitions.get(path)
    if (existing?.generation === navGeneration) return
    const generation = beginNavigation()
    const connection = existing ?? { generation, promise: resolvePartition(path) }
    connection.generation = generation
    connectingPartitions.set(path, connection)
    const navigate = async () => {
      try {
        const target = await connection.promise
        if (generation !== navGeneration) return
        if (target.warning) deps.showToast(target.warning)
        if (target.message) deps.showToast(target.message)
        if (target.path) await loadDirIfIdleForRequest(generation, target.path)
      } catch (err) {
        if (generation !== navGeneration) return
        const label = isUnmountedUsb(path) ? 'Mount failed'
          : isMtpUri(path) ? 'Phone connection failed' : 'Connect failed'
        deps.showToast(`${label}: ${getErrorMessage(err)}`)
      } finally {
        if (connection.generation === generation && connectingPartitions.get(path) === connection) {
          connectingPartitions.delete(path)
        }
      }
    }
    // A renewed click can claim the still-running connection after another
    // navigation, while ordinary repeated clicks remain coalesced.
    if (existing) { void navigate(); return }
    await navigate()
  }

  const goToPath = async (path: string) => {
    const trimmed = path.trim()
    if (!trimmed) return
    const generation = beginNavigation()

    if (isCloudPath(trimmed)) {
      try {
        const entry = await statCloudEntry(trimmed)
        if (generation !== navGeneration) return
        if (entry?.kind === 'dir') {
          if (entry.path !== get(deps.current) && !get(deps.loading)) {
            await loadDirForRequest(generation, entry.path)
          }
          return
        }
        if (entry?.kind === 'file') {
          openPathAsFile(entry.path)
          deps.setPathInput(get(deps.current))
          return
        }
      } catch {
        // Fall through to directory navigation fallback, matching local-path behavior.
      }
      if (generation !== navGeneration) return
      if (trimmed !== get(deps.current) && !get(deps.loading)) {
        await loadDirForRequest(generation, trimmed)
      }
      return
    }

    const mountUri = await isMountUri(trimmed)
    if (generation !== navGeneration) return
    if (mountUri) {
      await openPartition(trimmed)
      return
    }

    try {
      const kind = await entryKind(trimmed)
      if (generation !== navGeneration) return
      if (kind === 'dir') {
        if (trimmed !== get(deps.current) && !get(deps.loading)) {
          await loadDirForRequest(generation, trimmed)
        }
        return
      }

      openPathAsFile(trimmed)
      deps.setPathInput(get(deps.current))
    } catch {
      if (generation !== navGeneration) return
      if (trimmed !== get(deps.current) && !get(deps.loading)) {
        await loadDirForRequest(generation, trimmed)
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
    if (pendingNav.gen === navGeneration) {
      const next = pendingNav
      pendingNav = null
      void loadDirForRequest(next.gen, next.path, next.opts ?? {})
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
