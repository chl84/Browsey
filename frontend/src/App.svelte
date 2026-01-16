<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { get } from 'svelte/store'
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
  import DragGhost from './ui/DragGhost.svelte'
  import ConflictModal from './ui/ConflictModal.svelte'
  import './features/explorer/ExplorerLayout.css'

  type ExtractResult = { destination: string; skipped_symlinks: number; skipped_entries: number }
  type ExtractProgressPayload = { bytes: number; total: number; finished?: boolean }
  let sidebarCollapsed = false
  let pathInput = ''
  let mode: 'address' | 'filter' | 'search' = 'address'
  let inputFocused = false
  let rowsElRef: HTMLDivElement | null = null
  let headerElRef: HTMLDivElement | null = null
  let pathInputEl: HTMLInputElement | null = null
  let unlistenDirChanged: UnlistenFn | null = null
  let unlistenEntryMeta: UnlistenFn | null = null
  let unlistenEntryMetaBatch: UnlistenFn | null = null
  let refreshTimer: ReturnType<typeof setTimeout> | null = null
  let rowsObserver: ResizeObserver | null = null

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
  let extractActivity: { label: string; percent: number | null } | null = null
  let extractEventUnlisten: UnlistenFn | null = null

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

  const cleanupExtractListener = async () => {
    if (extractEventUnlisten) {
      await extractEventUnlisten()
      extractEventUnlisten = null
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

  $: rowsElStore.set(rowsElRef)
  $: headerElStore.set(headerElRef)

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

  const computeDirBytes = async (paths: string[], onProgress?: (bytes: number) => void) => {
    if (paths.length === 0) return 0
    const token = ++dirSizeAbort
    const progressEvent = `dir-size-progress-${token}-${Math.random().toString(16).slice(2)}`
    let unlisten: UnlistenFn | null = null
    const partials = new Map<string, number>()
    try {
      if (onProgress) {
        unlisten = await listen<{ path: string; bytes: number }>(progressEvent, (event) => {
          if (token !== dirSizeAbort) return
          const { path, bytes } = event.payload
          partials.set(path, bytes)
          const total = Array.from(partials.values()).reduce((sum, b) => sum + b, 0)
          onProgress(total)
        })
      }
      const result = await invoke<{ total: number }>('dir_sizes', { paths, progressEvent })
      if (token !== dirSizeAbort) return 0
      return result.total ?? 0
    } catch (err) {
      console.error('Failed to compute dir sizes', err)
      return 0
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
      try {
        if (currentView === 'trash') {
          for (const p of entries.map((e) => e.path)) {
            await invoke('delete_entry', { path: p })
          }
        } else {
          for (const p of entries.map((e) => e.path)) {
            await invoke('move_to_trash', { path: p })
          }
        }
        await reloadCurrent()
      } catch (err) {
        console.error('Failed to move to trash', err)
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
  })
  const { handleGlobalKeydown } = shortcuts

  const handleDocumentKeydown = (event: KeyboardEvent) => {
    const key = event.key.toLowerCase()
    const inRows = rowsElRef?.contains(event.target as Node) ?? false

    if ((event.ctrlKey || event.metaKey) && !isEditableTarget(event.target)) {
      if (event.shiftKey && key === 'i') {
        return
      }
      const allowed = new Set(['f', 'b', 'c', 'x', 'v', 'p', 'a'])
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
      if (rowsElRef && document.activeElement === rowsElRef) {
        rowsElRef.blur()
      }
    }
    void handleGlobalKeydown(event)
  }

  $: updateViewportHeight()
  $: {
    // Recompute virtualization when viewport or scroll changes.
    $viewportHeight
    $scrollTop
    recompute($filteredEntries)
  }

  const setupRowsObserver = () => {
    if (!rowsElRef || typeof ResizeObserver === 'undefined') return
    rowsObserver?.disconnect()
    rowsObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        if (entry.contentRect.height > 0) {
          updateViewportHeight()
        }
      }
    })
    rowsObserver.observe(rowsElRef)
  }

  $: {
    if (rowsElRef) {
      setupRowsObserver()
      updateViewportHeight()
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
      openContextMenu(entry, actions, event.clientX, event.clientY)
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
    extractActivity = { label: 'Extracting…', percent: 0 }
    try {
      extractEventUnlisten = await listen<ExtractProgressPayload>(progressEvent, (event) => {
        const payload = event.payload
        const pct =
          payload.total > 0 ? Math.min(100, Math.round((payload.bytes / payload.total) * 100)) : null
        if (payload.finished) {
          extractActivity = { label: 'Finalizing…', percent: pct ?? null }
        } else {
          extractActivity = { label: 'Extracting…', percent: pct }
        }
      })
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
      showToast(`Failed to extract: ${msg}`)
    } finally {
      extracting = false
      extractActivity = null
      await cleanupExtractListener()
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
    if (!rowsElRef) return
    const target = event.target as HTMLElement | null
    if (target && target.closest('.row')) return
    event.preventDefault()
    rowsElRef.focus()
    const list = get(filteredEntries)
    if (list.length === 0) return
    selectionDrag = false
    selectionBox.start(event, {
      rowsEl: rowsElRef,
      headerEl: headerElRef,
      entries: list,
      rowHeight,
      onSelect: (paths, anchor, caret) => {
        selected.set(paths)
        anchorIndex.set(anchor)
        caretIndex.set(caret)
      },
      onEnd: (didDrag) => {
        selectionDrag = didDrag
      },
    })
  }

  const handleRowsClickSafe = (event: MouseEvent) => {
    if (selectionDrag) {
      selectionDrag = false
      return
    }
    handleRowsClick(event)
  }

  const handleRowContextMenu = (entry: Entry, event: MouseEvent) => {
    const alreadySelected = $selected.has(entry.path)
    if (!alreadySelected) {
      const idx = get(filteredEntries).findIndex((e) => e.path === entry.path)
      selected.set(new Set([entry.path]))
      anchorIndex.set(idx >= 0 ? idx : null)
      caretIndex.set(idx >= 0 ? idx : null)
    }
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
    const actions: ContextAction[] = [{ id: 'new-folder', label: 'New Folder…' }]
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

  const closeOpenWith = () => {
    openWithOpen = false
    openWithEntry = null
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
    const lvl = Math.min(Math.max(Math.round(level), 0), 9)
    const paths = compressTargets.map((e) => e.path)
    const base = get(current)
    try {
      await invoke('compress_entries', { paths, name, level: lvl })
      await load(base, { recordHistory: false })
      closeCompress()
    } catch (err) {
      compressError = err instanceof Error ? err.message : String(err)
    }
  }

  const closeProperties = () => {
    propertiesOpen = false
    propertiesEntry = null
    propertiesSize = null
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
    if (deleteTargets.length === 0) return
    try {
      for (const target of deleteTargets) {
        await invoke('delete_entry', { path: target.path })
      }
      await reloadCurrent()
    } catch (err) {
      console.error('Failed to delete', err)
    } finally {
      closeDeleteConfirm()
    }
  }

  const openPropertiesModal = async (entries: Entry[]) => {
    const token = ++propertiesToken
    propertiesEntry = entries.length === 1 ? entries[0] : null
    propertiesCount = entries.length
    propertiesOpen = true
    const files = entries.filter((e) => e.kind === 'file')
    const dirs = entries.filter((e) => e.kind === 'dir')
    const fileBytes = files.reduce((sum, f) => sum + (f.size ?? 0), 0)
    propertiesSize = fileBytes
    let loadingDirs = dirs.length > 0

    if (dirs.length > 0) {
      const bytes = await computeDirBytes(
        dirs.map((d) => d.path),
        (partial) => {
          if (token === propertiesToken) {
            propertiesSize = fileBytes + partial
          }
        }
      )
      if (token === propertiesToken) {
        propertiesSize = fileBytes + bytes
        loadingDirs = false
      }
    } else {
      loadingDirs = false
    }

    if (entries.length === 1 && propertiesEntry) {
      void loadEntryTimes(propertiesEntry, token)
    }
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
            void load(latest, { recordHistory: false })
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
  activity={extractActivity}
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
    onRowsScroll={handleRowsScroll}
    onWheel={handleWheel}
    onRowsKeydown={rowsKeydownHandler}
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
  {openWithEntry}
  onCloseOpenWith={closeOpenWith}
    propertiesOpen={propertiesOpen}
    {propertiesEntry}
    {propertiesCount}
    {propertiesSize}
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
