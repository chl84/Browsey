<script lang="ts">
  // --- Imports -------------------------------------------------------------
  import { onMount, onDestroy, tick } from 'svelte'
  import { getErrorMessage } from '@/shared/lib/error'
  import { get } from 'svelte/store'
  import { formatItems, formatSelectionLine, formatSize, normalizePath, parentPath } from '@/features/explorer/utils'
  import { createListState } from '@/features/explorer/state/list.store'
  import { ExplorerShell, useGridVirtualizer, createViewObservers } from '@/features/explorer/ui-shell'
  import { useExplorerData } from '@/features/explorer/hooks/useExplorerData'
  import { createColumnResize } from '@/features/explorer/hooks/createColumnResize'
  import { createGlobalShortcuts } from '@/features/explorer/hooks/createGlobalShortcuts'
  import { createBookmarkModal } from '@/features/explorer/hooks/createBookmarkModal'
  import { useExplorerDragDrop, createClipboard, useExplorerFileOps } from '@/features/explorer/file-ops'
  import { useExplorerInputHandlers } from '@/features/explorer/hooks/useExplorerInputHandlers'
  import { useModalsController } from '@/features/explorer/hooks/useModalsController'
  import { addBookmark, removeBookmark } from '@/features/explorer/services/bookmarks.service'
  import { ejectDrive } from '@/features/explorer/services/drives.service'
  import { openConsole } from '@/features/explorer/services/console.service'
  import { copyPathsToSystemClipboard } from '@/features/explorer/services/clipboard.service'
  import { undoAction, redoAction } from '@/features/explorer/services/history.service'
  import { deleteEntries, moveToTrashMany, purgeTrashItems } from '@/features/explorer/services/trash.service'
  import type { Entry, Partition, SortField } from '@/features/explorer/model/types'
  import { toast, showToast } from '@/features/explorer/hooks/useToast'
  import {
    createContextActions,
    createContextMenus,
    createTextContextMenu,
    useContextMenuBlocker,
    useExplorerContextMenuOps,
    type CurrentView,
  } from '@/features/explorer/context'
  import { createSelectionBox } from '@/features/explorer/selection'
  import {
    createTopbarActions,
    createViewSwitchAnchor,
    useExplorerNavigation,
    useExplorerSearchSession,
  } from '@/features/explorer/navigation'
  import { loadDefaultView, storeDefaultView } from '@/features/explorer/services/settings.service'
  import { createActivity } from '@/features/explorer/hooks/createActivity'
  import {
    DEFAULT_SHORTCUTS,
    loadShortcuts,
    matchesAnyShortcut,
    matchesShortcut,
    setShortcutBinding,
    type ShortcutBinding,
    type ShortcutCommandId,
  } from '@/features/shortcuts'
  import {
    clearBookmarks,
    clearRecents,
    clearStars,
    clearThumbnailCache,
  } from '@/features/explorer/services/data.service'
  import { createNewFileTypeHint } from '@/features/explorer/hooks/createNewFileTypeHint'
  import { SettingsModal } from '@/features/settings'
  import { anyModalOpen as anyModalOpenStore } from '@/shared/ui/modalOpenState'
  import { createCheckDuplicatesModal } from '@/features/explorer/modals/checkDuplicatesModal'
  import { buildExplorerSelectionText, computeSelectionAnchorRepair } from './explorerPageDerived'
  import {
    createExplorerContextActionsDeps,
    createExplorerContextMenuOpsDeps,
    createExplorerDragDropDeps,
    createExplorerFileOpsDeps,
    createExplorerInputHandlersDeps,
    createExplorerModalsControllerDeps,
    createExplorerNavigationDeps,
  } from './explorerPageDeps'
  import { createExplorerSettingsModalProps } from './createExplorerSettingsModalProps'
  import { createExplorerShellProps } from './createExplorerShellProps'
  import ExplorerPageOverlays from './ExplorerPageOverlays.svelte'
  import { useBookmarkModalFlow } from './useBookmarkModalFlow'
  import { useExplorerPageLifecycle } from './useExplorerPageLifecycle'
  import { useExplorerPageUiState } from './useExplorerPageUiState'
  import { useExplorerViewportLayout } from './useExplorerViewportLayout'
  import '@/features/explorer/ExplorerLayout.css'

  // --- Types --------------------------------------------------------------
  type ViewMode = 'list' | 'grid'

  /*
   * Temporary ownership boundaries (Phase 2 split plan):
   * - Keep this page as the composition root (DOM refs, top-level render, high-level orchestration).
   * - Prefer extracting pure prop/dependency assembly and page-only glue into pages/* helpers/hooks.
   * - Keep feature behavior inside existing feature hooks/services (navigation, file-ops, input, modals).
   */

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

  // DOM refs & observers
  let rowsElRef: HTMLDivElement | null = null
  let gridElRef: HTMLDivElement | null = null
  let headerElRef: HTMLDivElement | null = null
  let pathInputEl: HTMLInputElement | null = null

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
  let compressName = 'Archive'
  let compressLevel = 6
  let newFolderName = 'New folder'
  let newFileName = ''
  let settingsOpen = false
  let aboutOpen = false
  let settingsInitialFilter = ''
  let thumbnailRefreshToken = 0
  let shortcutBindings: ShortcutBinding[] = DEFAULT_SHORTCUTS

  // Drag & clipboard
  const { store: bookmarkStore } = bookmarkModal
  let clipboardMode: 'copy' | 'cut' = 'copy'
  let clipboardPaths = new Set<string>()

  // View / navigation tracking
  let currentView: CurrentView = 'dir'
  let lastLocation = ''
  let hasShownCloudRefreshHint = false
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
  const pageUiState = useExplorerPageUiState({
    getSettingsOpen: () => settingsOpen,
    setSettingsOpen: (next) => {
      settingsOpen = next
    },
    setSettingsInitialFilter: (next) => {
      settingsInitialFilter = next
    },
    setAboutOpen: (next) => {
      aboutOpen = next
    },
  })
  const bookmarkModalFlow = useBookmarkModalFlow({
    bookmarkModal,
    getBookmarkInputEl: () => bookmarkInputEl,
    getBookmarks: () => bookmarks,
    setBookmarks: (next) => {
      bookmarks = next
    },
    addBookmarkToStore: (entry) => {
      bookmarksStore.update((list) => [...list, entry])
    },
    persistBookmark: (label, path) => {
      void addBookmark(label, path)
    },
    setModalUiState: ({ open, name, candidate }) => {
      bookmarkModalOpen = open
      bookmarkName = name
      bookmarkCandidate = candidate as Entry | null
    },
  })

  const fsLabel = (fs?: string) => {
    const kind = (fs ?? '').toLowerCase()
    if (kind === 'mtp') return 'MTP device'
    if (kind === 'sftp' || kind === 'ssh') return 'SFTP server'
    if (kind === 'smb' || kind === 'cifs') return 'SMB server'
    if (kind === 'nfs') return 'NFS server'
    if (kind === 'ftp' || kind === 'ftps') return 'FTP server'
    if (kind === 'dav' || kind === 'davs' || kind === 'webdav' || kind === 'webdavs') return 'WebDAV server'
    if (kind === 'afp') return 'AFP server'
    return 'network resource'
  }

  const isCloudPath = (path: string) => path.startsWith('rclone://')

  // --- Data + preferences --------------------------------------------------
  const explorer = useExplorerData({
    onCurrentChange: (path) => {
      const typingFilterOrSearch = mode === 'filter' || isSearchSessionEnabled
      if (typingFilterOrSearch) {
        return
      }
      pathInput = path
      if (isCloudPath(path)) {
        if (!hasShownCloudRefreshHint) {
          hasShownCloudRefreshHint = true
          showToast('Cloud folders use manual refresh (F5); live watch is not available yet', 2600)
        }
      }
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
    loadBookmarks: _loadBookmarks,
    loadPartitions,
    loadSavedWidths: _loadSavedWidths,
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
    // Ensure columns cannot become wider than the visible list (including the star column).
    const el = rowsElRef ?? headerElRef
    if (!el) return null
    const style = getComputedStyle(el)
    const paddingLeft = parseFloat(style.paddingLeft) || 0
    const paddingRight = parseFloat(style.paddingRight) || 0
    return Math.max(0, el.clientWidth - paddingLeft - paddingRight)
  }

  const focusCurrentView = async () => {
    await tick()
    rowsElRef?.focus()
  }

  const isShortcut = (event: KeyboardEvent, commandId: ShortcutCommandId) => {
    return matchesShortcut(event, shortcutBindings, commandId)
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
    return Math.max(0, rowsElRef?.scrollTop ?? 0)
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

  // --- Extraction seam: view state + metrics + navigation/search assembly --
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
    handleRowsKeydown,
    handleRowsClick,
    handleRowClick,
    resetScrollPosition,
    recompute,
  } = createListState()

  $: rowsElStore.set(viewMode === 'list' ? rowsElRef : null)
  $: headerElStore.set(viewMode === 'list' ? headerElRef : null)
  $: {
    gridElRef = viewMode === 'grid' ? rowsElRef : null
    if (viewMode !== 'grid') {
      viewObservers.disconnectGrid()
    }
  }

  let rowsKeydownHandler: ((event: KeyboardEvent) => void) | null = null
  let rowSelectionHandler: ((entry: Entry, absoluteIndex: number, event: MouseEvent) => void) | null = null
  $: rowsKeydownHandler = handleRowsKeydown($filteredEntries)
  $: rowSelectionHandler = handleRowClick($filteredEntries)

  $: bookmarks = $bookmarksStore
  $: partitions = $partitionsStore
  $: {
    const state = $clipboardStore
    clipboardMode = state.mode
    clipboardPaths = state.paths
  }
  $: {
    const state = $bookmarkStore
    bookmarkModalFlow.syncFromStoreState(state)
  }
  let gridCardWidth = 120
  let gridRowHeight = 126
  let gridGap = 6
  const GRID_OVERSCAN = 8

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
    gridCols: _gridCols,
    getGridCols,
    gridTotalHeight,
    handleGridScroll,
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

  const viewportLayout = useExplorerViewportLayout({
    getViewMode: () => viewMode,
    setSidebarCollapsed: (collapsed) => {
      sidebarCollapsed = collapsed
    },
    listResize: handleListResize,
    recomputeGrid,
    setDensityMetrics: ({ rowHeight: nextRowHeight, gridGap: nextGridGap, gridCardWidth: nextGridCardWidth, gridRowHeight: nextGridRowHeight }) => {
      setRowHeight(nextRowHeight)
      gridGap = nextGridGap
      gridCardWidth = nextGridCardWidth
      gridRowHeight = nextGridRowHeight
      gridConfig.gap = nextGridGap
      gridConfig.cardWidth = nextGridCardWidth
      gridConfig.rowHeight = nextGridRowHeight
    },
    recreateViewAnchor: ({ rowHeight: nextRowHeight, gridGap: nextGridGap, gridRowHeight: nextGridRowHeight }) => {
      viewAnchor = createViewSwitchAnchor({
        filteredEntries,
        rowHeight: nextRowHeight,
        gridRowHeight: nextGridRowHeight,
        gridGap: nextGridGap,
      })
    },
    getFilteredEntries: () => get(filteredEntries),
    getVisibleEntries: () => get(visibleEntries),
    getGridTotalHeight: () => get(gridTotalHeight),
    getTotalHeight: () => get(totalHeight),
    getRowsEl: () => rowsElRef,
    getGridEl: () => gridElRef,
    updateViewportHeight,
    recomputeList: (entriesList) => recompute(entriesList as Entry[]),
  })

  $: viewportLayout.applyDensityClass($density)
  $: {
    $density
    viewportLayout.applyDensityMetrics()
  }
  $: {
    bookmarkModalFlow.syncDraftNameToModal(bookmarkModalOpen, bookmarkName)
  }

  const navigation = useExplorerNavigation(createExplorerNavigationDeps({
    current,
    loading,
    filteredEntries,
    selected,
    anchorIndex,
    caretIndex,
    rowHeight,
    gridTotalHeight,
    getViewMode: () => viewMode,
    getRowsEl: () => rowsElRef,
    getHeaderEl: () => headerElRef,
    getGridEl: () => gridElRef,
    getGridCols,
    getGridRowHeight: () => gridRowHeight,
    getGridGap: () => gridGap,
    resetScrollPosition,
    loadRaw: (path, opts) => loadRaw(path, opts),
    loadRecentRaw: (recordHistory, applySort) => loadRecentRaw(recordHistory, applySort),
    loadStarredRaw: (recordHistory) => loadStarredRaw(recordHistory),
    loadNetworkRaw: (recordHistory, opts) => loadNetworkRaw(recordHistory, opts),
    loadTrashRaw: (recordHistory) => loadTrashRaw(recordHistory),
    goBackRaw: () => goBackRaw(),
    goForwardRaw: () => goForwardRaw(),
    open: (entry) => open(entry),
    loadPartitions: (opts) => loadPartitions(opts),
    showToast,
    setPathInput: (value) => {
      pathInput = value
    },
  }))
  const {
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
  } = navigation
  $: currentView = viewFromPath($current)
  $: navigation.flushPendingNavigation()
  $: navigation.dropPendingNavIfCurrent()

  const searchSession = useExplorerSearchSession({
    getCurrentPath: () => get(current),
    getPathInput: () => pathInput,
    setPathInput: (value) => {
      pathInput = value
    },
    blurPathInput: () => {
      pathInputEl?.blur()
    },
    setMode: (next) => {
      mode = next
    },
    getMode: () => mode,
    isSearchSessionEnabled: () => isSearchSessionEnabled,
    canUseSearch: () => currentView === 'dir',
    cancelSearch,
    setSearchMode: (value) => searchMode.set(value),
    setFilterValue: (value) => filter.set(value),
    toggleMode: (enabled, opts) => toggleMode(enabled, opts),
    goToPath: (path) => goToPath(path),
    runSearch: (query) => runSearch(query),
  })
  const {
    transitionToAddressMode,
    transitionToFilterMode,
    setSearchModeState,
    submitPath,
    submitSearch,
    enterAddressMode,
    syncSearchSessionWithInput,
    resetInputModeForNavigation,
  } = searchSession

  $: {
    isSearchSessionEnabled = $searchMode
  }

  $: {
    filterActive = mode === 'filter' && pathInput.length > 0
    if (mode === 'filter') {
      filter.set(pathInput)
    }
  }

  $: syncSearchSessionWithInput()

  $: {
    const curr = $current
    if (curr !== lastLocation) {
      const wasInSearchMode = isSearchSessionEnabled
      lastLocation = curr
      resetInputModeForNavigation(wasInSearchMode, curr)
    }
  }

  const { startResize } = createColumnResize(cols, persistWidths, getListMaxWidth)

  let selectionText = ''

  $: {
    selectionText = buildExplorerSelectionText({
      entries: $entries,
      selectedPaths: $selected,
      filterValue: $filter,
      filteredCount: $filteredEntries.length,
      formatSelectionLine,
    })
  }

  // Re-anchor keyboard navigation after returning to a view so arrows start from the selected item.
  $: {
    const repair = computeSelectionAnchorRepair({
      list: $filteredEntries,
      selectedPaths: $selected,
      anchorIndex: $anchorIndex,
      caretIndex: $caretIndex,
    })
    if (repair) {
      if (repair.nextAnchorIndex !== null) anchorIndex.set(repair.nextAnchorIndex)
      if (repair.nextCaretIndex !== null) caretIndex.set(repair.nextCaretIndex)
    }
  }

  const ariaSort = (field: SortField) =>
    $sortField === field ? ($sortDirection === 'asc' ? 'ascending' : 'descending') : 'none'

  const isHidden = (entry: Entry) => entry.hidden === true || entry.name.startsWith('.')

  const displayName = (entry: Entry) => {
    if (entry.kind === 'file' && entry.ext) {
      return entry.name.replace(new RegExp(`\\.${entry.ext}$`), '')
    }
    return entry.name
  }

  // --- UI sizing & viewport updates ---------------------------------------
  const handleResize = () => viewportLayout.handleResize()

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

  const { handleTopbarAction, handleTopbarViewModeChange } = createTopbarActions({
    openSettings: (initialFilter) => pageUiState.openSettings(initialFilter),
    isSearchMode: () => isSearchSessionEnabled,
    setSearchMode: setSearchModeState,
    focusPathInput,
    toggleShowHidden: () => toggleShowHidden(),
    openAbout: pageUiState.openAbout,
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
      if (paths.some(isCloudPath)) {
        return true
      }
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
      if (paths.some(isCloudPath)) {
        return true
      }
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
      const hasCloud = entries.some((e) => isCloudPath(e.path))
      const inTrashView = currentView === 'trash'

      if (permanent || (hasNetwork && !inTrashView) || (hasCloud && !inTrashView)) {
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
      const label = 'Deleting…'
      const progressEvent = `delete-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
      try {
        await activityApi.start(label, progressEvent)
        const cloudDelete = !inTrashView && list.some((e) => isCloudPath(e.path))
        if (inTrashView) {
          const ids = list.map((e) => e.trash_id ?? e.path)
          await purgeTrashItems(ids)
        } else {
          const paths = list.map((e) => e.path)
          await deleteEntries(paths, progressEvent)
        }
        if (cloudDelete) {
          const refreshTarget = get(current)
          void (async () => {
            if (get(current) !== refreshTarget) {
              return
            }
            try {
              await reloadCurrent()
            } catch {
              if (get(current) !== refreshTarget) {
                return
              }
              showToast('Delete completed, but refresh took too long. Press F5 to refresh.', 3500)
            }
          })()
        } else {
          await reloadCurrent()
        }
        activityApi.hideSoon()
        showToast('Deleted')
        return true
      } catch (err) {
        activityApi.clearNow()
        await activityApi.cleanup()
        showToast(`Delete failed: ${getErrorMessage(err)}`)
        return false
      } finally {
        const hadTimer = activityApi.hasHideTimer()
        await activityApi.cleanup(true)
        if (!hadTimer) {
          activityApi.clearNow()
        }
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
      if (isCloudPath(get(current))) {
        showToast('Open in console is not available for cloud folders')
        return true
      }
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
      pageUiState.toggleSettings()
      return true
    },
  })
  const { handleGlobalKeydown } = shortcuts

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

  const { openBookmarkModal, closeBookmarkModal, confirmBookmark } = bookmarkModalFlow

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

  // --- Extraction seam: file ops / modals / context / input wiring --------
  const checkDuplicatesModal = createCheckDuplicatesModal({ parentPath })
  const checkDuplicatesState = checkDuplicatesModal.state
  const fileOps = useExplorerFileOps(createExplorerFileOpsDeps({
    currentView: () => currentView,
    getCurrentPath: () => get(current),
    clipboardMode: () => clipboardMode,
    setClipboardPaths: (paths) => {
      clipboardPaths = paths
    },
    shouldOpenDestAfterExtract: () => get(openDestAfterExtract),
    loadPath: (path, opts) => loadRaw(path, opts),
    reloadCurrent,
    getDuplicateScanInput: () => {
      const state = get(checkDuplicatesState)
      return {
        target: state.target,
        searchRoot: state.searchRoot,
        scanning: state.scanning,
      }
    },
    duplicateModalStart: () => checkDuplicatesModal.startScan(),
    duplicateModalSetProgress: (percent, label) => checkDuplicatesModal.setProgress(percent, label),
    duplicateModalFinish: (paths) => checkDuplicatesModal.finishScan(paths),
    duplicateModalFail: (error) => checkDuplicatesModal.failScan(error),
    duplicateModalStop: () => checkDuplicatesModal.stopScan(),
    duplicateModalClose: () => checkDuplicatesModal.close(),
    showToast,
    activityApi,
  }))
  const {
    conflictModalOpen,
    conflictList,
    computeDirStats,
    abortDirStats,
    pasteIntoCurrent,
    handlePasteOrMove,
    resolveConflicts,
    canExtractPaths,
    extractEntries,
    stopDuplicateScan,
    closeCheckDuplicatesModal,
    searchCheckDuplicates,
    cancelConflicts,
  } = fileOps
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
    actions: modalActions,
  } = useModalsController(createExplorerModalsControllerDeps({
    activityApi,
    reloadCurrent,
    showToast,
    getCurrentPath: () => get(current),
    loadPath: (path, opts) => loadRaw(path, opts),
    parentPath,
    checkDuplicatesModal,
    computeDirStats,
  }))

  $: if ($newFileState.open) {
    scheduleNewFileTypeHintLookup(newFileName)
  } else {
    resetNewFileTypeHint()
  }

  let clearPendingOpenCandidate = () => {}

  const contextActions = createContextActions(createExplorerContextActionsDeps({
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
  }))
  const contextMenuOps = useExplorerContextMenuOps(createExplorerContextMenuOpsDeps({
    currentView: () => currentView,
    isSearchSessionEnabled: () => isSearchSessionEnabled,
    shortcutBindings: () => shortcutBindings,
    getCurrentPath: () => get(current),
    getContextMenuEntry: () => get(contextMenu).entry,
    getClipboardPathCount: () => clipboardPaths.size,
    getSelectedSet: () => get(selected),
    getFilteredEntries: () => get(filteredEntries),
    setSelection: (paths, anchor, caret) => {
      selected.set(paths)
      anchorIndex.set(anchor)
      caretIndex.set(caret)
    },
    openContextMenu,
    closeContextMenu,
    openBlankContextMenu,
    closeBlankContextMenu,
    loadNetwork,
    openPartition,
    loadPartitions,
    pasteIntoCurrent,
    openNewFolderModal: () => {
      newFolderName = newFolderModal.open()
    },
    openNewFileModal: () => {
      newFileName = newFileModal.open()
    },
    openAdvancedRename: (entries) => {
      advancedRenameModal.open(entries)
    },
    startRename: (entry) => {
      renameValue = entry.name
      modalActions.startRename(entry)
    },
    contextActions,
    showToast,
    onBeforeRowContextMenu: () => {
      clearPendingOpenCandidate()
    },
  }))
  const {
    handleRowContextMenu,
    handleBlankContextMenu,
    handleBlankContextAction,
    handleContextSelect,
  } = contextMenuOps

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

  const dragDropDeps = createExplorerDragDropDeps({
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
  } = useExplorerDragDrop(dragDropDeps)

  const inputHandlerDeps = createExplorerInputHandlersDeps({
    getViewMode: () => viewMode,
    getMode: () => mode,
    setPathInput: (value) => {
      pathInput = value
    },
    getPathInput: () => pathInput,
    isInputFocused: () => inputFocused,
    getCurrentPath: () => get(current),
    isSearchSessionEnabled: () => isSearchSessionEnabled,
    getRowsEl: () => rowsElRef,
    getHeaderEl: () => headerElRef,
    getFilteredEntries: () => get(filteredEntries),
    getSelected: () => get(selected),
    setSelected: (next) => {
      selected.set(next)
    },
    getAnchorIndex: () => get(anchorIndex),
    setAnchorIndex: (next) => {
      anchorIndex.set(next)
    },
    getCaretIndex: () => get(caretIndex),
    setCaretIndex: (next) => {
      caretIndex.set(next)
    },
    getRowHeight: () => get(rowHeight),
    getDoubleClickMs: () => get(doubleClickMs),
    setCopyModifierActive,
    isEditableTarget,
    hasAppShortcut: (event) => matchesAnyShortcut(event, shortcutBindings),
    handleGlobalKeydown,
    transitionToAddressMode,
    blurPathInput,
    ensureGridVisible,
    getRowsKeydownHandler: () => rowsKeydownHandler,
    getRowSelectionHandler: () => rowSelectionHandler,
    selectionBox,
    getGridCols,
    getGridCardWidth: () => gridCardWidth,
    getGridRowHeight: () => gridRowHeight,
    getGridGap: () => gridGap,
    handleRowsScroll,
    handleGridScroll,
    handleRowsClick,
    currentView: () => currentView,
    loadDir: (path) => loadDir(path),
    openPartition: (path) => openPartition(path),
    canExtractPaths,
    extractEntries,
    open,
    isBlockingModalOpen: () => get(anyModalOpenStore),
    isDeleteModalOpen: () => get(deleteState).open,
    closeDeleteModal: () => deleteModal.close(),
    isRenameModalOpen: () => get(renameState).open,
    closeRenameModal: () => renameModal.close(),
    isOpenWithModalOpen: () => get(openWithState).open,
    closeOpenWithModal: () => openWithModal.close(),
    isPropertiesModalOpen: () => get(propertiesState).open,
    closePropertiesModal: () => propertiesModal.close(),
    isCompressModalOpen: () => get(compressState).open,
    closeCompressModal: () => compressModal.close(),
    isCheckDuplicatesModalOpen: () => get(checkDuplicatesState).open,
    closeCheckDuplicatesModal: () => closeCheckDuplicatesModal(),
    isNewFolderModalOpen: () => get(newFolderState).open,
    closeNewFolderModal: () => newFolderModal.close(),
    isNewFileModalOpen: () => get(newFileState).open,
    closeNewFileModal: () => newFileModal.close(),
    isBookmarkModalOpen: () => bookmarkModalOpen,
    closeBookmarkModal,
    isContextMenuOpen: () => get(contextMenu).open,
    closeContextMenu,
    isBlankMenuOpen: () => get(blankMenu).open,
    closeBlankContextMenu,
    suppressHoverMs: 150,
  })
  const inputHandlers = useExplorerInputHandlers(inputHandlerDeps)
  const {
    handleDocumentKeydown,
    handleDocumentKeyup,
    handleOpenEntry,
    handleRowClickWithOpen,
    handleRowsMouseDown,
    handleRowsScrollCombined,
    handleRowsKeydownCombined,
    handleWheelCombined,
    handleRowsClickSafe,
    clearPendingOpenCandidate: clearPendingOpenCandidateHandler,
    cleanupScrollHover,
  } = inputHandlers
  clearPendingOpenCandidate = clearPendingOpenCandidateHandler

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

  const handleSidebarBookmarkSelect = (path: string) => {
    void loadDirIfIdle(path)
  }

  const handleSidebarRemoveBookmark = (path: string) => {
    void removeBookmark(path)
    bookmarksStore.update((list) => list.filter((b) => b.path !== path))
  }

  const handleSidebarPartitionSelect = (path: string) => {
    void openPartition(path)
  }

  const handleSidebarPartitionEject = async (path: string) => {
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
  }

  const handleSettingsDefaultViewChange = (val: 'list' | 'grid') => {
    viewMode = val
    defaultViewPref = val
    void storeDefaultView(val)
  }

  // --- Extraction seam: ExplorerShell prop assembly (Step 2) --------------
  let explorerShellSidebarProps: any
  let explorerShellTopbarProps: any
  let explorerShellListingProps: any
  let explorerShellMenuProps: any
  let explorerShellModalProps: any
  let explorerShellStatusProps: any
  let settingsModalProps: any

  $: ({
    sidebarProps: explorerShellSidebarProps,
    topbarProps: explorerShellTopbarProps,
    listingProps: explorerShellListingProps,
    menuProps: explorerShellMenuProps,
    modalProps: explorerShellModalProps,
    statusProps: explorerShellStatusProps,
  } = createExplorerShellProps({
    sidebarCollapsed,
    places,
    bookmarks,
    partitions,
    handlePlace,
    handleSidebarBookmarkSelect,
    handleSidebarRemoveBookmark,
    handleSidebarPartitionSelect,
    handleSidebarPartitionEject,
    mode,
    isSearchSessionEnabled,
    loading: $loading,
    viewMode,
    showHidden: $showHidden,
    activity: $activity,
    handleInputFocus,
    handleInputBlur,
    submitPath,
    submitSearch,
    transitionToAddressMode,
    currentPathValue: $current,
    navigateToBreadcrumb,
    handleTopbarAction,
    handleTopbarViewModeChange,
    errorMessage: $error,
    searchRunning: $searchRunning,
    filterActive,
    filterValue: $filter,
    cols: $cols,
    gridTemplate: $gridTemplate,
    filterSourceEntries: $filterSourceEntries,
    filteredEntries: $filteredEntries,
    visibleEntries: $visibleEntries,
    columnFilters: $columnFilters,
    columnFacets: $columnFacets,
    columnFacetsLoading: $columnFacetsLoading,
    ensureColumnFacets,
    start: $start,
    offsetY: $offsetY,
    totalHeight: $totalHeight,
    selected: $selected,
    sortField: $sortField,
    sortDirection: $sortDirection,
    isHidden,
    displayName,
    formatSize,
    formatItems,
    clipboardMode,
    clipboardPaths,
    handleRowsScrollCombined,
    handleWheelCombined,
    handleRowsKeydownCombined,
    handleRowsMouseDown,
    handleRowsClickSafe,
    handleBlankContextMenu,
    changeSort,
    toggleColumnFilter,
    resetColumnFilter,
    startResize,
    ariaSort,
    handleRowClickWithOpen,
    handleOpenEntry,
    handleRowContextMenu,
    toggleStar,
    handleRowDragStart,
    handleRowDragEnd,
    handleRowDragEnter,
    handleRowDragOver,
    handleRowDrop,
    handleRowDragLeave,
    dragTargetPath: $dragState.target,
    dragPathsLength: $dragState.paths.length,
    dragging: $dragState.dragging,
    handleBreadcrumbDragOver,
    handleBreadcrumbDragLeave,
    handleBreadcrumbDrop,
    selectionActive: $selectionActive,
    selectionRect: $selectionRect,
    videoThumbs: $videoThumbs,
    currentView,
    thumbnailRefreshToken,
    contextMenu: $contextMenu,
    blankMenu: $blankMenu,
    handleContextSelect,
    handleBlankContextAction,
    closeContextMenu,
    closeBlankContextMenu,
    deleteState: $deleteState,
    deleteModal,
    renameState: $renameState,
    confirmRename,
    closeRenameModal,
    advancedRenameState: $advancedRenameState,
    advancedRenameModal,
    compressState: $compressState,
    confirmCompress,
    closeCompress,
    checkDuplicatesState: $checkDuplicatesState,
    checkDuplicatesModal,
    copyCheckDuplicatesList,
    searchCheckDuplicates,
    closeCheckDuplicatesModal,
    newFolderState: $newFolderState,
    confirmNewFolder,
    closeNewFolderModal,
    newFileState: $newFileState,
    newFileTypeHint: $newFileTypeHint,
    confirmNewFile,
    closeNewFileModal,
    openWithState: $openWithState,
    openWithModal,
    propertiesState: $propertiesState,
    propertiesModal,
    bookmarkModalOpen,
    bookmarkCandidate,
    confirmBookmark,
    closeBookmarkModal,
    toastMessage: $toast,
    selectionText,
  }))

  $: settingsModalProps = createExplorerSettingsModalProps({
    settingsInitialFilter,
    defaultViewPref,
    showHidden: $showHidden,
    hiddenFilesLast: $hiddenFilesLast,
    foldersFirst: $foldersFirst,
    confirmDelete: $confirmDelete,
    density: $density,
    archiveName: $archiveName,
    archiveLevel: $archiveLevel,
    openDestAfterExtract: $openDestAfterExtract,
    videoThumbs: $videoThumbs,
    hardwareAcceleration: $hardwareAcceleration,
    ffmpegPath: $ffmpegPath,
    thumbCacheMb: $thumbCacheMb,
    mountsPollMs: $mountsPollMs,
    doubleClickMs: $doubleClickMs,
    startDir: $startDirPref ?? '~',
    sortField: $sortFieldPref,
    sortDirection: $sortDirectionPref,
    shortcuts: shortcutBindings,
    onChangeDefaultView: handleSettingsDefaultViewChange,
    onToggleShowHidden: toggleShowHidden,
    onToggleHiddenFilesLast: toggleHiddenFilesLast,
    onToggleFoldersFirst: toggleFoldersFirst,
    onToggleConfirmDelete: toggleConfirmDelete,
    onChangeStartDir: setStartDirPref,
    onChangeDensity: setDensityPref,
    onChangeArchiveName: setArchiveNamePref,
    onChangeArchiveLevel: setArchiveLevelPref,
    onToggleOpenDestAfterExtract: toggleOpenDestAfterExtract,
    onToggleVideoThumbs: toggleVideoThumbs,
    onToggleHardwareAcceleration: setHardwareAccelerationPref,
    onChangeFfmpegPath: setFfmpegPathPref,
    onChangeThumbCacheMb: setThumbCachePref,
    onChangeMountsPollMs: setMountsPollPref,
    onChangeDoubleClickMs: setDoubleClickMsPref,
    onClearThumbCache: clearThumbnailCacheFromSettings,
    onClearStars: clearStarsFromSettings,
    onClearBookmarks: clearBookmarksFromSettings,
    onClearRecents: clearRecentsFromSettings,
    onChangeSortField: setSortFieldPref,
    onChangeSortDirection: setSortDirectionPref,
    onChangeShortcut: updateShortcutBinding,
    onClose: pageUiState.closeSettings,
  })

  // --- Extraction seam: app lifecycle glue (Step 6) -----------------------
  const pageLifecycle = useExplorerPageLifecycle({
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
    fsLabel,
    activityApi,
    activity,
    showToast,
    abortDirStats,
    cleanupScrollHover,
    viewObservers,
    cancelToggleViewModeRaf,
    resetNewFileTypeHint,
    cancelSearch,
    stopDuplicateScan,
  })

  onDestroy(pageLifecycle.handlePageDestroy)
  onMount(pageLifecycle.initLifecycle)
</script>

<!-- Render root: keep composition at page level; push glue/helpers out over time. -->
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
<ExplorerPageOverlays
  dragGhostVisible={$dragState.dragging}
  dragGhostX={$dragState.position.x}
  dragGhostY={$dragState.position.y}
  dragGhostCount={$dragState.paths.length}
  dragGhostAllowed={$dragState.target !== null}
  dragGhostAction={$dragAction}
  textMenuOpen={$textMenuOpen}
  textMenuX={$textMenuX}
  textMenuY={$textMenuY}
  textMenuTarget={$textMenuTarget}
  textMenuReadonly={$textMenuReadonly}
  {shortcutBindings}
  {closeTextContextMenu}
  conflictModalOpen={$conflictModalOpen}
  conflictList={$conflictList}
  {cancelConflicts}
  renameAllConflicts={() => resolveConflicts('rename')}
  overwriteConflicts={() => resolveConflicts('overwrite')}
  {aboutOpen}
  closeAbout={pageUiState.closeAbout}
>
  <ExplorerShell
    bind:pathInput
    bind:pathInputEl
    bind:rowsEl={rowsElRef}
    bind:headerEl={headerElRef}
    bind:bookmarkName
    bind:bookmarkInputEl
    bind:renameValue
    bind:compressName
    bind:compressLevel
    bind:newFolderName
    bind:newFileName
    sidebarProps={explorerShellSidebarProps}
    topbarProps={explorerShellTopbarProps}
    listingProps={explorerShellListingProps}
    menuProps={explorerShellMenuProps}
    modalProps={explorerShellModalProps}
    statusProps={explorerShellStatusProps}
  />
</ExplorerPageOverlays>
{#if settingsOpen}
  <SettingsModal {...settingsModalProps} />
{/if}
