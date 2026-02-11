<script lang="ts">
  // --- Imports -------------------------------------------------------------
  import { onMount, onDestroy, tick } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { get, writable } from 'svelte/store'
  import { formatItems, formatSelectionLine, formatSize, normalizePath, parentPath } from './features/explorer/utils'
  import { createListState } from './features/explorer/stores/listState'
  import ExplorerShell from './features/explorer/components/ExplorerShell.svelte'
  import { useExplorerData } from './features/explorer/hooks/useExplorerData'
  import { createColumnResize } from './features/explorer/hooks/columnWidths'
  import { createGlobalShortcuts } from './features/explorer/hooks/shortcuts'
  import { createBookmarkModal } from './features/explorer/hooks/bookmarkModal'
  import { useDragDrop } from './features/explorer/hooks/useDragDrop'
  import { useModalsController } from './features/explorer/hooks/useModalsController'
  import { useGridVirtualizer } from './features/explorer/hooks/useGridVirtualizer'
  import { addBookmark, removeBookmark } from './features/explorer/services/bookmarks'
  import { ejectDrive, mountPartition } from './features/explorer/services/drives'
  import { openConsole } from './features/explorer/services/console'
  import {
    copyPathsToSystemClipboard,
    setClipboardCmd,
    clearSystemClipboard,
    pasteClipboardCmd,
    pasteClipboardPreview,
  } from './features/explorer/services/clipboard'
  import { undoAction, redoAction } from './features/explorer/services/history'
  import { cancelTask } from './features/explorer/services/activity'
  import { deleteEntry, deleteEntries, moveToTrashMany } from './features/explorer/services/trash'
  import type { Entry, Partition, SortField, Density } from './features/explorer/types'
  import { toast, showToast } from './features/explorer/hooks/useToast'
  import { createClipboard } from './features/explorer/hooks/useClipboard'
  import { setClipboardState, clearClipboardState } from './features/explorer/stores/clipboardState'
  import { createContextMenus } from './features/explorer/hooks/useContextMenus'
  import type { ContextAction } from './features/explorer/hooks/useContextMenus'
  import { createContextActions, type CurrentView } from './features/explorer/hooks/useContextActions'
  import { createSelectionBox } from './features/explorer/hooks/selectionBox'
  import { hitTestGridVirtualized } from './features/explorer/helpers/lassoHitTest'
  import { createViewSwitchAnchor } from './features/explorer/hooks/viewAnchor'
  import { ensureSelectionBeforeMenu } from './features/explorer/helpers/contextMenuHelpers'
  import { moveCaret } from './features/explorer/helpers/navigationController'
  import { createSelectionMemory } from './features/explorer/selectionMemory'
  import { loadDefaultView, storeDefaultView } from './features/explorer/services/settings'
  import { createGridKeyboardHandler } from './features/explorer/hooks/useGridHandlers'
  import { useContextMenuBlocker } from './features/explorer/hooks/useContextMenuBlocker'
  import { createActivity } from './features/explorer/hooks/useActivity'
  import { loadShortcuts, setShortcutBinding } from './features/shortcuts/service'
  import {
    DEFAULT_SHORTCUTS,
    matchesAnyShortcut,
    matchesShortcut,
    shortcutFor,
    type ShortcutBinding,
    type ShortcutCommandId,
  } from './features/shortcuts/keymap'
  import DragGhost from './ui/DragGhost.svelte'
  import TextContextMenu from './ui/TextContextMenu.svelte'
  import { createNativeFileDrop } from './features/explorer/hooks/useNativeFileDrop'
  import { startNativeFileDrag } from './features/explorer/services/nativeDrag'
  import {
    checkDuplicatesStream,
    type DuplicateScanProgress,
  } from './features/explorer/services/duplicates'
  import {
    clearBookmarks,
    clearRecents,
    clearStars,
    clearThumbnailCache,
  } from './features/explorer/services/data'
  import ConflictModal from './ui/ConflictModal.svelte'
  import SettingsModal from './features/settings/SettingsModal.svelte'
  import { anyModalOpen as anyModalOpenStore } from './ui/modalOpenState'
  import './features/explorer/ExplorerLayout.css'

  // --- Types --------------------------------------------------------------
  type ExtractResult = { destination: string; skipped_symlinks: number; skipped_entries: number }
  type ExtractBatchItem = {
    path: string
    ok: boolean
    result?: ExtractResult | null
    error?: string | null
  }
  type ViewMode = 'list' | 'grid'
  type MountEvent = { path: string; fs?: string; ok?: boolean; duration_ms?: number }

  // --- Core UI state -------------------------------------------------------
  let sidebarCollapsed = false

  // Path / mode
  let pathInput = ''
  let mode: 'address' | 'filter' | 'search' = 'address'
  let viewMode: ViewMode = 'list'
  let defaultViewPref: ViewMode = 'list'
  let inputFocused = false

  // DOM refs & observers
  let rowsElRef: HTMLDivElement | null = null
  let gridElRef: HTMLDivElement | null = null
  let headerElRef: HTMLDivElement | null = null
  let pathInputEl: HTMLInputElement | null = null
  let rowsObserver: ResizeObserver | null = null
  let gridObserver: ResizeObserver | null = null
  let rowsRaf: number | null = null
  let gridRaf: number | null = null

  // --- Nav + bookmarks -----------------------------------------------------
  const places = [
    { label: 'Home', path: '~' },
    { label: 'Recent', path: '' },
    { label: 'Starred', path: '' },
    { label: 'Network', path: '' },
    { label: 'Wastebasket', path: 'trash://' },
  ]

  let bookmarks: { label: string; path: string }[] = []
  let partitions: Partition[] = []

  // Modals & dialogs
  const bookmarkModal = createBookmarkModal()
  let bookmarkModalOpen = false
  let bookmarkName = ''
  let bookmarkCandidate: Entry | null = null
  let bookmarkInputEl: HTMLInputElement | null = null
  let renameValue = ''
  let conflictModalOpen = false
  let conflictList: { src: string; target: string; is_dir: boolean }[] = []
  let conflictDest: string | null = null
  let compressName = 'Archive'
  let compressLevel = 6
  let newFolderName = 'New folder'
  let newFileName = ''
  let settingsOpen = false
  let thumbnailRefreshToken = 0
  let shortcutBindings: ShortcutBinding[] = DEFAULT_SHORTCUTS
  let pendingNav: { path: string; opts?: { recordHistory?: boolean; silent?: boolean }; gen: number } | null = null
  let navGeneration = 0

  // Drag & clipboard
  const dragDrop = useDragDrop()
  const { store: bookmarkStore } = bookmarkModal
  const dragState = dragDrop.state
  let dragAction: 'copy' | 'move' | null = null
  let clipboardMode: 'copy' | 'cut' = 'copy'
  let clipboardPaths = new Set<string>()

  // View / navigation tracking
  let currentView: CurrentView = 'dir'
  let lastLocation = ''
  let extracting = false
  useContextMenuBlocker()

  // --- Helpers -------------------------------------------------------------
  const isEditableTarget = (target: EventTarget | null) => {
    if (!(target instanceof HTMLElement)) return false
    const tag = target.tagName.toLowerCase()
    return target.isContentEditable || tag === 'input' || tag === 'textarea' || tag === 'select'
  }
  const clipboard = createClipboard()
  const clipboardStore = clipboard.state
  const {
    contextMenu,
    blankMenu,
    openContextMenu,
    closeContextMenu,
    openBlankContextMenu,
    closeBlankContextMenu,
  } = createContextMenus()
  const selectionBox = createSelectionBox()
  const activityApi = createActivity({ onError: showToast })
  const activity = activityApi.activity

  const fsLabel = (fs?: string) => {
    const kind = (fs ?? '').toLowerCase()
    if (kind === 'onedrive') return 'OneDrive'
    if (kind === 'mtp') return 'MTP device'
    return 'drive'
  }

  // --- Data + preferences --------------------------------------------------
  const explorer = useExplorerData({
    onEntriesChanged: () => resetScrollPosition(),
    onCurrentChange: (path) => {
      const sameLocation = path === lastLocation
      const typingFilterOrSearch = mode === 'filter' || mode === 'search'
      if (sameLocation && typingFilterOrSearch) {
        return
      }
      pathInput = path
    },
  })

  const {
    cols,
    gridTemplate,
    current,
    entries,
    loading,
    error,
    filter,
    searchMode,
    searchActive,
    showHidden,
    hiddenFilesLast,
    foldersFirst,
    confirmDelete,
    startDirPref,
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
    sortFieldPref,
    sortDirectionPref,
    sortField,
    sortDirection,
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
    setMountsPollPref,
    setDoubleClickMsPref,
    bookmarks: bookmarksStore,
    partitions: partitionsStore,
    filteredEntries,
    load: loadRaw,
    loadRecent: loadRecentRaw,
    loadStarred: loadStarredRaw,
    loadTrash: loadTrashRaw,
    runSearch,
    toggleMode,
    toggleShowHidden,
    toggleHiddenFilesLast,
    toggleFoldersFirst,
    toggleConfirmDelete,
    setStartDirPref,
    changeSort,
    open,
    toggleStar,
    goBack: goBackRaw,
    goForward: goForwardRaw,
    loadBookmarks,
    loadPartitions,
    loadSavedWidths,
    persistWidths,
  } = explorer

  const selectionActive = selectionBox.active
  const selectionRect = selectionBox.rect

  function getListMaxWidth(): number | null {
    // Sørg for at kolonner ikke kan bli bredere enn den synlige listen (inkl. stjernekolonnen).
    const el = rowsElRef ?? headerElRef
    if (!el) return null
    const style = getComputedStyle(el)
    const paddingLeft = parseFloat(style.paddingLeft) || 0
    const paddingRight = parseFloat(style.paddingRight) || 0
    return Math.max(0, el.clientWidth - paddingLeft - paddingRight)
  }

  const getGridPadding = (el: HTMLDivElement) => {
    const style = getComputedStyle(el)
    return {
      paddingLeft: parseFloat(style.paddingLeft) || 0,
      paddingTop: parseFloat(style.paddingTop) || 0,
    }
  }

  const focusCurrentView = async () => {
    await tick()
    rowsElRef?.focus()
  }

  const isShortcut = (event: KeyboardEvent, commandId: ShortcutCommandId) => {
    return matchesShortcut(event, shortcutBindings, commandId)
  }

  const commandForContextAction = (actionId: string): ShortcutCommandId | null => {
    if (actionId === 'cut') return 'cut'
    if (actionId === 'copy') return 'copy'
    if (actionId === 'paste') return 'paste'
    if (actionId === 'rename') return 'rename'
    if (actionId === 'properties') return 'properties'
    if (actionId === 'move-trash') return 'delete_to_wastebasket'
    if (actionId === 'delete-permanent') return 'delete_permanently'
    if (actionId === 'open-console') return 'open_console'
    return null
  }

  const applyContextMenuShortcuts = (actions: ContextAction[]): ContextAction[] => {
    return actions.map((action) => {
      const commandId = commandForContextAction(action.id)
      const nextShortcut = commandId
        ? shortcutFor(shortcutBindings, commandId)?.accelerator ?? action.shortcut
        : action.shortcut
      const nextChildren = action.children ? applyContextMenuShortcuts(action.children) : undefined
      return {
        ...action,
        ...(nextShortcut ? { shortcut: nextShortcut } : {}),
        ...(nextChildren ? { children: nextChildren } : {}),
      }
    })
  }

  const applyShortcutBindings = (next: ShortcutBinding[]) => {
    if (Array.isArray(next) && next.length > 0) {
      shortcutBindings = next
    }
  }

  const updateShortcutBinding = async (
    commandId: ShortcutCommandId,
    accelerator: string,
  ) => {
    const next = await setShortcutBinding(commandId, accelerator)
    applyShortcutBindings(next)
  }

  const toggleViewMode = async () => {
    viewAnchor.capture({
      viewMode,
      rowsEl: rowsElRef,
      headerEl: headerElRef,
      gridEl: gridElRef,
      gridCols: getGridCols(),
    })
    const nextMode = viewMode === 'list' ? 'grid' : 'list'
    const switchingToList = nextMode === 'list'

    if (switchingToList && gridObserver) {
      gridObserver.disconnect()
      gridObserver = null
    }

    viewMode = nextMode
    selectionBox.active.set(false)
    selectionBox.rect.set({ x: 0, y: 0, width: 0, height: 0 })

    if (switchingToList) {
      gridTotalHeight.set(0)
      await tick()
      scrollTop.set(Math.max(0, rowsElRef?.scrollTop ?? 0))
      updateViewportHeight()
      recompute(get(filteredEntries))
      requestAnimationFrame(() => {
        scrollTop.set(Math.max(0, rowsElRef?.scrollTop ?? 0))
        updateViewportHeight()
        recompute(get(filteredEntries))
        viewAnchor.scroll({
          viewMode,
          rowsEl: rowsElRef,
          headerEl: headerElRef,
          gridEl: gridElRef,
          gridCols: getGridCols(),
        })
      })
    } else {
      await tick()
      viewAnchor.scroll({
        viewMode,
        rowsEl: rowsElRef,
        headerEl: headerElRef,
        gridEl: gridElRef,
        gridCols: getGridCols(),
      })
    }
    void focusCurrentView()
  }

  // --- List/grid derived state & handlers ---------------------------------
  const {
    selected,
    anchorIndex,
    caretIndex,
    rowHeight,
    setRowHeight,
    viewportHeight,
    scrollTop,
    rowsEl: rowsElStore,
    headerEl: headerElStore,
    totalHeight,
    visibleEntries,
    start,
    offsetY,
    updateViewportHeight,
    handleResize: handleListResize,
    handleRowsScroll,
    handleWheel,
    handleRowsKeydown,
    handleRowsClick,
    handleRowClick,
    resetScrollPosition,
    recompute,
  } = createListState()

  // --- Selection memory & drive helpers -----------------------------------

  const selectionMemory = createSelectionMemory()
  const driveLetter = (path: string): string | null => {
    const norm = normalizePath(path)
    const match = norm.match(/^([A-Za-z]):\//)
    return match ? match[1].toUpperCase() : null
  }
  const mountForPath = (path: string): string | null => {
    const norm = normalizePath(path)
    const parts = get(partitionsStore)
    let best: string | null = null
    for (const part of parts) {
      const root = normalizePath(part.path)
      if (!root) continue
      if (norm === root || norm.startsWith(`${root}/`)) {
        if (!best || root.length > best.length) {
          best = root
        }
      }
    }
    return best
  }
  const isCrossVolume = (paths: string[], dest: string): boolean | null => {
    if (paths.length === 0) return null
    const destMount = mountForPath(dest)
    let unknown = false
    for (const p of paths) {
      const srcMount = mountForPath(p)
      if (srcMount && destMount) {
        if (srcMount !== destMount) return true
        continue
      }
      if (srcMount || destMount) {
        unknown = true
        continue
      }
      const srcDrive = driveLetter(p)
      const destDrive = driveLetter(dest)
      if (srcDrive && destDrive) {
        if (srcDrive !== destDrive) return true
        continue
      }
      if (srcDrive || destDrive) {
        unknown = true
        continue
      }
      unknown = true
    }
    if (unknown) return null
    return false
  }
  const shouldCopyForDrop = (dest: string, event: DragEvent) => {
    if (copyModifierActive) return true
    const cross = isCrossVolume(dragPaths, dest)
    if (cross === true) return true
    return false
  }

  const viewFromPath = (value: string): CurrentView =>
    value === 'Recent'
      ? 'recent'
      : value === 'Starred'
        ? 'starred'
        : value.startsWith('Trash')
          ? 'trash'
          : 'dir'

  const captureSelectionSnapshot = () => {
    const curr = get(current)
    if (!curr || viewFromPath(curr) !== 'dir') return
    selectionMemory.capture(curr, get(filteredEntries), get(selected), get(anchorIndex), get(caretIndex))
  }

  const restoreSelectionForCurrent = () => {
    const curr = get(current)
    const view = curr ? viewFromPath(curr) : 'dir'
    if (!curr || view !== 'dir') {
      selected.set(new Set())
      anchorIndex.set(null)
      caretIndex.set(null)
      return
    }
    const restored = selectionMemory.restore(curr, get(filteredEntries))
    if (restored) {
      selected.set(restored.selected)
      anchorIndex.set(restored.anchorIndex)
      caretIndex.set(restored.caretIndex)
    } else {
      selected.set(new Set())
      anchorIndex.set(null)
      caretIndex.set(null)
    }
  }

  const centerSelectionIfAny = async () => {
    const list = get(filteredEntries)
    const sel = get(selected)
    if (sel.size === 0 || list.length === 0) return
    const idx =
      get(caretIndex) ??
      get(anchorIndex) ??
      list.findIndex((e) => sel.has(e.path))
    if (idx === null || idx < 0) return

    await tick()
    requestAnimationFrame(() => {
      if (viewMode === 'list') {
        const rowsEl = rowsElRef
        if (!rowsEl) return
        const headerH = headerElRef?.offsetHeight ?? 0
        const viewport = rowsEl.clientHeight - headerH
        const rowH = $rowHeight
        const target = headerH + Math.max(0, idx * rowH - (viewport - rowH) / 2)
        const maxScroll = Math.max(0, rowsEl.scrollHeight - rowsEl.clientHeight)
        rowsEl.scrollTo({ top: Math.min(target, maxScroll), behavior: 'auto' })
      } else {
        const gridEl = gridElRef
        const cols = getGridCols()
        if (!gridEl || cols <= 0) return
        const stride = gridRowHeight + gridGap
        const row = Math.floor(idx / Math.max(1, cols))
        const viewport = gridEl.clientHeight
        const target = Math.max(0, row * stride - (viewport - stride) / 2)
        const maxScroll =
          Math.max(get(gridTotalHeight) - viewport, gridEl.scrollHeight - gridEl.clientHeight, 0)
        gridEl.scrollTo({ top: Math.min(target, maxScroll), behavior: 'auto' })
      }
    })
  }

  const loadDir = async (path?: string, opts: { recordHistory?: boolean; silent?: boolean } = {}) => {
    navGeneration += 1
    pendingNav = null
    captureSelectionSnapshot()
    await loadRaw(path, opts)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const loadRecent = async (recordHistory = true, applySort = false) => {
    captureSelectionSnapshot()
    await loadRecentRaw(recordHistory, applySort)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const loadStarred = async (recordHistory = true) => {
    captureSelectionSnapshot()
    await loadStarredRaw(recordHistory)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const loadTrash = async (recordHistory = true) => {
    captureSelectionSnapshot()
    await loadTrashRaw(recordHistory)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const goBack = async () => {
    captureSelectionSnapshot()
    await goBackRaw()
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const goForward = async () => {
    captureSelectionSnapshot()
    await goForwardRaw()
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const openPathAsFile = (path: string) => {
    const name = path.split(/[\\/]+/).filter((segment) => segment.length > 0).pop() ?? path
    open({
      name,
      path,
      kind: 'file',
      iconId: 0,
    })
  }

  const goToPath = async (path: string) => {
    const trimmed = path.trim()
    if (!trimmed) return

    try {
      const kind = await invoke<'dir' | 'file'>('entry_kind_cmd', { path: trimmed })
      if (kind === 'dir') {
        if (trimmed !== get(current) && !get(loading)) {
          await loadDir(trimmed)
        }
        return
      }

      openPathAsFile(trimmed)
      pathInput = get(current)
    } catch {
      if (trimmed !== get(current) && !get(loading)) {
        await loadDir(trimmed)
      }
    }
  }

  const loadDirIfIdle = async (path: string, opts: { recordHistory?: boolean; silent?: boolean } = {}) => {
    if (get(loading)) {
      if (pendingNav && pendingNav.path === path) {
        return
      }
      pendingNav = { path, opts, gen: navGeneration + 1 }
      return
    }
    await loadDir(path, opts)
  }

  const openPartition = async (path: string) => {
    const lower = path.toLowerCase()
    if (lower.startsWith('onedrive://')) {
      try {
        if (get(loading)) return
        await mountPartition(path)
        await loadPartitions()
        const mounted = get(partitionsStore).find(
          (p) => p.fs?.toLowerCase() === 'onedrive' && !p.path.toLowerCase().startsWith('onedrive://')
        )
        if (mounted) {
          await loadDirIfIdle(mounted.path)
        } else {
          showToast('Mounted, but no OneDrive mount path found')
        }
      } catch (err) {
        showToast(`Mount failed: ${err instanceof Error ? err.message : String(err)}`)
      }
      return
    }

    await loadDirIfIdle(path)
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

  // If a navigation was queued while loading, execute it when idle.
  $: if (!get(loading) && pendingNav) {
    if (pendingNav.gen > navGeneration) {
      const next = pendingNav
      pendingNav = null
      navGeneration = next.gen
      void loadDir(next.path, next.opts ?? {})
    } else {
      pendingNav = null
    }
  }

  $: rowsElStore.set(viewMode === 'list' ? rowsElRef : null)
  $: headerElStore.set(viewMode === 'list' ? headerElRef : null)
  $: {
    gridElRef = viewMode === 'grid' ? rowsElRef : null
    if (viewMode !== 'grid' && gridObserver) {
      gridObserver.disconnect()
      gridObserver = null
    }
  }

  $: rowsKeydownHandler = handleRowsKeydown($filteredEntries)
  $: rowSelectionHandler = handleRowClick($filteredEntries)

  $: bookmarks = $bookmarksStore
  $: partitions = $partitionsStore
  $: {
    const state = $clipboardStore
    clipboardMode = state.mode
    clipboardPaths = state.paths
  }
  $: currentView =
    $current === 'Recent'
      ? 'recent'
      : $current === 'Starred'
        ? 'starred'
        : $current.startsWith('Trash')
          ? 'trash'
          : 'dir'
  // Drop stale pending nav if we already are at that path
  $: if (pendingNav && pendingNav.path === get(current)) {
    pendingNav = null
  }
  $: {
    const state = $bookmarkStore
    bookmarkModalOpen = state.open
    bookmarkName = state.name
    bookmarkCandidate = state.candidate as Entry | null
  }
  $: {
    const d = $density
    if (typeof document !== 'undefined') {
      document.body.classList.remove('density-cozy', 'density-compact')
      document.body.classList.add(`density-${d}`)
    }
  }

  const readCssNumber = (name: string, fallback: number) => {
    if (typeof document === 'undefined') return fallback
    const raw = getComputedStyle(document.body).getPropertyValue(name)
    const parsed = parseFloat(raw)
    return Number.isFinite(parsed) ? parsed : fallback
  }

  const applyDensityMetrics = () => {
    const nextRowHeight = readCssNumber('--row-height', 32)
    const nextGridGap = readCssNumber('--grid-gap', 6)
    const nextGridCardWidth = readCssNumber('--grid-card-width', 120)
    const nextGridRowHeight = readCssNumber('--grid-row-height', 126)

    setRowHeight(nextRowHeight)
    gridGap = nextGridGap
    gridCardWidth = nextGridCardWidth
    gridRowHeight = nextGridRowHeight

    gridConfig.gap = nextGridGap
    gridConfig.cardWidth = nextGridCardWidth
    gridConfig.rowHeight = nextGridRowHeight

    viewAnchor = createViewSwitchAnchor({
      filteredEntries,
      rowHeight: nextRowHeight,
      gridRowHeight,
      gridGap,
    })

    if (viewMode === 'grid') {
      recomputeGrid()
      const entriesList = get(filteredEntries)
      if (entriesList.length > 0 && get(visibleEntries).length === 0 && gridElRef) {
        const maxTop = Math.max(
          0,
          get(gridTotalHeight) - gridElRef.clientHeight,
          gridElRef.scrollHeight - gridElRef.clientHeight
        )
        gridElRef.scrollTop = Math.min(gridElRef.scrollTop, maxTop)
        recomputeGrid()
      }
    } else {
      const rowsEl = rowsElRef
      const entriesList = get(filteredEntries)
      if (rowsEl) {
        const headerH = headerElRef?.offsetHeight ?? 0
        const viewport = rowsEl.clientHeight - headerH
        const maxTop = Math.max(0, get(totalHeight) - viewport)
        if (rowsEl.scrollTop > maxTop) {
          rowsEl.scrollTop = maxTop
        }
      }
      recompute(entriesList)
    }
  }

  $: {
    $density
    applyDensityMetrics()
  }
  $: {
    if (bookmarkModalOpen) {
      bookmarkModal.setName(bookmarkName)
    }
  }

  let selectionDrag = false
  let gridCardWidth = 120
  let gridRowHeight = 126
  let gridGap = 6
  const GRID_OVERSCAN = 4
  let cursorX = 0
  let cursorY = 0
  const LASSO_GUTTER_WIDTH = 3

  const inLassoGutter = (event: MouseEvent, el: HTMLElement | null) => {
    if (!el) return false
    const rect = el.getBoundingClientRect()
    return event.clientX <= rect.left + LASSO_GUTTER_WIDTH
  }

  let viewAnchor = createViewSwitchAnchor({
    filteredEntries,
    rowHeight: get(rowHeight),
    gridRowHeight,
    gridGap,
  })

  const gridConfig = {
    cardWidth: gridCardWidth,
    rowHeight: gridRowHeight,
    gap: gridGap,
    overscan: GRID_OVERSCAN,
  }

  const {
    gridCols,
    getGridCols,
    gridTotalHeight,
    handleGridScroll,
    handleGridWheel,
    recomputeGrid,
    ensureGridVisible,
  } = useGridVirtualizer({
    getEntries: () => get(filteredEntries),
    getViewMode: () => viewMode,
    getGridEl: () => gridElRef,
    start,
    offsetY,
    totalHeight,
    visibleEntries,
    config: gridConfig,
  })

  const isScrollbarClick = (event: MouseEvent, el: HTMLDivElement | null) => {
    if (!el) return false
    const rect = el.getBoundingClientRect()
    const scrollbarX = el.offsetWidth - el.clientWidth
    const scrollbarY = el.offsetHeight - el.clientHeight
    if (scrollbarX > 0) {
      const x = event.clientX - rect.left
      if (x >= el.clientWidth) return true
    }
    if (scrollbarY > 0) {
      const y = event.clientY - rect.top
      if (y >= el.clientHeight) return true
    }
    return false
  }

  $: {
    if (mode === 'filter') {
      filter.set(pathInput)
      searchActive.set(pathInput.length > 0)
    }
  }

  const resetInputModeForNavigation = () => {
    mode = 'address'
    searchMode.set(false)
    searchActive.set(false)
    filter.set('')
  }

  $: {
    const curr = $current
    if (curr !== lastLocation) {
      lastLocation = curr
      resetInputModeForNavigation()
    }
  }

  const { startResize } = createColumnResize(cols, persistWidths, getListMaxWidth)

  let dirSizeAbort = 0
  let selectionText = ''

  const computeDirStats = async (
    paths: string[],
    onProgress?: (bytes: number, items: number) => void,
  ): Promise<{ total: number; items: number }> => {
    if (paths.length === 0) return { total: 0, items: 0 }
    const token = ++dirSizeAbort
    const progressEvent = `dir-size-progress-${token}-${Math.random().toString(16).slice(2)}`
    let unlisten: UnlistenFn | null = null
    const partials = new Map<string, { bytes: number; items: number }>()
    try {
      if (onProgress) {
        unlisten = await listen<{ path: string; bytes: number; items: number }>(progressEvent, (event) => {
          if (token !== dirSizeAbort) return
          const { path, bytes, items } = event.payload
          partials.set(path, { bytes, items })
          const totals = Array.from(partials.values()).reduce(
            (acc, v) => {
              acc.bytes += v.bytes
              acc.items += v.items
              return acc
            },
            { bytes: 0, items: 0 },
          )
          onProgress(totals.bytes, totals.items)
        })
      }
      const result = await invoke<{ total: number; total_items: number }>('dir_sizes', { paths, progressEvent })
      if (token !== dirSizeAbort) return { total: 0, items: 0 }
      return { total: result.total ?? 0, items: result.total_items ?? 0 }
    } catch (err) {
      console.error('Failed to compute dir sizes', err)
      return { total: 0, items: 0 }
    } finally {
      if (unlisten) {
        await unlisten()
      }
    }
  }

  $: {
    const selectedEntries = $entries.filter((e) => $selected.has(e.path))
    const files = selectedEntries.filter((e) => e.kind === 'file')
    const links = selectedEntries.filter((e) => e.kind === 'link')
    const dirs = selectedEntries.filter((e) => e.kind === 'dir')
    const fileBytes = files.reduce((sum, f) => sum + (f.size ?? 0), 0)
    const fileCount = files.length + links.length

    const dirLine = formatSelectionLine(dirs.length, 'folder')
    const fileLine = formatSelectionLine(fileCount, 'file', fileBytes)

    const filterActive = $filter.trim().length > 0
    const filterLine = filterActive ? `${$filteredEntries.length} results` : ''

    const parts = [filterLine, dirLine, fileLine].filter((p) => p.length > 0)
    selectionText = parts.join(' | ')
  }

  // Re-anchor keyboard navigation after returning to a view so arrows start from the selected item.
  $: {
    const list = $filteredEntries
    if ($selected.size > 0 && list.length > 0) {
      const firstIdx = list.findIndex((e) => $selected.has(e.path))
      if (firstIdx >= 0) {
        const anchorValid =
          $anchorIndex !== null &&
          $anchorIndex < list.length &&
          $selected.has(list[$anchorIndex].path)
        const caretValid =
          $caretIndex !== null &&
          $caretIndex < list.length &&
          $selected.has(list[$caretIndex].path)
        if (!anchorValid) anchorIndex.set(firstIdx)
        if (!caretValid) caretIndex.set(firstIdx)
      }
    }
  }

  const ariaSort = (field: SortField) =>
    $sortField === field ? ($sortDirection === 'asc' ? 'ascending' : 'descending') : 'none'

  const submitPath = () => {
    const trimmed = pathInput.trim()
    if (!trimmed) return
    void goToPath(trimmed)
  }

  const enterAddressMode = async () => {
    mode = 'address'
    if ($searchMode) {
      await toggleMode(false)
    }
    searchActive.set(false)
    filter.set('')
    pathInput = $current
  }

  const isHidden = (entry: Entry) => entry.hidden === true || entry.name.startsWith('.')

  const displayName = (entry: Entry) => {
    if (entry.kind === 'file' && entry.ext) {
      return entry.name.replace(new RegExp(`\\.${entry.ext}$`), '')
    }
    return entry.name
  }

  // --- UI sizing & viewport updates ---------------------------------------
  const handleResize = () => {
    if (typeof window === 'undefined') return
    sidebarCollapsed = window.innerWidth < 700
    handleListResize()
    if (viewMode === 'grid') {
      recomputeGrid()
    }
  }

  // --- Path input focus/blur ----------------------------------------------
  const handleInputFocus = () => {
    inputFocused = true
    if (mode === 'address' && !$searchMode) {
      mode = 'address'
    }
  }

  const handleInputBlur = () => {
    inputFocused = false
  }

  const focusPathInput = () => {
    if (pathInputEl) {
      pathInputEl.focus()
      pathInputEl.select()
    }
  }

  const blurPathInput = () => {
    pathInputEl?.blur()
  }

  const canUseSearch = () => currentView === 'dir'

  const setSearchModeState = async (value: boolean) => {
    if (value && !canUseSearch()) {
      mode = 'filter'
      searchActive.set(false)
      if ($searchMode) {
        await toggleMode(false)
      }
      pathInput = ''
      filter.set('')
      return
    }
    pathInput = ''
    await toggleMode(value)
    mode = value ? 'search' : 'address'
    if (!value) {
      searchActive.set(false)
      filter.set('')
      pathInput = $current
    }
  }

  const navigateToBreadcrumb = async (path: string) => {
    if (currentView !== 'dir') {
      return
    }
    mode = 'address'
    searchActive.set(false)
    filter.set('')
    if ($searchMode) {
      await toggleMode(false)
    }
    pathInput = path
    await goToPath(path)
  }

  const shortcuts = createGlobalShortcuts({
    isBookmarkModalOpen: () => get(bookmarkStore).open,
    searchMode: () => $searchMode,
    setSearchMode: async (value: boolean) => setSearchModeState(value),
    focusPath: () => focusPathInput(),
    blurPath: () => blurPathInput(),
    isShortcut,
    onToggleHidden: () => Promise.resolve(toggleShowHidden()),
    onTypeChar: async (char) => {
      if (inputFocused && mode === 'address' && canUseSearch()) {
        return false
      }
      if (($searchMode || mode === 'search') && canUseSearch()) {
        mode = 'search'
        pathInput = `${pathInput}${char}`
        searchActive.set(pathInput.length > 0)
        focusPathInput()
        return true
      }
      if (mode !== 'filter') {
        mode = 'filter'
        if ($searchMode) {
          await toggleMode(false)
        }
        pathInput = ''
      }
      pathInput = `${pathInput}${char}`
      filter.set(pathInput)
      searchActive.set(pathInput.length > 0)
      focusPathInput()
      return true
    },
    onRemoveChar: async () => {
      if (mode === 'address') {
        return false
      }
      if (mode === 'search') {
        if (pathInput.length === 0) {
          await enterAddressMode()
          focusPathInput()
          return true
        }
        pathInput = pathInput.slice(0, -1)
        searchActive.set(pathInput.length > 0)
        focusPathInput()
        return true
      }
      if (mode === 'filter') {
        if (pathInput.length <= 1) {
          await enterAddressMode()
          blurPathInput()
          return true
        }
        pathInput = pathInput.slice(0, -1)
        filter.set(pathInput)
        searchActive.set(pathInput.length > 0)
        focusPathInput()
        return true
      }
      return false
    },
    getSelectedPaths: () => Array.from($selected),
    findEntryByPath: (path: string) => $entries.find((e) => e.path === path) ?? null,
    openBookmarkModal: async (entry) => openBookmarkModal(entry as Entry),
    goBack,
    goForward,
    onCopy: async () => {
      const paths = Array.from($selected)
      if (paths.length === 0) return false
      const entries = $filteredEntries.filter((e) => paths.includes(e.path))
      const result = await clipboard.copy(entries, { writeText: true })
      if (!result.ok) {
        showToast(`Copy failed: ${result.error}`)
        return false
      }
      try {
        await copyPathsToSystemClipboard(paths)
        showToast('Copied', 1500)
      } catch (err) {
        showToast(
          `Copied (system clipboard unavailable: ${err instanceof Error ? err.message : String(err)})`,
          2500
        )
      }
      return true
    },
    onCut: async () => {
      const paths = Array.from($selected)
      if (paths.length === 0) return false
      const entries = $filteredEntries.filter((e) => paths.includes(e.path))
      const result = await clipboard.cut(entries)
      if (!result.ok) {
        showToast(`Cut failed: ${result.error}`)
        return false
      }
      try {
        await copyPathsToSystemClipboard(paths, 'cut')
        showToast('Cut', 1500)
      } catch (err) {
        showToast(
          `Cut (system clipboard unavailable: ${err instanceof Error ? err.message : String(err)})`,
          2500
        )
      }
      return true
    },
    onPaste: async () => {
      if (currentView === 'recent' || currentView === 'starred') return false
      return pasteIntoCurrent()
    },
    onRename: async () => {
      if ($selected.size !== 1) return false
      const path = Array.from($selected)[0]
      const entry = $entries.find((e) => e.path === path)
      if (!entry) return false
      renameValue = entry.name
      renameModal.open(entry)
      return true
    },
    onDelete: async (permanent) => {
      const paths = Array.from($selected)
      if (paths.length === 0) return false
      const entries = $filteredEntries.filter((e) => paths.includes(e.path))
      if (entries.length === 0) return false
      const hasNetwork = entries.some((e) => e.network)

      if (permanent || (hasNetwork && currentView !== 'trash')) {
        deleteModal.open(entries)
        return true
      }
      const label = currentView === 'trash' ? 'Deleting…' : 'Moving to trash…'
      const total = entries.length
      await activityApi.cleanup()
      activity.set({ label, percent: total > 0 ? 0 : null })
      try {
        if (currentView === 'trash') {
          let done = 0
          for (const p of entries.map((e) => e.path)) {
            await deleteEntry(p)
            done += 1
            activity.set({
              label,
              percent: total > 0 ? Math.round((done / total) * 100) : null,
            })
          }
        } else {
          const paths = entries.map((e) => e.path)
          const progressEvent = `trash-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
          await activityApi.start(label, progressEvent)
          await moveToTrashMany(paths, progressEvent)
        }
        await reloadCurrent()
      } catch (err) {
        console.error('Failed to move to trash', err)
        showToast(
          `Move to trash failed: ${err instanceof Error ? err.message : String(err)}`,
          3000
        )
      } finally {
        if (currentView === 'trash') {
          activityApi.hideSoon()
        } else {
          const hadTimer = activityApi.hasHideTimer()
          await activityApi.cleanup(true)
          if (!hadTimer) {
            activityApi.clearNow()
          }
        }
      }
      return true
    },
    onDeletePermanentFast: async () => {
      const sel = get(selected)
      if (sel.size === 0) return false
      const list = get(filteredEntries).filter((e) => sel.has(e.path))
      if (list.length === 0) return false
      if (get(explorer.confirmDelete)) {
        modalActions.confirmDelete(list)
        return true
      }
      try {
        const paths = list.map((e) => e.path)
        await deleteEntries(paths)
        await reloadCurrent()
        showToast('Deleted')
        return true
      } catch (err) {
        showToast(`Delete failed: ${err instanceof Error ? err.message : String(err)}`)
        return false
      }
    },
    onProperties: async () => {
      if ($selected.size === 0) return false
      const selection = $entries.filter((e) => $selected.has(e.path))
      if (selection.length === 0) return false
      void propertiesModal.open(selection)
      return true
    },
    onOpenConsole: async () => {
      if (currentView !== 'dir') return false
      try {
        await openConsole(get(current))
        return true
      } catch (err) {
        showToast(`Open console failed: ${err instanceof Error ? err.message : String(err)}`)
        return false
      }
    },
    onToggleView: async () => toggleViewMode(),
    onSelectAll: async () => {
      const list = get(filteredEntries)
      if (list.length === 0) return false
      selected.set(new Set(list.map((entry) => entry.path)))
      anchorIndex.set(0)
      caretIndex.set(list.length - 1)
      return true
    },
    onUndo: async () => {
      try {
        await undoAction()
        showToast('Undo')
        await reloadCurrent()
        return true
      } catch (err) {
        showToast(`Undo failed: ${err instanceof Error ? err.message : String(err)}`)
        return false
      }
    },
    onRedo: async () => {
      try {
        await redoAction()
        showToast('Redo')
        await reloadCurrent()
        return true
      } catch (err) {
        showToast(`Redo failed: ${err instanceof Error ? err.message : String(err)}`)
        return false
      }
    },
    onToggleSettings: async () => {
      settingsOpen = !settingsOpen
      return true
    },
  })
  const { handleGlobalKeydown } = shortcuts

  const handleDocumentKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Control' || event.key === 'Meta') {
      copyModifierActive = true
    }
    if (event.defaultPrevented) {
      return
    }
    const key = event.key.toLowerCase()
    const inRows = rowsElRef?.contains(event.target as Node) ?? false
    const blockingModalOpen = get(anyModalOpenStore)

    if (blockingModalOpen && key !== 'escape') {
      return
    }

    if ((event.ctrlKey || event.metaKey) && !isEditableTarget(event.target)) {
      if (key === 'control' || key === 'meta' || key === 'alt' || key === 'shift') {
        return
      }
      if (event.shiftKey && !event.altKey && key === 'i') {
        return
      }
      if (!matchesAnyShortcut(event, shortcutBindings)) {
        event.preventDefault()
        event.stopPropagation()
        return
      }
    }

    if (
      key === 'tab' &&
      mode === 'filter' &&
      !event.shiftKey &&
      rowsElRef &&
      !inRows &&
      pathInput.length > 0
    ) {
      const list = get(filteredEntries)
      if (list.length > 0) {
        event.preventDefault()
        event.stopPropagation()
        selected.set(new Set([list[0].path]))
        anchorIndex.set(0)
        caretIndex.set(0)
        const selector =
          viewMode === 'grid'
            ? `.card[data-index=\"0\"]`
            : `.row-viewport .row[data-index=\"0\"]`
        const targetEl = rowsElRef.querySelector<HTMLElement>(selector)
        if (targetEl) {
          targetEl.focus()
          targetEl.scrollIntoView({ block: 'nearest' })
        } else {
          rowsElRef.focus()
        }
        if (viewMode === 'grid') {
          ensureGridVisible(0)
        }
        return
      }
    }

    if (key === 'enter' && !isEditableTarget(event.target) && !inRows) {
      const list = get(filteredEntries)
      if (list.length > 0 && get(selected).size > 0) {
        event.preventDefault()
        event.stopPropagation()
        const sel = get(selected)
        let idx = list.findIndex((e) => sel.has(e.path))
        if (idx < 0) idx = 0
        anchorIndex.set(idx)
        caretIndex.set(idx)
        void handleOpenEntry(list[idx])
        return
      }
    }

    const arrowNav = key === 'arrowdown' || key === 'arrowup'
    const arrowHoriz = key === 'arrowleft' || key === 'arrowright'
    if ((arrowNav || (arrowHoriz && viewMode === 'grid')) && !isEditableTarget(event.target) && rowsElRef && !inRows) {
      const list = get(filteredEntries)
      if (list.length > 0) {
        event.preventDefault()
        event.stopPropagation()
        rowsElRef.focus()
        handleRowsKeydownCombined(event)
        return
      }
    }
    if (key === 'escape') {
      if ($deleteState.open) {
        event.preventDefault()
        event.stopPropagation()
        deleteModal.close()
        return
      }
      if ($renameState.open) {
        event.preventDefault()
        event.stopPropagation()
        renameModal.close()
        return
      }
      if ($openWithState.open) {
        event.preventDefault()
        event.stopPropagation()
        openWithModal.close()
        return
      }
      if ($propertiesState.open) {
        event.preventDefault()
        event.stopPropagation()
        propertiesModal.close()
        return
      }
      if ($compressState.open) {
        event.preventDefault()
        event.stopPropagation()
        compressModal.close()
        return
      }
      if ($checkDuplicatesState.open) {
        event.preventDefault()
        event.stopPropagation()
        closeCheckDuplicatesModal()
        return
      }
      if ($newFolderState.open) {
        event.preventDefault()
        event.stopPropagation()
        newFolderModal.close()
        return
      }
      if ($newFileState.open) {
        event.preventDefault()
        event.stopPropagation()
        newFileModal.close()
        return
      }
      if (bookmarkModalOpen) {
        event.preventDefault()
        event.stopPropagation()
        closeBookmarkModal()
        return
      }
      if ($contextMenu.open) {
        event.preventDefault()
        event.stopPropagation()
        closeContextMenu()
        return
      }
      if ($blankMenu.open) {
        event.preventDefault()
        event.stopPropagation()
        closeBlankContextMenu()
        return
      }
      if (blockingModalOpen) {
        return
      }
      if (mode === 'filter') {
        event.preventDefault()
        event.stopPropagation()
        void enterAddressMode()
      }
      if ($searchMode || mode === 'search') {
        event.preventDefault()
        event.stopPropagation()
        void setSearchModeState(false)
        return
      }
      if (inRows) {
        event.preventDefault()
        event.stopPropagation()
        selected.set(new Set())
        anchorIndex.set(null)
        caretIndex.set(null)
        return
      }
      if (mode === 'filter') {
        event.preventDefault()
        event.stopPropagation()
        void enterAddressMode()
        return
      }
      const hasSelection = get(selected).size > 0
      if (hasSelection) {
        selected.set(new Set())
        anchorIndex.set(null)
        caretIndex.set(null)
      }
    }
    if (blockingModalOpen) {
      return
    }
    void handleGlobalKeydown(event)
  }

  const handleDocumentKeyup = (event: KeyboardEvent) => {
    if (event.key === 'Control' || event.key === 'Meta') {
      copyModifierActive = false
    }
  }

  const isTextTarget = (target: EventTarget | null): target is HTMLElement => {
    if (!(target instanceof HTMLElement)) return false
    if (target.isContentEditable) return true
    if (target instanceof HTMLInputElement) {
      return !['button', 'submit', 'reset', 'checkbox', 'radio', 'file'].includes(target.type)
    }
    return target instanceof HTMLTextAreaElement
  }

  const handleDocumentContextMenu = (event: MouseEvent) => {
    if (!isTextTarget(event.target)) return
    event.preventDefault()
    event.stopPropagation()
    textMenuTarget = event.target as HTMLElement
    textMenuReadonly =
      (textMenuTarget instanceof HTMLInputElement || textMenuTarget instanceof HTMLTextAreaElement) &&
      (textMenuTarget.readOnly || textMenuTarget.disabled)
    textMenuOpen = true
    textMenuX = event.clientX
    textMenuY = event.clientY
  }

  $: updateViewportHeight()
  $: {
    if (viewMode === 'list') {
      // Recompute virtualization when viewport or scroll changes.
      $viewportHeight
      $scrollTop
      recompute($filteredEntries)
    } else {
      // Ensure grid virtualization reruns when entry set changes.
      $filteredEntries
      recomputeGrid()
    }
  }

  const setupRowsObserver = () => {
    if (!rowsElRef || typeof ResizeObserver === 'undefined') return
    rowsObserver?.disconnect()
    rowsObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (entry.contentRect.height > 0) {
          if (rowsRaf === null) {
            rowsRaf = requestAnimationFrame(() => {
              rowsRaf = null
              updateViewportHeight()
            })
          }
        }
      }
    })
    rowsObserver.observe(rowsElRef)
  }

  $: {
    if (rowsElRef && viewMode === 'list') {
      setupRowsObserver()
      updateViewportHeight()
    }
  }

  const setupGridObserver = () => {
    if (!gridElRef || viewMode !== 'grid' || typeof ResizeObserver === 'undefined') return
    gridObserver?.disconnect()
    gridObserver = new ResizeObserver(() => {
      if (gridRaf === null) {
        gridRaf = requestAnimationFrame(() => {
          gridRaf = null
          recomputeGrid()
        })
      }
    })
    gridObserver.observe(gridElRef)
  }

  $: {
    if (gridElRef && viewMode === 'grid') {
      setupGridObserver()
      recomputeGrid()
    }
  }

  // --- Lifecycle: global listeners ---------------------------------------
  onDestroy(() => {
    void stopDuplicateScan(true)
  })

  const openBookmarkModal = async (entry: Entry) => {
    bookmarkModal.openModal(entry)
    await tick()
    if (bookmarkInputEl) {
      bookmarkInputEl.focus()
      bookmarkInputEl.select()
    }
  }

  const closeBookmarkModal = () => {
    bookmarkModal.closeModal()
  }

  const confirmBookmark = () => {
    const add = (label: string, path: string) => {
      void addBookmark(label, path)
      bookmarksStore.update((list) => [...list, { label, path }])
    }
    const { bookmarks: updated } = bookmarkModal.confirm(bookmarks, add)
    bookmarks = updated
    closeBookmarkModal()
  }

  const loadAndOpenContextMenu = async (entry: Entry, event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    try {
      const selectionCount = $selected.has(entry.path) ? $selected.size : 1
      const selectionPaths = $selected.has(entry.path) ? Array.from($selected) : [entry.path]
      const selectionEntries = selectionPaths
        .map((p) => $visibleEntries.find((e) => e.path === p) || entry)
        .filter(Boolean) as Entry[]

      let actions = await invoke<ContextAction[]>('context_menu_actions', {
        count: selectionCount,
        kind: entry.kind,
        starred: Boolean(entry.starred),
        view: currentView,
        clipboardHasItems: clipboardPaths.size > 0,
      })
      actions = actions.filter((action) => action.id !== 'new-folder')
      // Drop any leading dividers that can appear after filtering
      while (actions.length > 0 && actions[0].id.startsWith('divider')) {
        actions.shift()
      }
      // Only show Extract when all selected entries are extractable archives
      const allExtractable = selectionEntries.length > 0 && selectionEntries.every(isExtractableArchive)
      if (!allExtractable) {
        actions = actions.filter((a) => a.id !== 'extract')
      }
      const inSearch = $searchMode || mode === 'search'
      if (inSearch && selectionCount === 1 && !actions.some((a) => a.id === 'open-location')) {
        actions.splice(1, 0, { id: 'open-location', label: 'Open item location' })
      }
      actions = applyContextMenuShortcuts(actions)
      if (actions.length > 0) {
        openContextMenu(entry, actions, event.clientX, event.clientY)
      }
    } catch (err) {
      console.error('Failed to load context menu actions', err)
    }
  }

  const pasteIntoCurrent = async () => {
    if (currentView === 'starred') {
      showToast('Cannot paste in Starred view')
      return false
    }

    // Always attempt to sync from system clipboard first, then paste.
    try {
      const sys = await invoke<{ mode: 'copy' | 'cut'; paths: string[] }>('system_clipboard_paths')
      if (sys.paths.length > 0) {
        await setClipboardCmd(sys.paths, sys.mode)
        const stubEntries = sys.paths.map((p) => ({
          path: p,
          name: p.split('/').pop() ?? p,
          kind: 'file',
          iconId: 12,
        }))
        setClipboardState(sys.mode, stubEntries as unknown as Entry[])
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      if (!msg.toLowerCase().includes('no file paths found')) {
        showToast(`System clipboard unavailable: ${msg}`, 2000)
      }
    }

    const ok = await handlePasteOrMove($current)
    if (ok && clipboardMode === 'cut') {
      // Clear internal and system clipboard after a successful move
      clearClipboardState()
      try {
        await setClipboardCmd([], 'copy')
        await clearSystemClipboard()
      } catch {
        // ignore; move already succeeded
      }
      clipboardPaths = new Set()
    }
    return ok
  }

  const reloadCurrent = async () => {
    if (currentView === 'recent') {
      await loadRecent(false)
      return
    }
    if (currentView === 'starred') {
      await loadStarred(false)
      return
    }
    if (currentView === 'trash') {
      await loadTrash(false)
      return
    }
    await loadRaw($current, { recordHistory: false })
  }

  const clearThumbnailCacheFromSettings = async () => {
    try {
      const result = await clearThumbnailCache()
      thumbnailRefreshToken += 1
      await reloadCurrent()
      if (result.removed_files > 0) {
        showToast(
          `Cleared thumbnail cache: ${result.removed_files} file${result.removed_files === 1 ? '' : 's'} (${formatSize(result.removed_bytes)})`,
          2200,
        )
      } else {
        showToast('Thumbnail cache already empty', 1800)
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      showToast(`Clear thumbnail cache failed: ${msg}`)
      throw err
    }
  }

  const clearStarsFromSettings = async () => {
    try {
      const removed = await clearStars()
      entries.update((list) =>
        list.map((entry) => (entry.starred ? { ...entry, starred: false } : entry)),
      )
      if (currentView === 'starred') {
        await loadStarred(false)
      }
      if (removed > 0) {
        showToast(`Cleared ${removed} star${removed === 1 ? '' : 's'}`, 1800)
      } else {
        showToast('No stars to clear', 1600)
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      showToast(`Clear stars failed: ${msg}`)
      throw err
    }
  }

  const clearBookmarksFromSettings = async () => {
    try {
      const removed = await clearBookmarks()
      bookmarksStore.set([])
      if (removed > 0) {
        showToast(`Cleared ${removed} bookmark${removed === 1 ? '' : 's'}`, 1800)
      } else {
        showToast('No bookmarks to clear', 1600)
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      showToast(`Clear bookmarks failed: ${msg}`)
      throw err
    }
  }

  const clearRecentsFromSettings = async () => {
    try {
      const removed = await clearRecents()
      if (currentView === 'recent') {
        await loadRecent(false)
      }
      if (removed > 0) {
        showToast(`Cleared ${removed} recent item${removed === 1 ? '' : 's'}`, 1800)
      } else {
        showToast('No recents to clear', 1600)
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      showToast(`Clear recents failed: ${msg}`)
      throw err
    }
  }

  const {
    deleteModal,
    deleteState,
    openWithModal,
    openWithState,
    propertiesModal,
    propertiesState,
    renameModal,
    renameState,
    advancedRenameModal,
    advancedRenameState,
    newFileModal,
    newFileState,
    newFolderModal,
    newFolderState,
    compressModal,
    compressState,
    checkDuplicatesModal,
    checkDuplicatesState,
    actions: modalActions,
  } = useModalsController({
    activityApi,
    reloadCurrent,
    showToast,
    getCurrentPath: () => get(current),
    loadPath: (path, opts) => loadRaw(path, opts),
    parentPath,
    computeDirStats,
  })

  let duplicateScanToken = 0
  let activeDuplicateProgressEvent: string | null = null
  let unlistenDuplicateProgress: UnlistenFn | null = null

  const duplicateProgressLabel = (payload: DuplicateScanProgress) => {
    if (payload.phase === 'collecting') {
      return `Scanning files: ${payload.scannedFiles} checked, ${payload.candidateFiles} candidate${payload.candidateFiles === 1 ? '' : 's'}`
    }
    if (payload.phase === 'comparing') {
      return `Comparing bytes: ${payload.comparedFiles}/${payload.candidateFiles}`
    }
    return `Finished: ${payload.matchedFiles} identical ${payload.matchedFiles === 1 ? 'file' : 'files'}`
  }

  const cleanupDuplicateProgressListener = async () => {
    if (unlistenDuplicateProgress) {
      await unlistenDuplicateProgress()
      unlistenDuplicateProgress = null
    }
    activeDuplicateProgressEvent = null
  }

  const stopDuplicateScan = async (invalidate = true) => {
    if (invalidate) {
      duplicateScanToken += 1
    }
    const cancelId = activeDuplicateProgressEvent
    checkDuplicatesModal.stopScan()
    if (cancelId) {
      try {
        await cancelTask(cancelId)
      } catch {
        // Task likely already completed or cleaned up.
      }
    }
    await cleanupDuplicateProgressListener()
  }

  const closeCheckDuplicatesModal = () => {
    void stopDuplicateScan(true)
      .catch(() => {})
      .finally(() => {
        checkDuplicatesModal.close()
      })
  }


  const isExtractableArchive = (entry: Entry) => {
    if (entry.kind !== 'file') return false
    const name = entry.name.toLowerCase()
    return (
      name.endsWith('.zip') ||
      name.endsWith('.tar') ||
      name.endsWith('.tar.gz') ||
      name.endsWith('.tgz') ||
      name.endsWith('.tar.bz2') ||
      name.endsWith('.tbz2') ||
      name.endsWith('.tar.xz') ||
      name.endsWith('.txz') ||
      name.endsWith('.tar.zst') ||
      name.endsWith('.tzst') ||
      name.endsWith('.7z') ||
      name.endsWith('.rar') ||
      name.endsWith('.gz') ||
      name.endsWith('.bz2') ||
      name.endsWith('.xz') ||
      name.endsWith('.zst')
    )
  }

  const extractEntries = async (entriesToExtract: Entry[]) => {
    if (extracting) return
    const allArchives = entriesToExtract.every(isExtractableArchive)
    if (!allArchives) {
      showToast('Extraction available for archive files only')
      return
    }
    if (entriesToExtract.length === 0) return

    extracting = true
    const progressEvent = `extract-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    await activityApi.start(
      `Extracting${entriesToExtract.length > 1 ? ` ${entriesToExtract.length} items…` : '…'}`,
      progressEvent,
      () => activityApi.requestCancel(progressEvent)
    )

    const summarize = (skippedSymlinks: number, skippedOther: number) => {
      const skipParts = []
      if (skippedSymlinks > 0) skipParts.push(`${skippedSymlinks} symlink${skippedSymlinks === 1 ? '' : 's'}`)
      if (skippedOther > 0) skipParts.push(`${skippedOther} entr${skippedOther === 1 ? 'y' : 'ies'}`)
      return skipParts.length > 0 ? ` (skipped ${skipParts.join(', ')})` : ''
    }

    try {
      if (entriesToExtract.length === 1) {
        const entry = entriesToExtract[0]
        const result = await invoke<ExtractResult>('extract_archive', {
          path: entry.path,
          progressEvent,
        })
        if (get(openDestAfterExtract) && result?.destination) {
          await loadRaw(result.destination, { recordHistory: true })
        } else {
          await reloadCurrent()
        }
        const suffix = summarize(result?.skipped_symlinks ?? 0, result?.skipped_entries ?? 0)
        showToast(`Extracted to ${result.destination}${suffix}`)
      } else {
        const result = await invoke<ExtractBatchItem[]>('extract_archives', {
          paths: entriesToExtract.map((e) => e.path),
          progressEvent,
        })
        const successes = result.filter((r) => r.ok && r.result)
        const failures = result.filter((r) => !r.ok)
        if (get(openDestAfterExtract)) {
          const firstDest = successes.find((r) => r.result?.destination)?.result?.destination
          if (firstDest) {
            await loadRaw(firstDest, { recordHistory: true })
          } else {
            await reloadCurrent()
          }
        } else {
          await reloadCurrent()
        }
        const totalSkippedSymlinks = successes.reduce(
          (n, r) => n + (r.result?.skipped_symlinks ?? 0),
          0
        )
        const totalSkippedOther = successes.reduce(
          (n, r) => n + (r.result?.skipped_entries ?? 0),
          0
        )
        const suffix = summarize(totalSkippedSymlinks, totalSkippedOther)
        if (failures.length === 0) {
          showToast(`Extracted ${successes.length} archives${suffix}`)
        } else if (successes.length === 0) {
          showToast(`Extraction failed for ${failures.length} archives`)
        } else {
          showToast(`Extracted ${successes.length} archives, ${failures.length} failed${suffix}`)
        }
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      if (msg.toLowerCase().includes('cancelled')) {
        showToast('Extraction cancelled')
      } else {
        showToast(`Failed to extract: ${msg}`)
      }
    } finally {
      extracting = false
      activityApi.clearNow()
      await activityApi.cleanup()
    }
  }

  const contextActions = createContextActions({
    getSelectedPaths: () => Array.from($selected),
    getSelectedSet: () => $selected,
    getFilteredEntries: () => $filteredEntries,
    currentView: () => currentView,
    confirmDeleteEnabled: () => get(explorer.confirmDelete),
    reloadCurrent,
    clipboard,
    showToast,
    openWith: (entry) => modalActions.openWith(entry),
    openCompress: (entries) => {
      compressName = modalActions.openCompress(entries, get(archiveName))
      compressLevel = get(archiveLevel)
    },
    openCheckDuplicates: (entry) => {
      modalActions.openCheckDuplicates(entry)
    },
    extractEntries: (entries) => extractEntries(entries),
    startRename: (entry) => {
      renameValue = entry.name
      modalActions.startRename(entry)
    },
    startAdvancedRename: (entries) => {
      advancedRenameModal.open(entries)
    },
    confirmDelete: (entries) => modalActions.confirmDelete(entries),
    openProperties: (entries) => {
      void modalActions.openProperties(entries)
    },
    openLocation: (entry) => {
      void openEntryLocation(entry)
    },
  })

  const copyCheckDuplicatesList = async () => {
    const list = $checkDuplicatesState.duplicates
    if (list.length === 0) return
    const payload = list.join('\n')
    try {
      await navigator.clipboard.writeText(payload)
      showToast('Duplicate list copied', 1500)
      return
    } catch {
      // Fallback for environments with blocked clipboard API.
    }

    try {
      const ta = document.createElement('textarea')
      ta.value = payload
      ta.setAttribute('readonly', 'true')
      ta.style.position = 'fixed'
      ta.style.left = '-9999px'
      ta.style.top = '0'
      document.body.appendChild(ta)
      ta.select()
      const ok = document.execCommand('copy')
      document.body.removeChild(ta)
      if (!ok) {
        throw new Error('copy command failed')
      }
      showToast('Duplicate list copied', 1500)
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      showToast(`Copy failed: ${msg}`)
    }
  }

  const searchCheckDuplicates = async () => {
    const target = $checkDuplicatesState.target
    const searchRoot = $checkDuplicatesState.searchRoot.trim()

    if (!target) {
      showToast('No target file selected')
      return
    }
    if (!searchRoot) {
      showToast('Choose a start folder first')
      return
    }
    if ($checkDuplicatesState.scanning) {
      return
    }

    await stopDuplicateScan(true)
    const runToken = ++duplicateScanToken
    const progressEvent = `duplicates-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    activeDuplicateProgressEvent = progressEvent
    checkDuplicatesModal.startScan()

    try {
      unlistenDuplicateProgress = await listen<DuplicateScanProgress>(progressEvent, (event) => {
        if (runToken !== duplicateScanToken) {
          return
        }

        const payload = event.payload
        checkDuplicatesModal.setProgress(payload.percent, duplicateProgressLabel(payload))

        if (payload.error) {
          checkDuplicatesModal.failScan(payload.error)
          showToast(`Duplicate scan failed: ${payload.error}`)
          if (activeDuplicateProgressEvent === progressEvent) {
            void cleanupDuplicateProgressListener()
          }
          return
        }

        if (!payload.done) {
          return
        }

        const paths = payload.duplicates ?? []
        checkDuplicatesModal.finishScan(paths)
        if (paths.length === 0) {
          showToast('No identical files found', 1600)
        } else {
          showToast(
            `Found ${paths.length} identical ${paths.length === 1 ? 'file' : 'files'}`,
            1800,
          )
        }

        if (activeDuplicateProgressEvent === progressEvent) {
          void cleanupDuplicateProgressListener()
        }
      })

      if (runToken !== duplicateScanToken) {
        await cleanupDuplicateProgressListener()
        return
      }

      await checkDuplicatesStream({
        targetPath: target.path,
        startPath: searchRoot,
        progressEvent,
      })
    } catch (err) {
      if (runToken !== duplicateScanToken) {
        return
      }
      const msg = err instanceof Error ? err.message : String(err)
      checkDuplicatesModal.failScan(msg)
      showToast(`Duplicate scan failed: ${msg}`)
      await cleanupDuplicateProgressListener()
    }
  }

  const handleOpenEntry = async (entry: Entry) => {
    pendingOpenCandidate = null
    if (entry.kind === 'dir') {
      mode = 'address'
      searchActive.set(false)
      filter.set('')
      if ($searchMode) {
        await toggleMode(false)
      }
      pathInput = entry.path
      await loadDir(entry.path)
      return
    }
    if (isExtractableArchive(entry)) {
      await extractEntries([entry])
      return
    }
    open(entry)
  }

  let pendingOpenCandidate: { path: string; atMs: number } | null = null

  const isOpenClickCandidate = (event: MouseEvent) =>
    event.button === 0 &&
    !event.shiftKey &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.altKey

  const clickTimestampMs = (event: MouseEvent) => {
    const stamp = Number(event.timeStamp)
    return Number.isFinite(stamp) ? stamp : performance.now()
  }

  const resolveDoubleClickMs = () => {
    const configured = get(doubleClickMs)
    return Math.min(600, Math.max(150, Math.round(configured)))
  }

  const handleRowClickWithOpen = (entry: Entry, absoluteIndex: number, event: MouseEvent) => {
    rowSelectionHandler(entry, absoluteIndex, event)

    if (!isOpenClickCandidate(event)) {
      pendingOpenCandidate = null
      return
    }

    const nowMs = clickTimestampMs(event)
    const thresholdMs = resolveDoubleClickMs()

    if (
      pendingOpenCandidate &&
      pendingOpenCandidate.path === entry.path &&
      nowMs - pendingOpenCandidate.atMs <= thresholdMs
    ) {
      pendingOpenCandidate = null
      void handleOpenEntry(entry)
      return
    }

    pendingOpenCandidate = { path: entry.path, atMs: nowMs }
  }

  const openEntryLocation = async (entry: Entry) => {
    const dir = parentPath(entry.path)
    await loadDir(dir)
    const idx = get(entries).findIndex((e) => e.path === entry.path)
    if (idx >= 0) {
      selected.set(new Set([entry.path]))
      anchorIndex.set(idx)
      caretIndex.set(idx)
    } else {
      selected.set(new Set())
      anchorIndex.set(null)
      caretIndex.set(null)
    }
  }

  // --- Pointer handlers (list & grid) -------------------------------------
  const handleRowsMouseDown = (event: MouseEvent) => {
    const target = event.target as HTMLElement | null
    if (viewMode === 'list') {
      if (!rowsElRef) return
      if (isScrollbarClick(event, rowsElRef)) return
      if (inLassoGutter(event, rowsElRef)) return
      if (target && target.closest('.row')) return
      event.preventDefault()
      rowsElRef.focus()
      const list = get(filteredEntries)
      if (list.length === 0) return
      selectionDrag = false
      const additive = event.ctrlKey || event.metaKey
      const subtractive = !additive && event.shiftKey
      const baseSelection = get(selected)
      const baseAnchor = get(anchorIndex)
      const baseCaret = get(caretIndex)
      selectionBox.start(event, {
        rowsEl: rowsElRef,
        headerEl: headerElRef,
        entries: list,
        rowHeight: $rowHeight,
        onSelect: (paths, anchor, caret) => {
          if (subtractive) {
            const next = new Set(baseSelection)
            for (const path of paths) next.delete(path)
            const anchorPath = baseAnchor !== null ? list[baseAnchor]?.path : null
            const caretPath = baseCaret !== null ? list[baseCaret]?.path : null
            anchorIndex.set(anchorPath && next.has(anchorPath) ? baseAnchor : null)
            caretIndex.set(caretPath && next.has(caretPath) ? baseCaret : null)
            selected.set(next)
          } else if (additive) {
            const merged = new Set(baseSelection)
            for (const path of paths) merged.add(path)
            selected.set(merged)
            anchorIndex.set(baseAnchor ?? anchor)
            caretIndex.set(baseCaret ?? caret)
          } else {
            selected.set(paths)
            anchorIndex.set(anchor)
            caretIndex.set(caret)
          }
        },
        onEnd: (didDrag) => {
          selectionDrag = didDrag
        },
      })
      return
    }

    // Grid mode lasso selection
    const gridEl = event.currentTarget as HTMLDivElement | null
    if (!gridEl) return
    if (isScrollbarClick(event, gridEl)) return
    if (inLassoGutter(event, gridEl)) return
    if (target && target.closest('.card')) return
    const gridEntries = get(filteredEntries)
    if (gridEntries.length === 0) return
    const { paddingLeft: gridPaddingLeft, paddingTop: gridPaddingTop } = getGridPadding(gridEl)
    event.preventDefault()
    // When clicking blank space in grid mode, leave address edit mode and focus the grid.
    blurPathInput()
    gridEl.focus()
    selectionDrag = false
    const additive = event.ctrlKey || event.metaKey
    const subtractive = !additive && event.shiftKey
    const baseSelection = get(selected)
    const baseAnchor = get(anchorIndex)
    const baseCaret = get(caretIndex)
      selectionBox.start(event, {
        rowsEl: gridEl,
        headerEl: null,
        entries: gridEntries,
        rowHeight: 1,
        hitTest: (rect) =>
        hitTestGridVirtualized(rect, gridEntries, {
          gridCols: getGridCols(),
          cardWidth: gridCardWidth,
          cardHeight: gridRowHeight,
          gap: gridGap,
          paddingLeft: gridPaddingLeft,
          paddingTop: gridPaddingTop,
        }),
      onSelect: (paths, anchor, caret) => {
        if (subtractive) {
          const next = new Set(baseSelection)
          for (const path of paths) next.delete(path)
          const anchorPath = baseAnchor !== null ? gridEntries[baseAnchor]?.path : null
          const caretPath = baseCaret !== null ? gridEntries[baseCaret]?.path : null
          anchorIndex.set(anchorPath && next.has(anchorPath) ? baseAnchor : null)
          caretIndex.set(caretPath && next.has(caretPath) ? baseCaret : null)
          selected.set(next)
        } else if (additive) {
          const merged = new Set(baseSelection)
          for (const path of paths) merged.add(path)
          selected.set(merged)
          anchorIndex.set(baseAnchor ?? anchor ?? null)
          caretIndex.set(baseCaret ?? caret ?? null)
        } else {
          selected.set(paths)
          anchorIndex.set(anchor ?? null)
          caretIndex.set(caret ?? null)
        }
      },
      onEnd: (didDrag) => {
        selectionDrag = didDrag
      },
    })
  }

  // --- Scroll / wheel routing --------------------------------------------
  const handleRowsScrollCombined = (event: Event) => {
    if (viewMode === 'list') {
      handleRowsScroll()
    } else {
      handleGridScroll()
    }
  }

  // --- Keyboard navigation (grid) -----------------------------------------
  const handleGridKeydown = createGridKeyboardHandler({
    getFilteredEntries: () => get(filteredEntries),
    selected,
    anchorIndex,
    caretIndex,
    getGridCols,
    ensureGridVisible,
    handleOpenEntry,
  })

  const handleRowsKeydownCombined = (event: KeyboardEvent) => {
    if (viewMode === 'list') {
      rowsKeydownHandler(event)
    } else {
      handleGridKeydown(event)
    }
  }

  const handleWheelCombined = (event: WheelEvent) => {
    if (viewMode === 'list') {
      handleWheel(event)
    } else {
      handleGridWheel(event)
    }
  }

  // --- Click handling (row/grid blank vs cards) ---------------------------
  const handleRowsClickSafe = (event: MouseEvent) => {
    if (selectionDrag) {
      selectionDrag = false
      pendingOpenCandidate = null
      return
    }
    if (isScrollbarClick(event, rowsElRef)) return
    if (viewMode === 'grid') {
      const target = event.target as HTMLElement | null
      if (target && target.closest('.card')) return
      pendingOpenCandidate = null
      if (get(selected).size > 0) {
        selected.set(new Set())
        anchorIndex.set(null)
        caretIndex.set(null)
      }
      return
    }
    pendingOpenCandidate = null
    handleRowsClick(event)
  }

  // --- Context menus ------------------------------------------------------
  const handleRowContextMenu = (entry: Entry, event: MouseEvent) => {
    pendingOpenCandidate = null
    const idx = get(filteredEntries).findIndex((e) => e.path === entry.path)
    ensureSelectionBeforeMenu($selected, entry.path, idx, (paths, anchor, caret) => {
      selected.set(paths)
      anchorIndex.set(anchor)
      caretIndex.set(caret)
    })
    void loadAndOpenContextMenu(entry, event)
  }

  const handleBlankContextMenu = (event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    if (currentView !== 'dir') {
      selected.set(new Set())
      anchorIndex.set(null)
      caretIndex.set(null)
      closeBlankContextMenu()
      return
    }
    const hasClipboardItems = clipboardPaths.size > 0
    const openConsoleShortcut =
      shortcutFor(shortcutBindings, 'open_console')?.accelerator ?? 'Ctrl+T'
    const pasteShortcut =
      shortcutFor(shortcutBindings, 'paste')?.accelerator ?? 'Ctrl+V'
    selected.set(new Set())
    anchorIndex.set(null)
    caretIndex.set(null)
    const actions: ContextAction[] = [
      { id: 'new-file', label: 'New File…' },
      { id: 'new-folder', label: 'New Folder…' },
      { id: 'open-console', label: 'Open in console', shortcut: openConsoleShortcut },
    ]
    if (hasClipboardItems) {
      actions.push({ id: 'paste', label: 'Paste', shortcut: pasteShortcut })
    }
    openBlankContextMenu(actions, event.clientX, event.clientY)
  }

  const handleBlankContextAction = async (id: string) => {
    if (currentView !== 'dir') return
    closeBlankContextMenu()
    if (id === 'new-folder') {
      newFolderName = newFolderModal.open()
      return
    }
    if (id === 'new-file') {
      newFileName = newFileModal.open()
      return
    }
    if (id === 'open-console') {
      try {
        await openConsole(get(current))
      } catch (err) {
        showToast(`Open console failed: ${err instanceof Error ? err.message : String(err)}`)
      }
      return
    }
    if (id === 'paste') {
      await pasteIntoCurrent()
    }
  }

  const handleContextSelect = async (id: string) => {
    const entry = $contextMenu.entry
    closeContextMenu()
    if (id === 'new-folder') {
      if (currentView !== 'dir') {
        showToast('Cannot create folder here')
        return
      }
      newFolderName = newFolderModal.open()
      return
    }
    if (id === 'new-file') {
      if (currentView !== 'dir') {
        showToast('Cannot create file here')
        return
      }
      newFileName = newFileModal.open()
      return
    }
    if (id === 'rename-advanced') {
      const selectedPaths = $selected.size > 0 ? Array.from($selected) : entry ? [entry.path] : []
      const entries =
        selectedPaths.length > 0
          ? $filteredEntries.filter((e) => selectedPaths.includes(e.path))
          : entry
            ? [entry]
            : []
      if (entries.length > 1) {
        advancedRenameModal.open(entries)
      } else {
        modalActions.startRename(entry!)
      }
      return
    }
    await contextActions(id, entry)
  }

  let dragPaths: string[] = []
  let copyModifierActive = false
  let nativeDragActive = false
  let textMenuOpen = false
  let textMenuX = 0
  let textMenuY = 0
  let textMenuTarget: HTMLElement | null = null
  let textMenuReadonly = false
  const nativeDrop = createNativeFileDrop({
    onDrop: async (paths) => {
      if (!paths || paths.length === 0) return
      const curr = get(current)
      const view = viewFromPath(curr ?? '')
      // If we’re in a directory view, treat external drop as a copy-into-current.
      if (view === 'dir' && curr) {
        try {
          await setClipboardCmd(paths, 'copy')
          const ok = await handlePasteOrMove(curr)
          if (ok) {
            showToast(`Pasted ${paths.length} item${paths.length === 1 ? '' : 's'}`)
          }
        } catch (err) {
          showToast(`Drop failed: ${err instanceof Error ? err.message : String(err)}`)
        }
        return
      }
      // Otherwise, navigate to the parent of the first path and select it.
      const first = paths[0]
      const dest = parentPath(first)
      await loadDir(dest)
      const list = get(entries)
      const match = list.find((e) => normalizePath(e.path) === normalizePath(first))
      if (match) {
        selected.set(new Set([match.path]))
        anchorIndex.set(list.findIndex((e) => e.path === match.path))
        caretIndex.set(list.findIndex((e) => e.path === match.path))
      }
      showToast('Dropped item navigated')
    },
  })
  const nativeDropHovering = nativeDrop.hovering
  const nativeDropPosition = nativeDrop.position
  $: if ($nativeDropHovering && currentView === 'dir') {
    showToast('Drop to paste into this folder', 1500)
  }
  const handleRowDragStart = (entry: Entry, event: DragEvent) => {
    const selectedPaths = $selected.has(entry.path) ? Array.from($selected) : [entry.path]
    const nativeCopy = event.ctrlKey || event.metaKey
    if (event.altKey) {
      nativeDragActive = true
      event.preventDefault()
      event.stopPropagation()
      void startNativeFileDrag(selectedPaths, nativeCopy ? 'copy' : 'move').then((ok) => {
        if (!ok) showToast('Native drag failed')
      })
      event.preventDefault()
      return
    }
    nativeDragActive = false
    dragPaths = selectedPaths
    dragDrop.start(selectedPaths, event)
    if (!event.ctrlKey && !event.metaKey) {
      copyModifierActive = false
    }
    dragAction = null
  }

  const handleRowDragEnd = () => {
    dragPaths = []
    dragDrop.end()
    dragAction = null
    nativeDragActive = false
  }

  const handleRowDragOver = (entry: Entry, event: DragEvent) => {
    if (entry.kind !== 'dir') return
    const allowed = dragDrop.canDropOn(dragPaths, entry.path)
    dragDrop.setTarget(allowed ? entry.path : null)
    if (event.dataTransfer) {
      const copy = allowed ? shouldCopyForDrop(entry.path, event) : false
      event.dataTransfer.dropEffect = allowed ? (copy ? 'copy' : 'move') : 'none'
    }
    dragAction = allowed ? (shouldCopyForDrop(entry.path, event) ? 'copy' : 'move') : null
    dragDrop.setPosition(event.clientX, event.clientY)
  }

  const handleRowDragEnter = (entry: Entry, event: DragEvent) => {
    handleRowDragOver(entry, event)
  }

  const handleRowDragLeave = (entry: Entry, event: DragEvent) => {
    const target = event.currentTarget as HTMLElement | null
    const related = event.relatedTarget as HTMLElement | null
    if (target && related && target.contains(related)) {
      return
    }
    if (dragDrop.canDropOn(dragPaths, entry.path)) {
      dragDrop.setTarget(null)
      dragAction = null
    }
  }

  const handleRowDrop = async (entry: Entry, event: DragEvent) => {
    if (entry.kind !== 'dir') return
    if (!dragDrop.canDropOn(dragPaths, entry.path)) return
    event.preventDefault()
    try {
      const copy = shouldCopyForDrop(entry.path, event)
      const mode: 'copy' | 'cut' = copy ? 'copy' : 'cut'
      if (dragPaths.length > 0) {
        await setClipboardCmd(dragPaths, mode)
      }
      await handlePasteOrMove(entry.path)
    } catch (err) {
      showToast(`Move failed: ${err instanceof Error ? err.message : String(err)}`)
    } finally {
      handleRowDragEnd()
    }
  }


  const handleBreadcrumbDragOver = (path: string, event: DragEvent) => {
    if (dragPaths.length === 0) return
    const allowed = dragDrop.canDropOn(dragPaths, path)
    dragDrop.setTarget(allowed ? path : null)
    if (event.dataTransfer) {
      const copy = allowed ? shouldCopyForDrop(path, event) : false
      event.dataTransfer.dropEffect = allowed ? (copy ? 'copy' : 'move') : 'none'
    }
    dragAction = allowed ? (shouldCopyForDrop(path, event) ? 'copy' : 'move') : null
    dragDrop.setPosition(event.clientX, event.clientY)
    event.preventDefault()
  }

  const handleBreadcrumbDragLeave = (path: string) => {
    if (get(dragState).target === path) {
      dragDrop.setTarget(null)
    }
    dragAction = null
  }

  const handleBreadcrumbDrop = async (path: string, event: DragEvent) => {
    if (dragPaths.length === 0) return
    if (!dragDrop.canDropOn(dragPaths, path)) return
    event.preventDefault()
    try {
      const copy = shouldCopyForDrop(path, event)
      const mode: 'copy' | 'cut' = copy ? 'copy' : 'cut'
      await setClipboardCmd(dragPaths, mode)
      await handlePasteOrMove(path)
    } catch (err) {
      showToast(`Move failed: ${err instanceof Error ? err.message : String(err)}`)
    } finally {
      handleRowDragEnd()
    }
  }

  const handlePasteOrMove = async (dest: string) => {
    const runPaste = async (target: string, policy: 'rename' | 'overwrite' = 'rename') => {
      const progressEvent = `copy-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
      try {
        await activityApi.start('Copying…', progressEvent, () => activityApi.requestCancel(progressEvent))
        await pasteClipboardCmd(target, policy, progressEvent)
        await reloadCurrent()
        activityApi.hideSoon()
        return true
      } catch (err) {
        activityApi.clearNow()
        await activityApi.cleanup()
        showToast(`Paste failed: ${err instanceof Error ? err.message : String(err)}`)
        return false
      }
    }
    try {
      const conflicts = await pasteClipboardPreview(dest)
      if (conflicts && conflicts.length > 0) {
        const destNorm = normalizePath(dest)
        const selfPaste = conflicts.every((c) => normalizePath(parentPath(c.src)) === destNorm)
        if (selfPaste) {
          return await runPaste(dest, 'rename')
        }
        conflictList = conflicts
        conflictDest = dest
        conflictModalOpen = true
        return false
      }
      return await runPaste(dest, 'rename')
    } catch (err) {
      showToast(`Paste failed: ${err instanceof Error ? err.message : String(err)}`)
      return false
    }
  }

  const resolveConflicts = async (policy: 'rename' | 'overwrite') => {
    if (!conflictDest) return
    conflictModalOpen = false
    try {
      const progressEvent = `copy-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
      await activityApi.start('Copying…', progressEvent, () => activityApi.requestCancel(progressEvent))
      await pasteClipboardCmd(conflictDest, policy, progressEvent)
      await reloadCurrent()
      activityApi.hideSoon()
    } catch (err) {
      activityApi.clearNow()
      await activityApi.cleanup()
      showToast(`Paste failed: ${err instanceof Error ? err.message : String(err)}`)
    } finally {
      conflictDest = null
      conflictList = []
    }
  }

  const closeRenameModal = () => {
    renameModal.close()
    renameValue = ''
  }

  const closeNewFolderModal = () => {
    newFolderModal.close()
  }

  const closeNewFileModal = () => {
    newFileModal.close()
  }

  const confirmNewFolder = async () => {
    const created = await newFolderModal.confirm(newFolderName)
    if (!created) return
    selected.set(new Set([created]))
    anchorIndex.set(null)
    caretIndex.set(null)
  }

  const confirmNewFile = async () => {
    const created = await newFileModal.confirm(newFileName)
    if (!created) return
    selected.set(new Set([created]))
    anchorIndex.set(null)
    caretIndex.set(null)
  }

  const confirmRename = async (name: string) => {
    await renameModal.confirm(name)
  }

  const closeCompress = () => {
    compressModal.close()
  }

  const confirmCompress = async (name: string, level: number) => {
    await compressModal.confirm(name, level)
  }


  const initLifecycle = () => {
    const cleanupFns: Array<() => void> = []

    const handleError = (event: ErrorEvent) => {
      const msg = event.error instanceof Error ? event.error.message : event.message ?? 'Unknown error'
      console.error('Unhandled error', event)
      showToast(`Error: ${msg}`)
    }
    const handleRejection = (event: PromiseRejectionEvent) => {
      const reason = event.reason
      const msg = reason instanceof Error ? reason.message : String(reason)
      console.error('Unhandled rejection', reason)
      showToast(`Error: ${msg}`)
    }

    const setupCore = async () => {
      handleResize()
      window.addEventListener('resize', handleResize)
      cleanupFns.push(() => window.removeEventListener('resize', handleResize))

      window.addEventListener('error', handleError)
      window.addEventListener('unhandledrejection', handleRejection)
      cleanupFns.push(() => {
        window.removeEventListener('error', handleError)
        window.removeEventListener('unhandledrejection', handleRejection)
      })

      const prefView = await loadDefaultView().catch(() => null)
      if (prefView === 'list' || prefView === 'grid') {
        defaultViewPref = prefView
        viewMode = prefView
      }

      const savedShortcuts = await loadShortcuts().catch((err) => {
        console.error('Failed to load shortcuts', err)
        return null
      })
      if (savedShortcuts && savedShortcuts.length > 0) {
        applyShortcutBindings(savedShortcuts)
      }

      await nativeDrop.start()
      cleanupFns.push(() => {
        void nativeDrop.stop()
      })

      const unlistenMountStart = await listen<MountEvent>('mounting-started', (event) => {
        const { fs } = event.payload ?? {}
        activityApi.clearNow()
        activity.set({ label: `Mounting ${fsLabel(fs)}…`, percent: null, cancel: null, cancelling: false })
      })
      const unlistenMountDone = await listen<MountEvent>('mounting-done', (event) => {
        const { fs, ok } = event.payload ?? {}
        const label = ok ? `${fsLabel(fs)} mounted` : 'Mount failed'
        activity.set({ label, percent: null, cancel: null, cancelling: false })
        activityApi.hideSoon()
        if (ok === false) {
          showToast('Mount failed. Please try again.')
        }
      })
      cleanupFns.push(() => {
        unlistenMountStart()
        unlistenMountDone()
      })
    }

    void setupCore()

    return () => {
      rowsObserver?.disconnect()
      gridObserver?.disconnect()
      cleanupFns.forEach((fn) => fn())
      dirSizeAbort++
    }
  }

  onMount(initLifecycle)
</script>

<svelte:document
  on:keydown|capture={handleDocumentKeydown}
  on:keyup|capture={handleDocumentKeyup}
  on:contextmenu|capture={handleDocumentContextMenu}
  on:cut|capture={(e) => {
    const target = e.target as HTMLElement | null
    if (target && (target.isContentEditable || ['input', 'textarea'].includes(target.tagName.toLowerCase()))) {
      return
    }
    e.preventDefault()
    e.stopPropagation()
  }}
/>
<DragGhost
  visible={$dragState.dragging}
  x={$dragState.position.x}
  y={$dragState.position.y}
  count={$dragState.paths.length}
  allowed={$dragState.target !== null}
  action={dragAction}
/>
<TextContextMenu
  open={textMenuOpen}
  x={textMenuX}
  y={textMenuY}
  target={textMenuTarget}
  readonly={textMenuReadonly}
  shortcuts={shortcutBindings}
  onClose={() => {
    textMenuOpen = false
    textMenuTarget = null
  }}
/>
  <ExplorerShell
    bind:pathInput
    bind:pathInputEl
    bind:rowsEl={rowsElRef}
  bind:headerEl={headerElRef}
  currentPath={$current}
  bind:bookmarkName
  bind:bookmarkInputEl
  {viewMode}
  {sidebarCollapsed}
  {places}
  {bookmarks}
  {partitions}
  onPlaceSelect={handlePlace}
  onBookmarkSelect={(path) => void loadDirIfIdle(path)}
  onRemoveBookmark={(path) => {
    void removeBookmark(path)
    bookmarksStore.update((list) => list.filter((b) => b.path !== path))
  }}
  onPartitionSelect={(path) => void openPartition(path)}
  onPartitionEject={async (path) => {
    try {
      await ejectDrive(path)
      partitionsStore.update((list) =>
        list.filter((p) => p.path.trim().toUpperCase() !== path.trim().toUpperCase())
      )
      showToast(`Ejected ${path}`)
      await loadPartitions()
    } catch (err) {
      showToast(`Eject failed: ${err instanceof Error ? err.message : String(err)}`)
    }
  }}
  searchMode={$searchMode}
  loading={$loading}
  activity={$activity}
  onFocus={handleInputFocus}
  onBlur={handleInputBlur}
  onSubmitPath={submitPath}
  onSearch={() => runSearch(pathInput)}
  onExitSearch={() => void enterAddressMode().then(() => blurPathInput())}
  onNavigateSegment={(path) => void navigateToBreadcrumb(path)}
  noticeMessage={$error}
  searchActive={$searchActive}
  {mode}
  filterValue={$filter}
  videoThumbs={$videoThumbs}
  {thumbnailRefreshToken}
  cols={$cols}
  gridTemplate={$gridTemplate}
  filteredEntries={$filteredEntries}
  visibleEntries={$visibleEntries}
  start={$start}
  offsetY={$offsetY}
  totalHeight={$totalHeight}
  wide={sidebarCollapsed}
  selected={$selected}
  sortField={$sortField}
  sortDirection={$sortDirection}
  isHidden={isHidden}
  displayName={displayName}
  {formatSize}
  {formatItems}
  clipboardMode={clipboardMode}
  clipboardPaths={clipboardPaths}
    onRowsScroll={handleRowsScrollCombined}
    onWheel={handleWheelCombined}
    onRowsKeydown={handleRowsKeydownCombined}
    onRowsMousedown={handleRowsMouseDown}
    onRowsClick={handleRowsClickSafe}
    onRowsContextMenu={handleBlankContextMenu}
    onChangeSort={changeSort}
    onStartResize={startResize}
    ariaSort={ariaSort}
    onRowClick={handleRowClickWithOpen}
    onOpen={handleOpenEntry}
    onContextMenu={handleRowContextMenu}
    onToggleStar={toggleStar}
    onRowDragStart={handleRowDragStart}
    onRowDragEnd={handleRowDragEnd}
    onRowDragEnter={handleRowDragEnter}
    onRowDragOver={handleRowDragOver}
    onRowDrop={handleRowDrop}
    onRowDragLeave={handleRowDragLeave}
    dragTargetPath={$dragState.target}
    dragAllowed={dragPaths.length > 0}
    dragging={$dragState.dragging}
    onBreadcrumbDragOver={handleBreadcrumbDragOver}
    onBreadcrumbDragLeave={handleBreadcrumbDragLeave}
    onBreadcrumbDrop={handleBreadcrumbDrop}
  {selectionText}
  selectionActive={$selectionActive}
  selectionRect={$selectionRect}
  contextMenu={$contextMenu}
  blankMenu={$blankMenu}
  onContextSelect={handleContextSelect}
  onBlankContextSelect={handleBlankContextAction}
  onCloseContextMenu={closeContextMenu}
  onCloseBlankContextMenu={closeBlankContextMenu}
  deleteConfirmOpen={$deleteState.open}
  deleteTargets={$deleteState.targets}
  onConfirmDelete={deleteModal.confirm}
  onCancelDelete={deleteModal.close}
  renameModalOpen={$renameState.open}
  renameTarget={$renameState.target}
  renameError={$renameState.error}
  bind:renameValue
  onConfirmRename={confirmRename}
  onCancelRename={closeRenameModal}
  advancedRenameOpen={$advancedRenameState.open}
  advancedRenameEntries={$advancedRenameState.entries}
  advancedRenameRegex={$advancedRenameState.regex}
  advancedRenameReplacement={$advancedRenameState.replacement}
  advancedRenamePrefix={$advancedRenameState.prefix}
  advancedRenameSuffix={$advancedRenameState.suffix}
  advancedRenameCaseSensitive={$advancedRenameState.caseSensitive}
  advancedRenameSequenceMode={$advancedRenameState.sequenceMode}
  advancedRenameSequencePlacement={$advancedRenameState.sequencePlacement}
  advancedRenameSequenceStart={$advancedRenameState.sequenceStart}
  advancedRenameSequenceStep={$advancedRenameState.sequenceStep}
  advancedRenameSequencePad={$advancedRenameState.sequencePad}
  advancedRenameError={$advancedRenameState.error}
  onAdvancedRenameChange={(payload) => advancedRenameState.update((s) => ({ ...s, ...payload, error: '' }))}
  onConfirmAdvancedRename={() => advancedRenameModal.confirm()}
  onCancelAdvancedRename={() => advancedRenameModal.close()}
  compressOpen={$compressState.open}
  bind:compressName
  bind:compressLevel
  compressError={$compressState.error}
  onConfirmCompress={confirmCompress}
  onCancelCompress={closeCompress}
  checkDuplicatesOpen={$checkDuplicatesState.open}
  checkDuplicatesTarget={$checkDuplicatesState.target}
  checkDuplicatesSearchRoot={$checkDuplicatesState.searchRoot}
  checkDuplicatesDuplicates={$checkDuplicatesState.duplicates}
  checkDuplicatesScanning={$checkDuplicatesState.scanning}
  checkDuplicatesProgressPercent={$checkDuplicatesState.progressPercent}
  checkDuplicatesProgressLabel={$checkDuplicatesState.progressLabel}
  checkDuplicatesError={$checkDuplicatesState.error}
  onChangeCheckDuplicatesSearchRoot={checkDuplicatesModal.setSearchRoot}
  onCopyCheckDuplicates={copyCheckDuplicatesList}
  onSearchCheckDuplicates={searchCheckDuplicates}
  onCloseCheckDuplicates={closeCheckDuplicatesModal}
  newFolderOpen={$newFolderState.open}
  bind:newFolderName
  newFolderError={$newFolderState.error}
  onConfirmNewFolder={confirmNewFolder}
  onCancelNewFolder={closeNewFolderModal}
  newFileOpen={$newFileState.open}
  bind:newFileName
  newFileError={$newFileState.error}
  onConfirmNewFile={confirmNewFile}
  onCancelNewFile={closeNewFileModal}
  openWithOpen={$openWithState.open}
  openWithApps={$openWithState.apps}
  openWithLoading={$openWithState.loading}
  openWithError={$openWithState.error}
  openWithBusy={$openWithState.submitting}
  onConfirmOpenWith={(choice) => openWithModal.confirm(choice)}
  onCloseOpenWith={openWithModal.close}
  propertiesOpen={$propertiesState.open}
  propertiesEntry={$propertiesState.entry}
  propertiesCount={$propertiesState.count}
  propertiesSize={$propertiesState.size}
  propertiesItemCount={$propertiesState.itemCount}
  propertiesHidden={$propertiesState.hidden}
  propertiesExtraMetadataLoading={$propertiesState.extraMetadataLoading}
  propertiesExtraMetadataError={$propertiesState.extraMetadataError}
  propertiesExtraMetadata={$propertiesState.extraMetadata}
  propertiesPermissionsLoading={$propertiesState.permissionsLoading}
  propertiesPermissions={$propertiesState.permissions}
  onTogglePermissionsAccess={(scope, key, next) => propertiesModal.toggleAccess(scope, key, next)}
  onToggleHidden={(next) => propertiesModal.toggleHidden(next)}
  onLoadPropertiesExtraMetadata={() => propertiesModal.loadExtraIfNeeded()}
  onCloseProperties={propertiesModal.close}
  bookmarkModalOpen={bookmarkModalOpen}
  {bookmarkCandidate}
  onConfirmBookmark={confirmBookmark}
  onCancelBookmark={closeBookmarkModal}
  toastMessage={$toast}
/>
<ConflictModal
  open={conflictModalOpen}
  conflicts={conflictList}
  onCancel={() => {
    conflictModalOpen = false
    conflictList = []
    conflictDest = null
  }}
  onRenameAll={() => resolveConflicts('rename')}
  onOverwrite={() => resolveConflicts('overwrite')}
/>
{#if settingsOpen}
  <SettingsModal
    open
    defaultViewValue={defaultViewPref}
    showHiddenValue={$showHidden}
    hiddenFilesLastValue={$hiddenFilesLast}
    foldersFirstValue={$foldersFirst}
    confirmDeleteValue={$confirmDelete}
    densityValue={$density}
    archiveNameValue={$archiveName}
    archiveLevelValue={$archiveLevel}
    openDestAfterExtractValue={$openDestAfterExtract}
    videoThumbsValue={$videoThumbs}
    hardwareAccelerationValue={$hardwareAcceleration}
    ffmpegPathValue={$ffmpegPath}
    thumbCacheMbValue={$thumbCacheMb}
    mountsPollMsValue={$mountsPollMs}
    doubleClickMsValue={$doubleClickMs}
    startDirValue={$startDirPref ?? '~'}
    sortFieldValue={$sortFieldPref}
    sortDirectionValue={$sortDirectionPref}
    shortcuts={shortcutBindings}
    onChangeDefaultView={(val) => {
      viewMode = val
      defaultViewPref = val
      void storeDefaultView(val)
    }}
    onToggleShowHidden={toggleShowHidden}
    onToggleHiddenFilesLast={toggleHiddenFilesLast}
    onToggleFoldersFirst={toggleFoldersFirst}
    onToggleConfirmDelete={toggleConfirmDelete}
    onChangeStartDir={setStartDirPref}
    onChangeDensity={setDensityPref}
    onChangeArchiveName={setArchiveNamePref}
    onChangeArchiveLevel={setArchiveLevelPref}
    onToggleOpenDestAfterExtract={toggleOpenDestAfterExtract}
    onToggleVideoThumbs={toggleVideoThumbs}
    onToggleHardwareAcceleration={setHardwareAccelerationPref}
    onChangeFfmpegPath={setFfmpegPathPref}
    onChangeThumbCacheMb={setThumbCachePref}
    onChangeMountsPollMs={setMountsPollPref}
    onChangeDoubleClickMs={setDoubleClickMsPref}
    onClearThumbCache={clearThumbnailCacheFromSettings}
    onClearStars={clearStarsFromSettings}
    onClearBookmarks={clearBookmarksFromSettings}
    onClearRecents={clearRecentsFromSettings}
    onChangeSortField={setSortFieldPref}
    onChangeSortDirection={setSortDirectionPref}
    onChangeShortcut={updateShortcutBinding}
    onClose={() => (settingsOpen = false)}
  />
{/if}
