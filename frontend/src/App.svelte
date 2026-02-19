<script lang="ts">
  // --- Imports -------------------------------------------------------------
  import { onMount, onDestroy, tick } from 'svelte'
  import { getErrorMessage } from '@/shared/lib/error'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { get, writable } from 'svelte/store'
  import { formatItems, formatSelectionLine, formatSize, normalizePath, parentPath } from './features/explorer/utils'
  import { createListState } from './features/explorer/stores/listState'
  import ExplorerShell from './features/explorer/components/ExplorerShell.svelte'
  import { useExplorerData } from './features/explorer/hooks/useExplorerData'
  import { createColumnResize } from './features/explorer/hooks/columnWidths'
  import { createGlobalShortcuts } from './features/explorer/hooks/shortcuts'
  import { createBookmarkModal } from './features/explorer/hooks/bookmarkModal'
  import { useExplorerDragDrop } from './features/explorer/hooks/useExplorerDragDrop'
  import { useModalsController } from './features/explorer/hooks/useModalsController'
  import { useGridVirtualizer } from './features/explorer/hooks/useGridVirtualizer'
  import { addBookmark, removeBookmark } from './features/explorer/services/bookmarks.service'
  import { ejectDrive } from './features/explorer/services/drives.service'
  import { openConsole } from './features/explorer/services/console.service'
  import {
    copyPathsToSystemClipboard,
    setClipboardCmd,
    clearSystemClipboard,
    pasteClipboardCmd,
    pasteClipboardPreview,
    getSystemClipboardPaths,
  } from './features/explorer/services/clipboard.service'
  import {
    entryKind,
    dirSizes,
    canExtractPaths as canExtractPathsCmd,
    extractArchive,
    extractArchives,
  } from './features/explorer/services/files.service'
  import { fetchContextMenuActions } from './features/explorer/services/contextMenu.service'
  import { undoAction, redoAction } from './features/explorer/services/history.service'
  import { cancelTask } from './features/explorer/services/activity.service'
  import { deleteEntries, moveToTrashMany, purgeTrashItems } from './features/explorer/services/trash.service'
  import type { Entry, Partition, SortField, Density } from './features/explorer/model/types'
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
  import { isScrollbarClick } from './features/explorer/helpers/scrollbar'
  import { moveCaret } from './features/explorer/helpers/navigationController'
  import { createSelectionMemory } from './features/explorer/model/selectionMemory'
  import { loadDefaultView, storeDefaultView } from './features/explorer/services/settings.service'
  import { createGridKeyboardHandler } from './features/explorer/hooks/useGridHandlers'
  import { useContextMenuBlocker } from './features/explorer/hooks/useContextMenuBlocker'
  import { createActivity } from './features/explorer/hooks/useActivity'
  import { createAppLifecycle } from './features/explorer/hooks/useAppLifecycle'
  import { createTopbarActions } from './features/explorer/hooks/useTopbarActions'
  import { createTextContextMenu } from './features/explorer/hooks/useTextContextMenu'
  import { createViewObservers } from './features/explorer/hooks/useViewObservers'
  import {
    buildNetworkEntryContextActions,
    connectNetworkUri,
    copyTextToSystemClipboard,
    isMountUri,
    networkBlankContextActions,
  } from './features/network'
  import { loadShortcuts, setShortcutBinding } from './features/shortcuts/service'
  import {
    DEFAULT_SHORTCUTS,
    matchesAnyShortcut,
    matchesShortcut,
    shortcutFor,
    type ShortcutBinding,
    type ShortcutCommandId,
  } from './features/shortcuts/keymap'
  import DragGhost from './shared/ui/DragGhost.svelte'
  import TextContextMenu from './features/explorer/components/TextContextMenu.svelte'
  import {
    checkDuplicatesStream,
    type DuplicateScanProgress,
  } from './features/explorer/services/duplicates.service'
  import {
    clearBookmarks,
    clearRecents,
    clearStars,
    clearThumbnailCache,
  } from './features/explorer/services/data.service'
  import { createNewFileTypeHint } from './features/explorer/hooks/useNewFileTypeHint'
  import ConflictModal from './shared/ui/ConflictModal.svelte'
  import SettingsModal from './features/settings/SettingsModal.svelte'
  import AboutBrowseyModal from './features/explorer/components/AboutBrowseyModal.svelte'
  import { anyModalOpen as anyModalOpenStore } from './shared/ui/modalOpenState'
  import './features/explorer/ExplorerLayout.css'

  // --- Types --------------------------------------------------------------
  type ViewMode = 'list' | 'grid'
  type SearchSession = {
    submittedQuery: string
    clearedDraftQuery: string | null
  }

  // --- Core UI state -------------------------------------------------------
  let sidebarCollapsed = false

  // Path / mode
  let pathInput = ''
  let mode: 'address' | 'filter' = 'address'
  let viewMode: ViewMode = 'list'
  let defaultViewPref: ViewMode = 'list'
  let inputFocused = false
  let filterActive = false
  let isSearchSessionEnabled = false
  let searchSession: SearchSession = {
    submittedQuery: '',
    clearedDraftQuery: null,
  }

  // DOM refs & observers
  let rowsElRef: HTMLDivElement | null = null
  let gridElRef: HTMLDivElement | null = null
  let headerElRef: HTMLDivElement | null = null
  let pathInputEl: HTMLInputElement | null = null
  const SCROLL_HOVER_SUPPRESS_MS = 150
  let scrollHoverTimer: ReturnType<typeof setTimeout> | null = null

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
  let aboutOpen = false
  let settingsInitialFilter = ''
  let thumbnailRefreshToken = 0
  let shortcutBindings: ShortcutBinding[] = DEFAULT_SHORTCUTS
  let pendingNav: { path: string; opts?: { recordHistory?: boolean; silent?: boolean }; gen: number } | null = null
  let navGeneration = 0

  // Drag & clipboard
  const { store: bookmarkStore } = bookmarkModal
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
  const {
    open: textMenuOpen,
    x: textMenuX,
    y: textMenuY,
    target: textMenuTarget,
    readonly: textMenuReadonly,
    close: closeTextContextMenu,
    handleDocumentContextMenu,
  } = createTextContextMenu()
  const viewObservers = createViewObservers()
  const {
    hint: newFileTypeHint,
    scheduleLookup: scheduleNewFileTypeHintLookup,
    reset: resetNewFileTypeHint,
  } = createNewFileTypeHint()

  const fsLabel = (fs?: string) => {
    const kind = (fs ?? '').toLowerCase()
    if (kind === 'onedrive') return 'OneDrive'
    if (kind === 'mtp') return 'MTP device'
    if (kind === 'sftp' || kind === 'ssh') return 'SFTP server'
    if (kind === 'smb' || kind === 'cifs') return 'SMB server'
    if (kind === 'nfs') return 'NFS server'
    if (kind === 'ftp' || kind === 'ftps') return 'FTP server'
    if (kind === 'dav' || kind === 'davs' || kind === 'webdav' || kind === 'webdavs') return 'WebDAV server'
    if (kind === 'afp') return 'AFP server'
    return 'network resource'
  }

  // --- Data + preferences --------------------------------------------------
  const explorer = useExplorerData({
    onCurrentChange: (path) => {
      const typingFilterOrSearch = mode === 'filter' || isSearchSessionEnabled
      if (typingFilterOrSearch) {
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
    searchRunning,
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
    columnFilters,
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
    visibleEntries: filterSourceEntries,
    load: loadRaw,
    loadRecent: loadRecentRaw,
    loadStarred: loadStarredRaw,
    loadNetwork: loadNetworkRaw,
    loadTrash: loadTrashRaw,
    cancelSearch,
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
    toggleColumnFilter,
    resetColumnFilter,
    columnFacets,
    columnFacetsLoading,
    ensureColumnFacets,
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

  const listContentScrollTop = () => {
    const rawTop = rowsElRef?.scrollTop ?? 0
    const headerOffset = headerElRef?.offsetHeight ?? 0
    return Math.max(0, rawTop - headerOffset)
  }

  let toggleViewModeInFlight = false
  let toggleViewModeRaf: number | null = null

  const cancelToggleViewModeRaf = () => {
    if (toggleViewModeRaf !== null) {
      cancelAnimationFrame(toggleViewModeRaf)
      toggleViewModeRaf = null
    }
  }

  const toggleViewMode = async () => {
    if (toggleViewModeInFlight) return
    toggleViewModeInFlight = true
    cancelToggleViewModeRaf()
    viewAnchor.capture({
      viewMode,
      rowsEl: rowsElRef,
      headerEl: headerElRef,
      gridEl: gridElRef,
      gridCols: getGridCols(),
    })
    const nextMode = viewMode === 'list' ? 'grid' : 'list'
    const switchingToList = nextMode === 'list'

    try {
      if (switchingToList) {
        viewObservers.disconnectGrid()
      }

      viewMode = nextMode
      selectionBox.active.set(false)
      selectionBox.rect.set({ x: 0, y: 0, width: 0, height: 0 })

      if (switchingToList) {
        gridTotalHeight.set(0)
        await tick()
        scrollTop.set(listContentScrollTop())
        updateViewportHeight()
        recompute(get(filteredEntries))
        toggleViewModeRaf = requestAnimationFrame(() => {
          toggleViewModeRaf = null
          if (viewMode !== 'list') return
          scrollTop.set(listContentScrollTop())
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
    } finally {
      toggleViewModeInFlight = false
    }
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

  const normalizeSearchQuery = (value: string) => value.trim()

  const patchSearchSession = (patch: Partial<SearchSession>) => {
    const next: SearchSession = {
      ...searchSession,
      ...patch,
    }
    if (
      next.submittedQuery === searchSession.submittedQuery &&
      next.clearedDraftQuery === searchSession.clearedDraftQuery
    ) {
      return
    }
    searchSession = next
  }

  const resetSearchSession = () => {
    patchSearchSession({
      submittedQuery: '',
      clearedDraftQuery: null,
    })
  }

  const markSearchResultsStale = (draftQuery: string) => {
    if (searchSession.clearedDraftQuery === draftQuery) return
    cancelSearch()
    filter.set(searchSession.submittedQuery)
    patchSearchSession({ clearedDraftQuery: draftQuery })
  }

  const exitSearchSession = async (
    options: {
      reloadOnDisable?: boolean
      skipToggle?: boolean
    } = {},
  ) => {
    const { reloadOnDisable = true, skipToggle = false } = options
    if (skipToggle) {
      cancelSearch()
      searchMode.set(false)
    } else if (isSearchSessionEnabled) {
      await toggleMode(false, { reloadOnDisable })
    }
    filter.set('')
    resetSearchSession()
  }

  const transitionToAddressMode = async (
    options: {
      path?: string
      blur?: boolean
      reloadOnDisable?: boolean
      skipToggle?: boolean
    } = {},
  ) => {
    const { path = $current, blur = false, reloadOnDisable = true, skipToggle = false } = options
    mode = 'address'
    await exitSearchSession({ reloadOnDisable, skipToggle })
    pathInput = path
    if (blur) {
      pathInputEl?.blur()
    }
  }

  const transitionToFilterMode = async (path = '') => {
    mode = 'filter'
    await exitSearchSession({ reloadOnDisable: false })
    pathInput = path
  }

  // --- Selection memory ----------------------------------------------------

  const selectionMemory = createSelectionMemory()

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

  const loadDir = async (
    path?: string,
    opts: { recordHistory?: boolean; silent?: boolean } = {},
    navOpts: { resetScroll?: boolean } = {},
  ) => {
    const shouldResetScroll = navOpts.resetScroll ?? !opts.silent
    if (shouldResetScroll) {
      resetScrollPosition()
    }
    navGeneration += 1
    pendingNav = null
    captureSelectionSnapshot()
    await loadRaw(path, opts)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const loadRecent = async (recordHistory = true, applySort = false, navOpts: { resetScroll?: boolean } = {}) => {
    if (navOpts.resetScroll ?? true) {
      resetScrollPosition()
    }
    captureSelectionSnapshot()
    await loadRecentRaw(recordHistory, applySort)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const loadStarred = async (recordHistory = true, navOpts: { resetScroll?: boolean } = {}) => {
    if (navOpts.resetScroll ?? true) {
      resetScrollPosition()
    }
    captureSelectionSnapshot()
    await loadStarredRaw(recordHistory)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const loadNetwork = async (
    recordHistory = true,
    navOpts: { resetScroll?: boolean; forceRefresh?: boolean } = {},
  ) => {
    if (navOpts.resetScroll ?? true) {
      resetScrollPosition()
    }
    captureSelectionSnapshot()
    await loadNetworkRaw(recordHistory, { forceRefresh: navOpts.forceRefresh === true })
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const loadTrash = async (recordHistory = true, navOpts: { resetScroll?: boolean } = {}) => {
    if (navOpts.resetScroll ?? true) {
      resetScrollPosition()
    }
    captureSelectionSnapshot()
    await loadTrashRaw(recordHistory)
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const goBack = async () => {
    captureSelectionSnapshot()
    const before = get(current)
    await goBackRaw()
    if (get(current) !== before) {
      resetScrollPosition()
    }
    restoreSelectionForCurrent()
    await centerSelectionIfAny()
  }

  const goForward = async () => {
    captureSelectionSnapshot()
    const before = get(current)
    await goForwardRaw()
    if (get(current) !== before) {
      resetScrollPosition()
    }
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

    if (await isMountUri(trimmed)) {
      await openPartition(trimmed)
      return
    }

    try {
      const kind = await entryKind(trimmed)
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
    if (await isMountUri(path)) {
      try {
        const result = await connectNetworkUri(path)
        if (result.kind === 'unsupported') {
          showToast('Unsupported network protocol')
          return
        }
        if (result.kind === 'mountable') {
          await loadPartitions({ forceNetworkRefresh: true })
          if (result.mountedPath) {
            await loadDirIfIdle(result.mountedPath)
          } else {
            showToast('Mounted, but no mount path found')
          }
        }
      } catch (err) {
        showToast(`Connect failed: ${getErrorMessage(err)}`)
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
    if (viewMode !== 'grid') {
      viewObservers.disconnectGrid()
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
  $: currentView = viewFromPath($current)
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
      updateViewportHeight()
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

  $: {
    isSearchSessionEnabled = $searchMode
  }

  $: {
    filterActive = mode === 'filter' && pathInput.length > 0
    if (mode === 'filter') {
      filter.set(pathInput)
    }
  }

  $: {
    if (!isSearchSessionEnabled) {
      resetSearchSession()
    } else {
      const draftQuery = normalizeSearchQuery(pathInput)
      const stale = draftQuery !== searchSession.submittedQuery
      if (stale) {
        markSearchResultsStale(draftQuery)
      } else {
        patchSearchSession({
          clearedDraftQuery: null,
        })
      }
    }
  }

  const resetInputModeForNavigation = (returnToBreadcrumb = false, currentPath = get(current)) => {
    void transitionToAddressMode({
      path: currentPath,
      blur: returnToBreadcrumb,
      reloadOnDisable: false,
      skipToggle: true,
    })
  }

  $: {
    const curr = $current
    if (curr !== lastLocation) {
      const wasInSearchMode = isSearchSessionEnabled
      lastLocation = curr
      resetInputModeForNavigation(wasInSearchMode, curr)
    }
  }

  const { startResize } = createColumnResize(cols, persistWidths, getListMaxWidth)

  let dirSizeAbort = 0
  let activeDirSizeProgressEvent: string | null = null
  let selectionText = ''

  const computeDirStats = async (
    paths: string[],
    onProgress?: (bytes: number, items: number) => void,
  ): Promise<{ total: number; items: number }> => {
    if (paths.length === 0) return { total: 0, items: 0 }
    if (activeDirSizeProgressEvent) {
      void cancelTask(activeDirSizeProgressEvent).catch(() => {})
      activeDirSizeProgressEvent = null
    }
    const token = ++dirSizeAbort
    const progressEvent = `dir-size-progress-${token}-${Math.random().toString(16).slice(2)}`
    activeDirSizeProgressEvent = progressEvent
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
      const result = await dirSizes(paths, progressEvent)
      if (token !== dirSizeAbort) {
        void cancelTask(progressEvent).catch(() => {})
        return { total: 0, items: 0 }
      }
      return { total: result.total ?? 0, items: result.total_items ?? 0 }
    } catch (err) {
      console.error('Failed to compute dir sizes', err)
      return { total: 0, items: 0 }
    } finally {
      if (activeDirSizeProgressEvent === progressEvent) {
        activeDirSizeProgressEvent = null
      } else {
        void cancelTask(progressEvent).catch(() => {})
      }
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

    const hasFilter = $filter.trim().length > 0
    const filterLine = hasFilter ? `${$filteredEntries.length} results` : ''

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

  const submitSearch = () => {
    patchSearchSession({
      submittedQuery: normalizeSearchQuery(pathInput),
      clearedDraftQuery: null,
    })
    void runSearch(pathInput)
  }

  const enterAddressMode = async () => {
    await transitionToAddressMode({ path: $current })
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
      await transitionToFilterMode('')
      return
    }
    if (!value) {
      await transitionToAddressMode({ path: $current })
      return
    }
    pathInput = ''
    resetSearchSession()
    await toggleMode(true)
    mode = 'address'
  }

  const { handleTopbarAction, handleTopbarViewModeChange } = createTopbarActions({
    openSettings: (initialFilter) => {
      settingsInitialFilter = initialFilter
      settingsOpen = true
    },
    isSearchMode: () => isSearchSessionEnabled,
    setSearchMode: setSearchModeState,
    focusPathInput,
    toggleShowHidden: () => toggleShowHidden(),
    openAbout: () => {
      aboutOpen = true
    },
    refresh: () => reloadCurrent(),
    getViewMode: () => viewMode,
    toggleViewMode: () => toggleViewMode(),
  })

  const navigateToBreadcrumb = async (path: string) => {
    if (currentView !== 'dir') {
      return
    }
    await transitionToAddressMode({ path, reloadOnDisable: false })
    await goToPath(path)
  }

  const shortcuts = createGlobalShortcuts({
    isBookmarkModalOpen: () => get(bookmarkStore).open,
    searchMode: () => isSearchSessionEnabled,
    setSearchMode: async (value: boolean) => setSearchModeState(value),
    focusPath: () => focusPathInput(),
    isShortcut,
    onToggleHidden: () => Promise.resolve(toggleShowHidden()),
    onTypeChar: async (char) => {
      if (inputFocused && mode === 'address' && canUseSearch()) {
        return false
      }
      if (isSearchSessionEnabled && canUseSearch()) {
        pathInput = `${pathInput}${char}`
        focusPathInput()
        return true
      }
      if (mode !== 'filter') {
        await transitionToFilterMode('')
      }
      pathInput = `${pathInput}${char}`
      focusPathInput()
      return true
    },
    onRemoveChar: async () => {
      if (isSearchSessionEnabled) {
        if (pathInput.length === 0) {
          await enterAddressMode()
          focusPathInput()
          return true
        }
        pathInput = pathInput.slice(0, -1)
        focusPathInput()
        return true
      }
      if (mode === 'filter') {
        if (pathInput.length <= 1) {
          await transitionToAddressMode({ path: $current, blur: true })
          return true
        }
        pathInput = pathInput.slice(0, -1)
        focusPathInput()
        return true
      }
      if (mode === 'address') {
        return false
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
      const result = await clipboard.copyPaths(paths)
      if (!result.ok) {
        showToast(`Copy failed: ${result.error}`)
        return false
      }
      showToast('Copied', 1500)
      void copyPathsToSystemClipboard(paths).catch((err) => {
        showToast(
          `Copied (system clipboard unavailable: ${getErrorMessage(err)})`,
          2500
        )
      })
      return true
    },
    onCut: async () => {
      if (currentView === 'network') return false
      const paths = Array.from($selected)
      const result = await clipboard.cutPaths(paths)
      if (!result.ok) {
        showToast(`Cut failed: ${result.error}`)
        return false
      }
      showToast('Cut', 1500)
      void copyPathsToSystemClipboard(paths, 'cut').catch((err) => {
        showToast(
          `Cut (system clipboard unavailable: ${getErrorMessage(err)})`,
          2500
        )
      })
      return true
    },
    onPaste: async () => {
      if (currentView !== 'dir') return false
      return pasteIntoCurrent()
    },
    onRename: async () => {
      if (currentView === 'network') return false
      if ($selected.size !== 1) return false
      const path = Array.from($selected)[0]
      const entry = $entries.find((e) => e.path === path)
      if (!entry) return false
      renameValue = entry.name
      renameModal.open(entry)
      return true
    },
    onDelete: async (permanent) => {
      if (currentView === 'network') return false
      const selectedPaths = Array.from($selected)
      if (selectedPaths.length === 0) return false
      const selectedPathSet = new Set(selectedPaths)
      const entries = $filteredEntries.filter((e) => selectedPathSet.has(e.path))
      if (entries.length === 0) return false
      const hasNetwork = entries.some((e) => e.network)
      const inTrashView = currentView === 'trash'

      if (permanent || (hasNetwork && !inTrashView)) {
        deleteModal.open(entries, inTrashView ? 'trash' : 'default')
        return true
      }
      const label = inTrashView ? 'Deleting…' : 'Moving to trash…'
      const total = entries.length
      await activityApi.cleanup()
      activity.set({ label, percent: total > 0 ? 0 : null })
      try {
        if (inTrashView) {
          const ids = entries.map((e) => e.trash_id ?? e.path)
          await purgeTrashItems(ids)
          activity.set({ label, percent: 100 })
        } else {
          const paths = entries.map((e) => e.path)
          const progressEvent = `trash-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
          await activityApi.start(label, progressEvent)
          await moveToTrashMany(paths, progressEvent)
        }
        await reloadCurrent()
      } catch (err) {
        console.error(inTrashView ? 'Failed to delete from trash' : 'Failed to move to trash', err)
        showToast(
          `${inTrashView ? 'Delete failed' : 'Move to trash failed'}: ${getErrorMessage(err)}`,
          3000
        )
      } finally {
        if (inTrashView) {
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
      if (currentView === 'network') return false
      const sel = get(selected)
      if (sel.size === 0) return false
      const list = get(filteredEntries).filter((e) => sel.has(e.path))
      if (list.length === 0) return false
      const inTrashView = currentView === 'trash'
      if (get(explorer.confirmDelete)) {
        modalActions.confirmDelete(list, inTrashView ? 'trash' : 'default')
        return true
      }
      try {
        if (inTrashView) {
          const ids = list.map((e) => e.trash_id ?? e.path)
          await purgeTrashItems(ids)
        } else {
          const paths = list.map((e) => e.path)
          await deleteEntries(paths)
        }
        await reloadCurrent()
        showToast('Deleted')
        return true
      } catch (err) {
        showToast(`Delete failed: ${getErrorMessage(err)}`)
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
        showToast(`Open console failed: ${getErrorMessage(err)}`)
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
        showToast(`Undo failed: ${getErrorMessage(err)}`)
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
        showToast(`Redo failed: ${getErrorMessage(err)}`)
        return false
      }
    },
    onToggleSettings: async () => {
      if (settingsOpen) {
        settingsOpen = false
      } else {
        settingsInitialFilter = ''
        settingsOpen = true
      }
      return true
    },
  })
  const { handleGlobalKeydown } = shortcuts

  const handleDocumentKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Control' || event.key === 'Meta') {
      setCopyModifierActive(true)
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
      const hasAppShortcut = matchesAnyShortcut(event, shortcutBindings)
      if (!hasAppShortcut) {
        event.preventDefault()
        event.stopPropagation()
        return
      }
      // Claim app-owned Ctrl/Cmd shortcuts immediately in capture phase so
      // native/webview handlers do not also process the same accelerator.
      event.preventDefault()
      event.stopPropagation()
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
        void transitionToAddressMode({ path: $current, blur: true })
        return
      }
      if (isSearchSessionEnabled) {
        event.preventDefault()
        event.stopPropagation()
        void transitionToAddressMode({ path: $current, blur: true })
        return
      }
      if (inputFocused && mode === 'address') {
        event.preventDefault()
        event.stopPropagation()
        pathInput = $current
        blurPathInput()
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
      setCopyModifierActive(false)
    }
  }

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

  $: {
    if (rowsElRef && viewMode === 'list') {
      viewObservers.setupRows(rowsElRef, updateViewportHeight)
      updateViewportHeight()
    }
  }

  $: {
    if (gridElRef && viewMode === 'grid') {
      viewObservers.setupGrid(gridElRef, viewMode, recomputeGrid)
      recomputeGrid()
    }
  }

  // --- Lifecycle: global listeners ---------------------------------------
  onDestroy(() => {
    cancelToggleViewModeRaf()
    resetNewFileTypeHint()
    cancelSearch()
    activityApi.clearNow()
    void activityApi.cleanup()
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

      let actions = await fetchContextMenuActions<ContextAction[]>({
        count: selectionCount,
        kind: entry.kind,
        starred: Boolean(entry.starred),
        view: currentView,
        clipboardHasItems: clipboardPaths.size > 0,
        selectionPaths,
      })
      actions = actions.filter((action) => action.id !== 'new-folder')
      // Drop any leading dividers that can appear after filtering
      while (actions.length > 0 && actions[0].id.startsWith('divider')) {
        actions.shift()
      }
      if (currentView === 'network') {
        const networkActions = await buildNetworkEntryContextActions(entry.path, selectionCount)
        if (networkActions) {
          actions = networkActions
        }
      }
      const inSearch = isSearchSessionEnabled
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
    if (currentView !== 'dir') {
      showToast('Cannot paste here')
      return false
    }

    // Always attempt to sync from system clipboard first, then paste.
    try {
      const sys = await getSystemClipboardPaths()
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
      const msg = getErrorMessage(err)
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
      await loadRecent(false, false, { resetScroll: false })
      return
    }
    if (currentView === 'starred') {
      await loadStarred(false, { resetScroll: false })
      return
    }
    if (currentView === 'network') {
      await loadNetwork(false, { resetScroll: false, forceRefresh: true })
      return
    }
    if (currentView === 'trash') {
      await loadTrash(false, { resetScroll: false })
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
      const msg = getErrorMessage(err)
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
        await loadStarred(false, { resetScroll: false })
      }
      if (removed > 0) {
        showToast(`Cleared ${removed} star${removed === 1 ? '' : 's'}`, 1800)
      } else {
        showToast('No stars to clear', 1600)
      }
    } catch (err) {
      const msg = getErrorMessage(err)
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
      const msg = getErrorMessage(err)
      showToast(`Clear bookmarks failed: ${msg}`)
      throw err
    }
  }

  const clearRecentsFromSettings = async () => {
    try {
      const removed = await clearRecents()
      if (currentView === 'recent') {
        await loadRecent(false, false, { resetScroll: false })
      }
      if (removed > 0) {
        showToast(`Cleared ${removed} recent item${removed === 1 ? '' : 's'}`, 1800)
      } else {
        showToast('No recents to clear', 1600)
      }
    } catch (err) {
      const msg = getErrorMessage(err)
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

  $: if ($newFileState.open) {
    scheduleNewFileTypeHintLookup(newFileName)
  } else {
    resetNewFileTypeHint()
  }

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

  const canExtractPaths = async (paths: string[]): Promise<boolean> => {
    if (paths.length === 0) return false
    try {
      return await canExtractPathsCmd(paths)
    } catch {
      return false
    }
  }

  const extractEntries = async (entriesToExtract: Entry[]) => {
    if (extracting) return
    if (entriesToExtract.length === 0) return
    const allArchives = await canExtractPaths(entriesToExtract.map((entry) => entry.path))
    if (!allArchives) {
      showToast('Extraction available for archive files only')
      return
    }

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
        const result = await extractArchive(entry.path, progressEvent)
        if (get(openDestAfterExtract) && result?.destination) {
          try {
            const kind = await entryKind(result.destination)
            const target = kind === 'dir' ? result.destination : parentPath(result.destination)
            await loadRaw(target, { recordHistory: true })
          } catch {
            await reloadCurrent()
          }
        } else {
          await reloadCurrent()
        }
        const suffix = summarize(result?.skipped_symlinks ?? 0, result?.skipped_entries ?? 0)
        showToast(`Extracted to ${result.destination}${suffix}`)
      } else {
        const result = await extractArchives(entriesToExtract.map((e) => e.path), progressEvent)
        const successes = result.filter((r) => r.ok && r.result)
        const failures = result.filter((r) => !r.ok)
        // In batch extraction, keep the current location stable even if
        // "Open destination after extract" is enabled.
        await reloadCurrent()
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
      const msg = getErrorMessage(err)
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
    confirmDelete: (entries) =>
      modalActions.confirmDelete(entries, currentView === 'trash' ? 'trash' : 'default'),
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
      const msg = getErrorMessage(err)
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
      const msg = getErrorMessage(err)
      checkDuplicatesModal.failScan(msg)
      showToast(`Duplicate scan failed: ${msg}`)
      await cleanupDuplicateProgressListener()
    }
  }

  const handleOpenEntry = async (entry: Entry) => {
    pendingOpenCandidate = null
    if (entry.kind === 'dir') {
      if (currentView === 'network') {
        await transitionToAddressMode({ path: entry.path, reloadOnDisable: false })
        await openPartition(entry.path)
      } else {
        await transitionToAddressMode({ path: entry.path, reloadOnDisable: false })
        await loadDir(entry.path)
      }
      return
    }
    if (entry.kind === 'file' && await canExtractPaths([entry.path])) {
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
  const suppressHoverWhileScrolling = () => {
    const el = rowsElRef
    if (!el) return
    el.classList.add('is-scrolling')
    if (scrollHoverTimer !== null) {
      clearTimeout(scrollHoverTimer)
    }
    scrollHoverTimer = setTimeout(() => {
      scrollHoverTimer = null
      rowsElRef?.classList.remove('is-scrolling')
    }, SCROLL_HOVER_SUPPRESS_MS)
  }

  const handleRowsScrollCombined = () => {
    suppressHoverWhileScrolling()
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
    if (currentView === 'network') {
      selected.set(new Set())
      anchorIndex.set(null)
      caretIndex.set(null)
      openBlankContextMenu(networkBlankContextActions(), event.clientX, event.clientY)
      return
    }
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
    if (id === 'refresh-network') {
      closeBlankContextMenu()
      if (currentView === 'network') {
        await loadNetwork(false, { resetScroll: false, forceRefresh: true })
      }
      return
    }
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
        showToast(`Open console failed: ${getErrorMessage(err)}`)
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
    if (entry && id === 'copy-network-address') {
      const selectedPaths = $selected.has(entry.path) ? Array.from($selected) : [entry.path]
      const uriFlags = await Promise.all(selectedPaths.map((path) => isMountUri(path)))
      const payload = selectedPaths.filter((_, idx) => uriFlags[idx]).join('\n')
      const result = await copyTextToSystemClipboard(payload || entry.path)
      if (result.ok) {
        showToast(selectedPaths.length > 1 ? 'Server addresses copied' : 'Server address copied', 1500)
      } else {
        showToast(`Copy failed: ${result.error}`)
      }
      return
    }
    if (entry && id === 'open-network-target') {
      await openPartition(entry.path)
      return
    }
    if (entry && id === 'disconnect-network') {
      try {
        await ejectDrive(entry.path)
        await loadPartitions({ forceNetworkRefresh: true })
        if (currentView === 'network') {
          await loadNetwork(false, { resetScroll: false, forceRefresh: true })
        }
        showToast('Disconnected')
      } catch (err) {
        showToast(`Disconnect failed: ${getErrorMessage(err)}`)
      }
      return
    }
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
          ? (() => {
              const selectedPathSet = new Set(selectedPaths)
              return $filteredEntries.filter((e) => selectedPathSet.has(e.path))
            })()
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
        showToast(`Paste failed: ${getErrorMessage(err)}`)
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
      showToast(`Paste failed: ${getErrorMessage(err)}`)
      return false
    }
  }

  const {
    dragState,
    dragAction,
    startNativeDrop,
    stopNativeDrop,
    setCopyModifierActive,
    handleRowDragStart,
    handleRowDragEnd,
    handleRowDragEnter,
    handleRowDragOver,
    handleRowDrop,
    handleRowDragLeave,
    handleBreadcrumbDragOver,
    handleBreadcrumbDragLeave,
    handleBreadcrumbDrop,
  } = useExplorerDragDrop({
    currentView: () => currentView,
    currentPath: () => get(current),
    getSelectedSet: () => get(selected),
    loadDir: (path: string) => loadDir(path),
    focusEntryInCurrentList: (path: string) => {
      const list = get(entries)
      const match = list.find((e) => normalizePath(e.path) === normalizePath(path))
      if (!match) return
      const idx = list.findIndex((e) => e.path === match.path)
      selected.set(new Set([match.path]))
      anchorIndex.set(idx)
      caretIndex.set(idx)
    },
    handlePasteOrMove,
    showToast,
  })

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
      showToast(`Paste failed: ${getErrorMessage(err)}`)
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


  const initLifecycle = createAppLifecycle({
    handleResize,
    loadDefaultView,
    applyDefaultView: (prefView) => {
      defaultViewPref = prefView
      viewMode = prefView
    },
    loadShortcuts,
    applyShortcutBindings,
    startNativeDrop,
    stopNativeDrop,
    onMountStarted: (fs) => {
      activityApi.clearNow()
      activity.set({ label: `Connecting to ${fsLabel(fs)}…`, percent: null, cancel: null, cancelling: false })
    },
    onMountDone: (fs, ok, outcome) => {
      const label =
        outcome === 'already_connected'
          ? `Already connected to ${fsLabel(fs)}`
          : outcome === 'connected'
            ? `Connected to ${fsLabel(fs)}`
            : outcome === 'failed'
              ? `Failed to connect to ${fsLabel(fs)}`
              : ok
                ? `Connected to ${fsLabel(fs)}`
                : `Failed to connect to ${fsLabel(fs)}`
      activity.set({ label, percent: null, cancel: null, cancelling: false })
      activityApi.hideSoon()
      if (outcome === 'already_connected') {
        showToast('Already connected', 1400)
      }
    },
    onErrorToast: showToast,
    onCleanup: () => {
      if (activeDirSizeProgressEvent) {
        void cancelTask(activeDirSizeProgressEvent).catch(() => {})
        activeDirSizeProgressEvent = null
      }
      if (scrollHoverTimer !== null) {
        clearTimeout(scrollHoverTimer)
        scrollHoverTimer = null
      }
      rowsElRef?.classList.remove('is-scrolling')
      viewObservers.cleanup()
      dirSizeAbort++
    },
  })

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
  action={$dragAction}
/>
<TextContextMenu
  open={$textMenuOpen}
  x={$textMenuX}
  y={$textMenuY}
  target={$textMenuTarget}
  readonly={$textMenuReadonly}
  shortcuts={shortcutBindings}
  onClose={closeTextContextMenu}
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
      await loadPartitions({ forceNetworkRefresh: true })
    } catch (err) {
      showToast(`Eject failed: ${getErrorMessage(err)}`)
    }
  }}
  searchMode={isSearchSessionEnabled}
  loading={$loading}
  activity={$activity}
  onFocus={handleInputFocus}
  onBlur={handleInputBlur}
  onSubmitPath={submitPath}
  onSearch={submitSearch}
  onExitSearch={() => void transitionToAddressMode({ path: $current, blur: true })}
  onNavigateSegment={(path) => void navigateToBreadcrumb(path)}
  onTopbarAction={handleTopbarAction}
  onTopbarViewModeChange={handleTopbarViewModeChange}
  noticeMessage={$error}
  searchRunning={$searchRunning}
  {filterActive}
  {mode}
  filterValue={$filter}
  showHidden={$showHidden}
  columnFilters={$columnFilters}
  columnFacets={$columnFacets}
  columnFacetsLoading={$columnFacetsLoading}
  onEnsureColumnFacets={ensureColumnFacets}
  videoThumbs={$videoThumbs}
  thumbnailsEnabled={currentView !== 'trash'}
  {thumbnailRefreshToken}
  cols={$cols}
  gridTemplate={$gridTemplate}
  filteredEntries={$filteredEntries}
  filterSourceEntries={$filterSourceEntries}
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
    onToggleFilter={(field, id, checked) => toggleColumnFilter(field, id, checked)}
    onResetFilter={(field) => resetColumnFilter(field)}
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
    dragAllowed={$dragState.paths.length > 0}
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
  advancedRenamePreview={$advancedRenameState.preview}
  advancedRenamePreviewError={$advancedRenameState.previewError}
  advancedRenamePreviewLoading={$advancedRenameState.previewLoading}
  onAdvancedRenameChange={(payload) => advancedRenameModal.change(payload)}
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
  newFileTypeHint={$newFileTypeHint}
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
  propertiesMutationsLocked={$propertiesState.mutationsLocked}
  propertiesCount={$propertiesState.count}
  propertiesSize={$propertiesState.size}
  propertiesItemCount={$propertiesState.itemCount}
  propertiesHidden={$propertiesState.hidden}
  propertiesExtraMetadataLoading={$propertiesState.extraMetadataLoading}
  propertiesExtraMetadataError={$propertiesState.extraMetadataError}
  propertiesExtraMetadata={$propertiesState.extraMetadata}
  propertiesPermissionsLoading={$propertiesState.permissionsLoading}
  propertiesOwnershipApplying={$propertiesState.ownershipApplying}
  propertiesOwnershipError={$propertiesState.ownershipError}
  propertiesOwnershipUsers={$propertiesState.ownershipUsers}
  propertiesOwnershipGroups={$propertiesState.ownershipGroups}
  propertiesOwnershipOptionsLoading={$propertiesState.ownershipOptionsLoading}
  propertiesOwnershipOptionsError={$propertiesState.ownershipOptionsError}
  propertiesPermissions={$propertiesState.permissions}
  onTogglePermissionsAccess={(scope, key, next) => propertiesModal.toggleAccess(scope, key, next)}
  onSetOwnership={(owner, group) => propertiesModal.setOwnership(owner, group)}
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
<AboutBrowseyModal open={aboutOpen} onClose={() => (aboutOpen = false)} />
{#if settingsOpen}
  <SettingsModal
    open
    initialFilter={settingsInitialFilter}
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
