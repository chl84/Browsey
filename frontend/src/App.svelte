<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { get, writable } from 'svelte/store'
  import { formatItems, formatSelectionLine, formatSize, normalizePath, parentPath } from './features/explorer/utils'
  import { createListState } from './features/explorer/stores/listState'
  import ExplorerShell from './features/explorer/components/ExplorerShell.svelte'
  import { createExplorerState } from './features/explorer/state'
  import { createColumnResize } from './features/explorer/hooks/columnWidths'
  import { createGlobalShortcuts } from './features/explorer/hooks/shortcuts'
  import { createBookmarkModal } from './features/explorer/hooks/bookmarkModal'
  import { createDragDrop } from './features/explorer/hooks/dragDrop'
  import type { Entry, Partition, SortField } from './features/explorer/types'
  import { toast, showToast } from './features/explorer/hooks/useToast'
  import { createClipboard } from './features/explorer/hooks/useClipboard'
  import { createContextMenus } from './features/explorer/hooks/useContextMenus'
  import type { ContextAction } from './features/explorer/hooks/useContextMenus'
  import { createContextActions, type CurrentView } from './features/explorer/hooks/useContextActions'
  import { createSelectionBox } from './features/explorer/hooks/selectionBox'
  import { hitTestGridVirtualized } from './features/explorer/helpers/lassoHitTest'
  import { ensureSelectionBeforeMenu } from './features/explorer/helpers/contextMenuHelpers'
  import { moveCaret } from './features/explorer/helpers/navigationController'
  import {
    fetchOpenWithApps,
    openWithSelection,
    defaultOpenWithApp,
    type OpenWithApp,
    type OpenWithChoice,
  } from './features/explorer/openWith'
  import DragGhost from './ui/DragGhost.svelte'
  import ConflictModal from './ui/ConflictModal.svelte'
  import './features/explorer/ExplorerLayout.css'

  type ExtractResult = { destination: string; skipped_symlinks: number; skipped_entries: number }
  type ProgressPayload = { bytes: number; total: number; finished?: boolean }
  type ActivityState = {
    label: string
    percent: number | null
    cancel?: (() => void) | null
    cancelling?: boolean
  }
  type ViewMode = 'list' | 'grid'
  let sidebarCollapsed = false
  let pathInput = ''
  let mode: 'address' | 'filter' | 'search' = 'address'
  let viewMode: ViewMode = 'list'
  let inputFocused = false
  let rowsElRef: HTMLDivElement | null = null
  let gridElRef: HTMLDivElement | null = null
  let headerElRef: HTMLDivElement | null = null
  let pathInputEl: HTMLInputElement | null = null
  let unlistenDirChanged: UnlistenFn | null = null
  let unlistenEntryMeta: UnlistenFn | null = null
  let unlistenEntryMetaBatch: UnlistenFn | null = null
  let refreshTimer: ReturnType<typeof setTimeout> | null = null
  let rowsObserver: ResizeObserver | null = null
  let gridObserver: ResizeObserver | null = null
  let rowsRaf: number | null = null
  let gridRaf: number | null = null

  const places = [
    { label: 'Home', path: '~' },
    { label: 'Recent', path: '' },
    { label: 'Starred', path: '' },
    { label: 'Network', path: '' },
    { label: 'Wastebasket', path: 'trash://' },
  ]

  let bookmarks: { label: string; path: string }[] = []
  let partitions: Partition[] = []
  let partitionsPoll: ReturnType<typeof setInterval> | null = null
  const bookmarkModal = createBookmarkModal()
  const dragDrop = createDragDrop()
  const { store: bookmarkStore } = bookmarkModal
  const dragState = dragDrop.state
  let bookmarkModalOpen = false
  let bookmarkName = ''
  let bookmarkCandidate: Entry | null = null
  let bookmarkInputEl: HTMLInputElement | null = null
  let renameModalOpen = false
  let renameTarget: Entry | null = null
  let renameValue = ''
  let compressOpen = false
  let compressTargets: Entry[] = []
  let compressName = 'Archive.zip'
  let compressLevel = 6
  let compressError = ''
  let newFolderOpen = false
  let newFolderName = 'New folder'
  let newFolderError = ''
  let openWithOpen = false
  let openWithEntry: Entry | null = null
  let openWithApps: OpenWithApp[] = []
  let openWithLoading = false
  let openWithError = ''
  let openWithSubmitting = false
  let openWithLoadId = 0
  let propertiesOpen = false
  let propertiesEntry: Entry | null = null
  let propertiesCount = 1
  let propertiesSize: number | null = null
  let conflictModalOpen = false
  let conflictList: { src: string; target: string; is_dir: boolean }[] = []
  let conflictDest: string | null = null
  let deleteConfirmOpen = false
  let deleteTargets: Entry[] = []
  let clipboardMode: 'copy' | 'cut' = 'copy'
  let clipboardPaths = new Set<string>()
  let currentView: CurrentView = 'dir'
  let renameError = ''
  let lastLocation = ''
  let extracting = false
  let compressing = false
  let deleting = false
  let activity: ActivityState | null = null
  let activityUnlisten: UnlistenFn | null = null
  let activityHideTimer: ReturnType<typeof setTimeout> | null = null

  const queueActivityHide = () => {
    if (activityHideTimer) {
      clearTimeout(activityHideTimer)
    }
    activityHideTimer = setTimeout(() => {
      activity = null
      activityHideTimer = null
    }, 2000)
  }

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

  const explorer = createExplorerState({
    onEntriesChanged: () => resetScrollPosition(),
    onCurrentChange: (path) => {
      pathInput = path
    },
  })

  const startActivityListener = async (label: string, eventName: string, onCancel?: () => void) => {
    await cleanupActivityListener()
    if (activityHideTimer) {
      clearTimeout(activityHideTimer)
      activityHideTimer = null
    }
    activity = { label, percent: 0, cancel: onCancel ?? null, cancelling: false }
    activityUnlisten = await listen<ProgressPayload>(eventName, (event) => {
      const payload = event.payload
      let pct =
        payload.total > 0 ? Math.min(100, Math.round((payload.bytes / payload.total) * 100)) : null
      if (pct === 0 && payload.bytes > 0) {
        pct = 1
      }
      const existing = activity
      const cancelling = existing?.cancelling ?? false
      const displayLabel = cancelling ? 'Cancelling…' : label
      if (payload.finished) {
        activity = {
          label: cancelling ? 'Cancelling…' : 'Finalizing…',
          percent: pct ?? null,
          cancel: null,
          cancelling,
        }
        queueActivityHide()
      } else {
        activity = {
          label: displayLabel,
          percent: pct,
          cancel: cancelling ? null : existing?.cancel ?? onCancel ?? null,
          cancelling,
        }
      }
    })
  }

  const cleanupActivityListener = async (preserveTimer = false) => {
    if (activityUnlisten) {
      await activityUnlisten()
      activityUnlisten = null
    }
    if (!preserveTimer && activityHideTimer) {
      clearTimeout(activityHideTimer)
      activityHideTimer = null
    }
  }

  const requestCancel = async (eventName: string) => {
    if (!activity || activity.cancelling) return
    activity = { ...activity, label: 'Cancelling…', cancel: null, cancelling: true }
    try {
      await invoke('cancel_task', { id: eventName })
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      showToast(`Cancel failed: ${msg}`)
      activity = null
      await cleanupActivityListener()
    }
  }

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
    sortField,
    sortDirection,
    bookmarks: bookmarksStore,
    partitions: partitionsStore,
    filteredEntries,
    load,
    loadRecent,
    loadStarred,
    loadTrash,
    runSearch,
    toggleMode,
    changeSort,
    open,
    toggleStar,
    handlePlace,
    goToPath,
    goBack,
    goForward,
    loadBookmarks,
    loadPartitions,
    loadSavedWidths,
    persistWidths,
  } = explorer

  const selectionActive = selectionBox.active
  const selectionRect = selectionBox.rect

  const focusCurrentView = async () => {
    await tick()
    rowsElRef?.focus()
  }

  const toggleViewMode = async () => {
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
      recompute(get(filteredEntries))
    } else {
      await tick()
    }
    void focusCurrentView()
  }

  const {
    selected,
    anchorIndex,
    caretIndex,
    rowHeight,
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
  $: rowClickHandler = handleRowClick($filteredEntries)

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
  $: {
    const state = $bookmarkStore
    bookmarkModalOpen = state.open
    bookmarkName = state.name
    bookmarkCandidate = state.candidate as Entry | null
  }
  $: {
    if (bookmarkModalOpen) {
      bookmarkModal.setName(bookmarkName)
    }
  }

  let selectionDrag = false
  const GRID_CARD_WIDTH = 128
  const GRID_ROW_HEIGHT = 136
  const GRID_GAP = 6
  const GRID_OVERSCAN = 2
  const gridStart = writable(0)
  const gridOffsetY = writable(0)
  const gridTotalHeight = writable(0)
  let gridCols = 1
  const GRID_WHEEL_SCALE = 0.7
  let gridWheelRaf: number | null = null
  let gridPendingDeltaX = 0
  let gridPendingDeltaY = 0
  let cursorX = 0
  let cursorY = 0

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

  const { startResize } = createColumnResize(cols, persistWidths)

  let dirSizeAbort = 0
  let propertiesToken = 0
  let selectionText = ''
  type Access = { read: boolean; write: boolean; exec: boolean }
  type PermissionsState = {
    accessSupported: boolean
    owner: Access | null
    group: Access | null
    other: Access | null
  }

  let propertiesItemCount: number | null = null
  let propertiesPermissions: PermissionsState | null = null

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

    const parts = [dirLine, fileLine].filter((p) => p.length > 0)
    selectionText = parts.join(' | ')
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

  const handleResize = () => {
    if (typeof window === 'undefined') return
    sidebarCollapsed = window.innerWidth < 700
    handleListResize()
    if (viewMode === 'grid') {
      recomputeGrid()
    }
  }

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
        if (pathInput.length === 0) {
          await enterAddressMode()
          focusPathInput()
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
      return true
    },
    onPaste: async () => {
      if (currentView === 'recent') return false
      return pasteIntoCurrent()
    },
    onRename: async () => {
      if ($selected.size !== 1) return false
      const path = Array.from($selected)[0]
      const entry = $entries.find((e) => e.path === path)
      if (!entry) return false
      renameTarget = entry
      renameValue = entry.name
      renameModalOpen = true
      return true
    },
    onDelete: async (permanent) => {
      const paths = Array.from($selected)
      if (paths.length === 0) return false
      const entries = $filteredEntries.filter((e) => paths.includes(e.path))
      if (entries.length === 0) return false
      const hasNetwork = entries.some((e) => e.network)

      if (permanent || (hasNetwork && currentView !== 'trash')) {
        deleteTargets = entries
        deleteConfirmOpen = true
        return true
      }
      const label = currentView === 'trash' ? 'Deleting…' : 'Moving to trash…'
      const total = entries.length
      await cleanupActivityListener()
      if (activityHideTimer) {
        clearTimeout(activityHideTimer)
        activityHideTimer = null
      }
      activity = { label, percent: total > 0 ? 0 : null }
      try {
        if (currentView === 'trash') {
          let done = 0
          for (const p of entries.map((e) => e.path)) {
            await invoke('delete_entry', { path: p })
            done += 1
            activity = {
              label,
              percent: total > 0 ? Math.round((done / total) * 100) : null,
            }
          }
        } else {
          let done = 0
          for (const p of entries.map((e) => e.path)) {
            await invoke('move_to_trash', { path: p })
            done += 1
            activity = {
              label,
              percent: total > 0 ? Math.round((done / total) * 100) : null,
            }
          }
        }
        await reloadCurrent()
      } catch (err) {
        console.error('Failed to move to trash', err)
      } finally {
        queueActivityHide()
      }
      return true
    },
    onProperties: async () => {
      if ($selected.size === 0) return false
      const selection = $entries.filter((e) => $selected.has(e.path))
      if (selection.length === 0) return false
      void openPropertiesModal(selection)
      return true
    },
    onOpenConsole: async () => {
      if (currentView !== 'dir') return false
      try {
        await invoke('open_console', { path: get(current) })
        return true
      } catch (err) {
        showToast(`Open console failed: ${err instanceof Error ? err.message : String(err)}`)
        return false
      }
    },
    onToggleView: async () => toggleViewMode(),
  })
  const { handleGlobalKeydown } = shortcuts

  const handleDocumentKeydown = (event: KeyboardEvent) => {
    const key = event.key.toLowerCase()
    const inRows = rowsElRef?.contains(event.target as Node) ?? false

    if ((event.ctrlKey || event.metaKey) && !isEditableTarget(event.target)) {
      if (event.shiftKey && key === 'i') {
        return
      }
      const allowed = new Set(['f', 'b', 'c', 'x', 'v', 'p', 'a', 't', 'g', 'z', 'y'])
      if (!allowed.has(key)) {
        event.preventDefault()
        event.stopPropagation()
        return
      }
    }

    if (key === 'arrowdown' && !isEditableTarget(event.target) && rowsElRef && !inRows) {
      const list = get(filteredEntries)
      if (list.length > 0) {
        event.preventDefault()
        event.stopPropagation()
        selected.set(new Set([list[0].path]))
        anchorIndex.set(0)
        caretIndex.set(0)
        const firstRow = rowsElRef.querySelector<HTMLButtonElement>('.row-viewport .row')
        if (firstRow) {
          firstRow.focus()
        } else {
          rowsElRef.focus()
        }
        return
      }
    }
    if (key === 'escape') {
      if (deleteConfirmOpen) {
        event.preventDefault()
        event.stopPropagation()
        closeDeleteConfirm()
        return
      }
      if (renameModalOpen) {
        event.preventDefault()
        event.stopPropagation()
        closeRenameModal()
        return
      }
      if (openWithOpen) {
        event.preventDefault()
        event.stopPropagation()
        closeOpenWith()
        return
      }
      if (propertiesOpen) {
        event.preventDefault()
        event.stopPropagation()
        closeProperties()
        return
      }
      if (compressOpen) {
        event.preventDefault()
        event.stopPropagation()
        closeCompress()
        return
      }
      if (newFolderOpen) {
        event.preventDefault()
        event.stopPropagation()
        closeNewFolderModal()
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
    if ((event.ctrlKey || event.metaKey) && !isEditableTarget(event.target)) {
      if (key === 'z') {
        event.preventDefault()
        event.stopPropagation()
        void invoke('undo_action').catch((err) =>
          showToast(`Undo failed: ${err instanceof Error ? err.message : String(err)}`)
        )
        return
      }
      if (key === 'y') {
        event.preventDefault()
        event.stopPropagation()
        void invoke('redo_action').catch((err) =>
          showToast(`Redo failed: ${err instanceof Error ? err.message : String(err)}`)
        )
        return
      }
    }

    void handleGlobalKeydown(event)
  }

  $: updateViewportHeight()
  const recomputeGrid = () => {
    if (!gridElRef || viewMode !== 'grid') return
    const list = get(filteredEntries)
    const width = Math.max(0, gridElRef.clientWidth)
    gridCols = Math.max(1, Math.floor((width + GRID_GAP) / (GRID_CARD_WIDTH + GRID_GAP)))
    const rowStride = GRID_ROW_HEIGHT + GRID_GAP
    const totalRows = Math.ceil(list.length / gridCols)
    const scrollTop = gridElRef.scrollTop
    const viewport = gridElRef.clientHeight
    const startRow = Math.max(0, Math.floor(scrollTop / rowStride) - GRID_OVERSCAN)
    const endRow = Math.min(totalRows, Math.ceil((scrollTop + viewport) / rowStride) + GRID_OVERSCAN)
    const startIdx = startRow * gridCols
    const endIdx = Math.min(list.length, endRow * gridCols)
    gridStart.set(startIdx)
    gridOffsetY.set(startRow * rowStride)
    gridTotalHeight.set(totalRows * rowStride)
    visibleEntries.set(list.slice(startIdx, endIdx))
    start.set(startIdx)
    offsetY.set(startRow * rowStride)
    totalHeight.set(totalRows * rowStride)
  }

  $: {
    if (viewMode === 'list') {
      // Recompute virtualization when viewport or scroll changes.
      $viewportHeight
      $scrollTop
      recompute($filteredEntries)
    } else {
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
      void invoke('add_bookmark', { label, path })
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
      const actions = await invoke<ContextAction[]>('context_menu_actions', {
        count: selectionCount,
        kind: entry.kind,
        starred: Boolean(entry.starred),
        view: currentView,
        clipboardHasItems: clipboardPaths.size > 0,
      })
      const filtered = actions.filter((action) => action.id !== 'new-folder')
      if (filtered.length > 0) {
        openContextMenu(entry, filtered, event.clientX, event.clientY)
      }
    } catch (err) {
      console.error('Failed to load context menu actions', err)
    }
  }

  const pasteIntoCurrent = async () => {
    const ok = await handlePasteOrMove($current)
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
    await load($current, { recordHistory: false })
  }

  const contextActions = createContextActions({
    getSelectedPaths: () => Array.from($selected),
    getSelectedSet: () => $selected,
    getFilteredEntries: () => $filteredEntries,
    currentView: () => currentView,
    reloadCurrent,
    clipboard,
    showToast,
    openWith: (entry) => {
      openWithApps = [defaultOpenWithApp]
      openWithError = ''
      openWithLoading = false
      openWithSubmitting = false
      openWithLoadId += 1
      openWithEntry = entry
      openWithOpen = true
    },
    openCompress: (entries) => {
      compressTargets = entries
      if (entries.length === 1) {
        const name = entries[0].name
        compressName = name.toLowerCase().endsWith('.zip') ? name : `${name}.zip`
      } else {
        compressName = 'Archive.zip'
      }
      compressLevel = 6
      compressError = ''
      compressOpen = true
    },
    startRename: (entry) => {
      renameTarget = entry
      renameValue = entry.name
      renameModalOpen = true
    },
    confirmDelete: (entries) => {
      deleteTargets = entries
      deleteConfirmOpen = true
    },
    openProperties: (entries) => {
      void openPropertiesModal(entries)
    },
    openLocation: (entry) => {
      void openEntryLocation(entry)
    },
  })

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
      name.endsWith('.gz') ||
      name.endsWith('.bz2') ||
      name.endsWith('.xz') ||
      name.endsWith('.zst')
    )
  }

  const extractArchiveEntry = async (entry: Entry) => {
    if (extracting) return
    extracting = true
    const progressEvent = `extract-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    await startActivityListener('Extracting…', progressEvent, () => requestCancel(progressEvent))
    try {
      const result = await invoke<ExtractResult>('extract_archive', {
        path: entry.path,
        progressEvent,
      })
      await reloadCurrent()
      const skippedSymlinks = result?.skipped_symlinks ?? 0
      const skippedOther = result?.skipped_entries ?? 0
      const skipParts = []
      if (skippedSymlinks > 0) {
        skipParts.push(`${skippedSymlinks} symlink${skippedSymlinks === 1 ? '' : 's'}`)
      }
      if (skippedOther > 0) {
        skipParts.push(`${skippedOther} entr${skippedOther === 1 ? 'y' : 'ies'}`)
      }
      const suffix = skipParts.length > 0 ? ` (skipped ${skipParts.join(', ')})` : ''
      showToast(`Extracted to ${result.destination}${suffix}`)
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      if (msg.toLowerCase().includes('cancelled')) {
        showToast('Extraction cancelled')
      } else {
        showToast(`Failed to extract: ${msg}`)
      }
    } finally {
      extracting = false
      activity = null
      await cleanupActivityListener()
    }
  }

  const handleOpenEntry = async (entry: Entry) => {
    if (entry.kind === 'dir') {
      mode = 'address'
      searchActive.set(false)
      filter.set('')
      if ($searchMode) {
        await toggleMode(false)
      }
      pathInput = entry.path
      await load(entry.path)
      return
    }
    if (isExtractableArchive(entry)) {
      await extractArchiveEntry(entry)
      return
    }
    open(entry)
  }

  const openEntryLocation = async (entry: Entry) => {
    const dir = parentPath(entry.path)
    await load(dir)
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

  const handleRowsMouseDown = (event: MouseEvent) => {
    const target = event.target as HTMLElement | null
    if (viewMode === 'list') {
      if (!rowsElRef) return
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
        rowHeight,
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
    if (target && target.closest('.card')) return
    const gridEntries = get(filteredEntries)
    if (gridEntries.length === 0) return
    event.preventDefault()
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
        gridCols,
        cardWidth: GRID_CARD_WIDTH,
        cardHeight: GRID_ROW_HEIGHT,
        gap: GRID_GAP,
        padding: GRID_GAP,
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

  const handleGridScroll = () => {
    if (viewMode !== 'grid') return
    recomputeGrid()
  }

  const handleGridWheel = (event: WheelEvent) => {
    const el = gridElRef
    if (!el) return
    gridPendingDeltaX += event.deltaX * GRID_WHEEL_SCALE
    gridPendingDeltaY += event.deltaY * GRID_WHEEL_SCALE
    if (gridWheelRaf !== null) return
    gridWheelRaf = requestAnimationFrame(() => {
      el.scrollLeft += gridPendingDeltaX
      el.scrollTop += gridPendingDeltaY
      gridPendingDeltaX = 0
      gridPendingDeltaY = 0
      gridWheelRaf = null
    })
  }

  const handleRowsScrollCombined = (event: Event) => {
    if (viewMode === 'list') {
      handleRowsScroll()
    } else {
      handleGridScroll()
    }
  }

  const ensureGridVisible = (index: number) => {
    if (!gridElRef || gridCols <= 0) return
    const rowStride = GRID_ROW_HEIGHT + GRID_GAP
    const row = Math.floor(index / gridCols)
    const top = row * rowStride
    const bottom = top + rowStride
    const currentTop = gridElRef.scrollTop
    const currentBottom = currentTop + gridElRef.clientHeight
    if (top < currentTop) {
      gridElRef.scrollTo({ top })
    } else if (bottom > currentBottom) {
      gridElRef.scrollTo({ top: bottom - gridElRef.clientHeight })
    }
  }

  const handleGridKeydown = (event: KeyboardEvent) => {
    if (viewMode !== 'grid') return
    const list = get(filteredEntries)
    if (list.length === 0) return
    const key = event.key.toLowerCase()
    const selectedSet = get(selected)
    const ctrl = event.ctrlKey || event.metaKey
    if (ctrl && key === 'a') {
      event.preventDefault()
      event.stopPropagation()
      selected.set(new Set(list.map((e) => e.path)))
      anchorIndex.set(0)
      caretIndex.set(list.length - 1)
      return
    }
    if (key === 'escape') {
      event.preventDefault()
      event.stopPropagation()
      selected.set(new Set())
      anchorIndex.set(null)
      caretIndex.set(null)
      return
    }
    if (key === 'enter') {
      event.preventDefault()
      event.stopPropagation()
      const idx =
        get(caretIndex) ??
        get(anchorIndex) ??
        list.findIndex((entry) => selectedSet.has(entry.path))
      if (idx !== null && idx >= 0) {
        const entry = list[idx]
        if (entry) {
          void handleOpenEntry(entry)
        }
      }
      return
    }

    const current = get(caretIndex) ?? get(anchorIndex)
    const rowDelta = Math.max(1, gridCols)
    let next: number | null = null
    if (key === 'arrowright') next = moveCaret({ count: list.length, current, delta: 1 })
    else if (key === 'arrowleft') next = moveCaret({ count: list.length, current, delta: -1 })
    else if (key === 'arrowdown') next = moveCaret({ count: list.length, current, delta: rowDelta })
    else if (key === 'arrowup') next = moveCaret({ count: list.length, current, delta: -rowDelta })
    else if (key === 'home') next = moveCaret({ count: list.length, current, toStart: true })
    else if (key === 'end') next = moveCaret({ count: list.length, current, toEnd: true })
    else return

    if (next === null) return
    event.preventDefault()
    event.stopPropagation()

    if (event.shiftKey) {
      const anchor = get(anchorIndex) ?? current ?? next
      const lo = Math.min(anchor, next)
      const hi = Math.max(anchor, next)
      const range = new Set<string>()
      for (let i = lo; i <= hi; i++) {
        const path = list[i]?.path
        if (path) range.add(path)
      }
      selected.set(range)
      anchorIndex.set(anchor)
      caretIndex.set(next)
    } else {
      const path = list[next]?.path
      if (path) {
        selected.set(new Set([path]))
        anchorIndex.set(next)
        caretIndex.set(next)
      }
    }
    ensureGridVisible(next)
  }

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

  const handleRowsClickSafe = (event: MouseEvent) => {
    if (selectionDrag) {
      selectionDrag = false
      return
    }
    if (viewMode === 'grid') {
      const target = event.target as HTMLElement | null
      if (target && target.closest('.card')) return
      if (get(selected).size > 0) {
        selected.set(new Set())
        anchorIndex.set(null)
        caretIndex.set(null)
      }
      return
    }
    handleRowsClick(event)
  }

  const handleRowContextMenu = (entry: Entry, event: MouseEvent) => {
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
    selected.set(new Set())
    anchorIndex.set(null)
    caretIndex.set(null)
    const actions: ContextAction[] = [
      { id: 'new-folder', label: 'New Folder…' },
      { id: 'open-console', label: 'Open in console', shortcut: 'Ctrl+T' },
    ]
    if (hasClipboardItems) {
      actions.push({ id: 'paste', label: 'Paste', shortcut: 'Ctrl+V' })
    }
    openBlankContextMenu(actions, event.clientX, event.clientY)
  }

  const handleBlankContextAction = async (id: string) => {
    if (currentView !== 'dir') return
    closeBlankContextMenu()
    if (id === 'new-folder') {
      newFolderName = 'New folder'
      newFolderError = ''
      newFolderOpen = true
      return
    }
    if (id === 'open-console') {
      try {
        await invoke('open_console', { path: get(current) })
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
      newFolderName = 'New folder'
      newFolderError = ''
      newFolderOpen = true
      return
    }
    await contextActions(id, entry)
  }

  let dragPaths: string[] = []
  const handleRowDragStart = (entry: Entry, event: DragEvent) => {
    const selectedPaths = $selected.has(entry.path) ? Array.from($selected) : [entry.path]
    dragPaths = selectedPaths
    dragDrop.start(selectedPaths, event)
  }

  const handleRowDragEnd = () => {
    dragPaths = []
    dragDrop.end()
  }

  const handleRowDragOver = (entry: Entry, event: DragEvent) => {
    if (entry.kind !== 'dir') return
    const allowed = dragDrop.canDropOn(dragPaths, entry.path)
    dragDrop.setTarget(allowed ? entry.path : null)
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = allowed ? 'move' : 'none'
    }
    dragDrop.setPosition(event.clientX, event.clientY)
  }

  const handleRowDragEnter = (entry: Entry, event: DragEvent) => {
    handleRowDragOver(entry, event)
  }

  const handleRowDrop = async (entry: Entry, event: DragEvent) => {
    if (entry.kind !== 'dir') return
    if (!dragDrop.canDropOn(dragPaths, entry.path)) return
    event.preventDefault()
    try {
      if (dragPaths.length > 0) {
        await invoke('set_clipboard_cmd', { paths: dragPaths, mode: 'cut' })
      }
      await handlePasteOrMove(entry.path)
    } catch (err) {
      showToast(`Move failed: ${err instanceof Error ? err.message : String(err)}`)
    } finally {
      handleRowDragEnd()
    }
  }

  const handleRowDragLeave = () => {
    dragDrop.setTarget(null)
  }

  const handlePasteOrMove = async (dest: string) => {
    try {
      const conflicts = await invoke<{ src: string; target: string; is_dir: boolean }[]>(
        'paste_clipboard_preview',
        { dest }
      )
      if (conflicts && conflicts.length > 0) {
        const destNorm = normalizePath(dest)
        const selfPaste = conflicts.every((c) => normalizePath(parentPath(c.src)) === destNorm)
        if (selfPaste) {
          await invoke('paste_clipboard_cmd', { dest, policy: 'rename' })
          await reloadCurrent()
          return true
        }
        conflictList = conflicts
        conflictDest = dest
        conflictModalOpen = true
        return false
      }
      await invoke('paste_clipboard_cmd', { dest, policy: 'rename' })
      await reloadCurrent()
      return true
    } catch (err) {
      showToast(`Paste failed: ${err instanceof Error ? err.message : String(err)}`)
      return false
    }
  }

  const resolveConflicts = async (policy: 'rename' | 'overwrite') => {
    if (!conflictDest) return
    conflictModalOpen = false
    try {
      await invoke('paste_clipboard_cmd', { dest: conflictDest, policy })
      await reloadCurrent()
    } catch (err) {
      showToast(`Paste failed: ${err instanceof Error ? err.message : String(err)}`)
    } finally {
      conflictDest = null
      conflictList = []
    }
  }

  const closeRenameModal = () => {
    renameModalOpen = false
    renameTarget = null
    renameError = ''
  }

  const closeNewFolderModal = () => {
    newFolderOpen = false
    newFolderError = ''
  }

  const confirmNewFolder = async () => {
    if (currentView !== 'dir') {
      closeNewFolderModal()
      showToast('Cannot create folder here')
      return
    }
    const name = newFolderName.trim()
    if (!name) {
      newFolderError = 'Folder name cannot be empty'
      return
    }
    const base = get(current)
    try {
      const created: string = await invoke('create_folder', { path: base, name })
      await load(base, { recordHistory: false })
      newFolderOpen = false
      newFolderError = ''
      selected.set(new Set([created]))
      anchorIndex.set(null)
      caretIndex.set(null)
    } catch (err) {
      newFolderError = err instanceof Error ? err.message : String(err)
    }
  }

  const confirmRename = async (name: string) => {
    if (!renameTarget) return
    const trimmed = name.trim()
    if (!trimmed) return
    try {
      await invoke('rename_entry', { path: renameTarget.path, newName: trimmed })
      await load(parentPath(renameTarget.path), { recordHistory: false })
      closeRenameModal()
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      renameError = msg
      return
    }
  }

  const loadOpenWithApps = async (targetPath?: string) => {
    const path = targetPath ?? openWithEntry?.path
    if (!path) return
    const requestId = ++openWithLoadId
    openWithLoading = true
    openWithError = ''
    try {
      const list = await fetchOpenWithApps(path)
      if (!openWithOpen || !openWithEntry || openWithEntry.path !== path || requestId !== openWithLoadId) {
        return
      }
      openWithApps = [defaultOpenWithApp, ...list]
    } catch (err) {
      if (!openWithOpen || !openWithEntry || openWithEntry.path !== path || requestId !== openWithLoadId) {
        return
      }
      openWithApps = [defaultOpenWithApp]
      openWithError = err instanceof Error ? err.message : String(err)
    } finally {
      if (requestId === openWithLoadId) {
        openWithLoading = false
      }
    }
  }

  $: if (openWithOpen && openWithEntry) {
    void loadOpenWithApps(openWithEntry.path)
  }

  const handleOpenWithConfirm = async (choice: OpenWithChoice) => {
    if (!openWithEntry || openWithSubmitting) return
    const normalized: OpenWithChoice = {
      appId: choice.appId ?? undefined,
      customCommand: choice.customCommand?.trim() || undefined,
      customArgs: choice.customArgs?.trim() || undefined,
    }
    const hasCustom = Boolean(normalized.customCommand)
    const hasApp = Boolean(normalized.appId)
    if (!hasCustom && !hasApp) {
      openWithError = 'Pick an application or enter a command.'
      return
    }
    openWithSubmitting = true
    openWithError = ''
    try {
      await openWithSelection(openWithEntry.path, normalized)
      showToast(`Opening ${openWithEntry.name}…`)
      closeOpenWith()
    } catch (err) {
      openWithError = err instanceof Error ? err.message : String(err)
    } finally {
      openWithSubmitting = false
    }
  }

  const closeOpenWith = () => {
    openWithOpen = false
    openWithEntry = null
    openWithApps = []
    openWithError = ''
    openWithLoading = false
    openWithSubmitting = false
  }

  const closeCompress = () => {
    compressOpen = false
    compressTargets = []
    compressError = ''
  }

  const confirmCompress = async (name: string, level: number) => {
    if (compressTargets.length === 0) {
      closeCompress()
      return
    }
    if (compressing) return
    compressing = true
    const lvl = Math.min(Math.max(Math.round(level), 0), 9)
    const paths = compressTargets.map((e) => e.path)
    const base = get(current)
    const progressEvent = `compress-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      await startActivityListener('Compressing…', progressEvent, () => requestCancel(progressEvent))
      const dest = await invoke<string>('compress_entries', {
        paths,
        name,
        level: lvl,
        progressEvent,
      })
      await load(base, { recordHistory: false })
      closeCompress()
      showToast(`Created ${dest}`)
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      if (msg.toLowerCase().includes('cancelled')) {
        compressError = ''
        closeCompress()
        showToast('Compression cancelled')
      } else {
        compressError = msg
      }
    } finally {
      compressing = false
      activity = null
      await cleanupActivityListener()
    }
  }

  const closeProperties = () => {
    propertiesOpen = false
    propertiesEntry = null
    propertiesSize = null
    propertiesItemCount = null
    propertiesPermissions = null
  }

  const loadEntryTimes = async (entry: Entry, token: number) => {
    try {
      const times = await invoke<{ accessed?: string | null; created?: string | null; modified?: string | null }>(
        'entry_times_cmd',
        { path: entry.path }
      )
      if (token === propertiesToken) {
        propertiesEntry = { ...entry, ...times }
      }
    } catch (err) {
      console.error('Failed to load entry times', err)
    }
  }

  const closeDeleteConfirm = () => {
    deleteConfirmOpen = false
    deleteTargets = []
  }

  const confirmDelete = async () => {
    if (deleteTargets.length === 0 || deleting) return
    deleting = true
    const paths = deleteTargets.map((t) => t.path)
    const progressEvent = `delete-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      await startActivityListener('Deleting…', progressEvent)
      await invoke('delete_entries', { paths, progressEvent })
      await reloadCurrent()
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      console.error('Failed to delete', err)
      showToast(`Delete failed: ${msg}`)
    } finally {
      deleting = false
      const hadTimer = activityHideTimer !== null
      await cleanupActivityListener(true)
      if (!hadTimer) {
        activity = null
        if (activityHideTimer) {
          clearTimeout(activityHideTimer)
          activityHideTimer = null
        }
      }
      closeDeleteConfirm()
    }
  }

  const openPropertiesModal = async (entries: Entry[]) => {
    const token = ++propertiesToken
    propertiesEntry = entries.length === 1 ? entries[0] : null
    propertiesCount = entries.length
    propertiesItemCount = null
    propertiesPermissions = null
    propertiesOpen = true
    const files = entries.filter((e) => e.kind === 'file')
    const dirs = entries.filter((e) => e.kind === 'dir')
    const fileBytes = files.reduce((sum, f) => sum + (f.size ?? 0), 0)
    const fileCount = files.length
    propertiesSize = fileBytes
    let loadingDirs = dirs.length > 0

    if (dirs.length > 0) {
      const { total, items } = await computeDirStats(
        dirs.map((d) => d.path),
        (partialBytes) => {
          if (token === propertiesToken) {
            propertiesSize = fileBytes + partialBytes
          }
        }
      )
      if (token === propertiesToken) {
        propertiesSize = fileBytes + total
        propertiesItemCount = fileCount + items
        loadingDirs = false
      }
    } else {
      propertiesItemCount = fileCount
      loadingDirs = false
    }

    if (entries.length === 1 && propertiesEntry) {
      try {
        const perms = await invoke<{
          access_supported: boolean
          owner?: Access
          group?: Access
          other?: Access
        }>('get_permissions', { path: propertiesEntry.path })
        if (token === propertiesToken) {
          propertiesPermissions = {
            accessSupported: perms.access_supported,
            owner: perms.owner ?? null,
            group: perms.group ?? null,
            other: perms.other ?? null,
          }
        }
      } catch (err) {
        console.error('Failed to load permissions', err)
      }
      void loadEntryTimes(propertiesEntry, token)
    }
  }

  const updatePermissions = async (opts: {
    owner?: Partial<Access>
    group?: Partial<Access>
    other?: Partial<Access>
  }) => {
    if (!propertiesEntry) {
      console.warn('updatePermissions called with no propertiesEntry', opts)
      showToast('Permissions are unavailable for this selection.')
      return
    }
    const payload: {
      path: string
      owner?: Partial<Access>
      group?: Partial<Access>
      other?: Partial<Access>
    } = { path: propertiesEntry.path }
    if (opts.owner && Object.keys(opts.owner).length > 0) {
      payload.owner = { ...opts.owner }
    }
    if (opts.group && Object.keys(opts.group).length > 0) {
      payload.group = { ...opts.group }
    }
    if (opts.other && Object.keys(opts.other).length > 0) {
      payload.other = { ...opts.other }
    }
    try {
      const perms = await invoke<{
        access_supported?: boolean
        owner?: Access
        group?: Access
        other?: Access
      }>('set_permissions', payload)
      propertiesPermissions = {
        accessSupported: perms.access_supported ?? propertiesPermissions?.accessSupported ?? false,
        owner: perms.owner ?? propertiesPermissions?.owner ?? null,
        group: perms.group ?? propertiesPermissions?.group ?? null,
        other: perms.other ?? propertiesPermissions?.other ?? null,
      }
    } catch (err) {
      console.error('Failed to update permissions', { path: propertiesEntry.path, opts, err })
      const msg = err instanceof Error ? err.message : String(err)
      showToast(`Permissions update failed: ${msg}`)
    }
  }

  const toggleAccess = (scope: 'owner' | 'group' | 'other', key: 'read' | 'write' | 'exec', next: boolean) => {
    if (!propertiesPermissions || !propertiesPermissions.accessSupported) return
    updatePermissions({ [scope]: { [key]: next } })
  }

  onMount(() => {
    handleResize()
    window.addEventListener('resize', handleResize)
    void loadSavedWidths()
    void loadBookmarks()
    void loadPartitions()
    partitionsPoll = setInterval(() => {
      void loadPartitions()
    }, 2000)
    void load()

    // Listen for backend watcher events to refresh the current directory.
    void (async () => {
      unlistenDirChanged = await listen<string>('dir-changed', (event) => {
        const curr = get(current)
        const payload = event.payload
        if (!curr || payload === curr) {
          if (refreshTimer) {
            clearTimeout(refreshTimer)
          }
          refreshTimer = setTimeout(() => {
            const latest = get(current)
            if (!latest || latest !== payload) return
            void load(latest, { recordHistory: false, silent: true })
          }, 300)
        }
      })
      unlistenEntryMeta = await listen<Entry>('entry-meta', (event) => {
        const update = event.payload
        entries.update((list) => {
          const idx = list.findIndex((e) => e.path === update.path)
          if (idx === -1) return list
          const next = [...list]
          next[idx] = { ...next[idx], ...update }
          return next
        })
      })
      unlistenEntryMetaBatch = await listen<Entry[]>('entry-meta-batch', (event) => {
        const updates = event.payload
        if (!Array.isArray(updates) || updates.length === 0) return
        const map = new Map(updates.map((u) => [u.path, u]))
        entries.update((list) => list.map((item) => (map.has(item.path) ? { ...item, ...map.get(item.path)! } : item)))
      })
    })()
    return () => {
      window.removeEventListener('resize', handleResize)
      if (refreshTimer) {
        clearTimeout(refreshTimer)
        refreshTimer = null
      }
      rowsObserver?.disconnect()
      if (unlistenDirChanged) {
        unlistenDirChanged()
        unlistenDirChanged = null
      }
      if (unlistenEntryMeta) {
        unlistenEntryMeta()
        unlistenEntryMeta = null
      }
      if (unlistenEntryMetaBatch) {
        unlistenEntryMetaBatch()
        unlistenEntryMetaBatch = null
      }
      if (partitionsPoll) {
        clearInterval(partitionsPoll)
        partitionsPoll = null
      }
      dirSizeAbort++
    }
  })

  onMount(() => {
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
    window.addEventListener('error', handleError)
    window.addEventListener('unhandledrejection', handleRejection)
    return () => {
      window.removeEventListener('error', handleError)
      window.removeEventListener('unhandledrejection', handleRejection)
    }
  })
</script>

<svelte:document
  on:keydown|capture={handleDocumentKeydown}
  on:cut|capture={(e) => {
    const target = e.target as HTMLElement | null
    if (target && (target.isContentEditable || ['input', 'textarea'].includes(target.tagName.toLowerCase()))) {
      return
    }
    e.preventDefault()
    e.stopPropagation()
  }}
  on:contextmenu|capture|preventDefault
/>
<DragGhost
  visible={$dragState.dragging}
  x={$dragState.position.x}
  y={$dragState.position.y}
  count={$dragState.paths.length}
  allowed={$dragState.target !== null}
  target={$dragState.target}
/>
  <ExplorerShell
    bind:pathInput
    bind:pathInputEl
    bind:rowsEl={rowsElRef}
  bind:headerEl={headerElRef}
  bind:bookmarkName
  bind:bookmarkInputEl
  {viewMode}
  {sidebarCollapsed}
  {places}
  {bookmarks}
  {partitions}
  onPlaceSelect={handlePlace}
  onBookmarkSelect={(path) => void load(path)}
  onRemoveBookmark={(path) => {
    void invoke('remove_bookmark', { path })
    bookmarksStore.update((list) => list.filter((b) => b.path !== path))
  }}
  onPartitionSelect={(path) => void load(path)}
  searchMode={$searchMode}
  loading={$loading}
  activity={activity}
  onFocus={handleInputFocus}
  onBlur={handleInputBlur}
  onSubmitPath={submitPath}
  onSearch={() => runSearch(pathInput)}
  onExitSearch={() => void setSearchModeState(false).then(() => blurPathInput())}
  onNavigateSegment={(path) => void navigateToBreadcrumb(path)}
  noticeMessage={$error}
  searchActive={$searchActive}
  {mode}
  filterValue={$filter}
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
    onRowClick={rowClickHandler}
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
    {selectionText}
    selectionActive={$selectionActive}
    selectionRect={$selectionRect}
    contextMenu={$contextMenu}
  blankMenu={$blankMenu}
  onContextSelect={handleContextSelect}
  onBlankContextSelect={handleBlankContextAction}
  onCloseContextMenu={closeContextMenu}
  onCloseBlankContextMenu={closeBlankContextMenu}
  deleteConfirmOpen={deleteConfirmOpen}
  {deleteTargets}
  onConfirmDelete={confirmDelete}
  onCancelDelete={closeDeleteConfirm}
  renameModalOpen={renameModalOpen}
  {renameTarget}
  renameError={renameError}
  bind:renameValue
  onConfirmRename={confirmRename}
  onCancelRename={closeRenameModal}
  compressOpen={compressOpen}
  bind:compressName
  bind:compressLevel
  compressError={compressError}
  onConfirmCompress={confirmCompress}
  onCancelCompress={closeCompress}
  newFolderOpen={newFolderOpen}
  bind:newFolderName
  newFolderError={newFolderError}
  onConfirmNewFolder={confirmNewFolder}
  onCancelNewFolder={closeNewFolderModal}
  openWithOpen={openWithOpen}
  openWithApps={openWithApps}
  openWithLoading={openWithLoading}
  openWithError={openWithError}
  openWithBusy={openWithSubmitting}
  onConfirmOpenWith={handleOpenWithConfirm}
  onCloseOpenWith={closeOpenWith}
  propertiesOpen={propertiesOpen}
  {propertiesEntry}
  {propertiesCount}
  {propertiesSize}
  {propertiesItemCount}
  propertiesPermissions={propertiesPermissions}
  onTogglePermissionsAccess={(scope, key, next) => toggleAccess(scope, key, next)}
  onCloseProperties={closeProperties}
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
