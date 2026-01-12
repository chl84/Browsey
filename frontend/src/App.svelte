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
  import type { Entry, Partition, SortField } from './features/explorer/types'
  import { toast, showToast } from './features/explorer/hooks/useToast'
  import { createClipboard } from './features/explorer/hooks/useClipboard'
  import { createContextMenus } from './features/explorer/hooks/useContextMenus'
  import type { ContextAction } from './features/explorer/hooks/useContextMenus'
  import { createContextActions, type CurrentView } from './features/explorer/hooks/useContextActions'
  import { createSelectionBox } from './features/explorer/hooks/selectionBox'
  import './features/explorer/ExplorerLayout.css'

  let sidebarCollapsed = false
  let pathInput = ''
  let mode: 'address' | 'filter' | 'search' = 'address'
  let inputFocused = false
  let rowsElRef: HTMLDivElement | null = null
  let headerElRef: HTMLDivElement | null = null
  let pathInputEl: HTMLInputElement | null = null
  let unlistenDirChanged: UnlistenFn | null = null
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
  const { store: bookmarkStore } = bookmarkModal
  let bookmarkModalOpen = false
  let bookmarkName = ''
  let bookmarkCandidate: Entry | null = null
  let bookmarkInputEl: HTMLInputElement | null = null
  let renameModalOpen = false
  let renameTarget: Entry | null = null
  let renameValue = ''
  let openWithOpen = false
  let openWithEntry: Entry | null = null
  let propertiesOpen = false
  let propertiesEntry: Entry | null = null
  let propertiesCount = 1
  let propertiesSize: number | null = null
  let deleteConfirmOpen = false
  let deleteTargets: Entry[] = []
  let clipboardMode: 'copy' | 'cut' = 'copy'
  let clipboardPaths = new Set<string>()
  let currentView: CurrentView = 'dir'

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

  $: {
    if (mode === 'filter') {
      filter.set(pathInput)
      searchActive.set(pathInput.length > 0)
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
    try {
      if (onProgress) {
        unlisten = await listen<{ path: string; bytes: number }>(progressEvent, (event) => {
          if (token !== dirSizeAbort) return
          onProgress(event.payload.bytes)
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

  const isHidden = (entry: Entry) => entry.name.startsWith('.')

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

  const setSearchModeState = async (value: boolean) => {
    pathInput = ''
    await toggleMode(value)
    mode = value ? 'search' : 'address'
    if (!value) {
      searchActive.set(false)
      filter.set('')
      pathInput = $current
    }
  }

  const shortcuts = createGlobalShortcuts({
    isBookmarkModalOpen: () => get(bookmarkStore).open,
    searchMode: () => $searchMode,
    setSearchMode: async (value: boolean) => setSearchModeState(value),
    focusPath: () => focusPathInput(),
    blurPath: () => blurPathInput(),
    onTypeChar: async (char) => {
      if (inputFocused && mode === 'address') {
        return false
      }
      if ($searchMode || mode === 'search') {
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
      if (permanent) {
        deleteTargets = $filteredEntries.filter((e) => paths.includes(e.path))
        if (deleteTargets.length === 0) return false
        deleteConfirmOpen = true
        return true
      }
      try {
        if (currentView === 'trash') {
          for (const p of paths) {
            await invoke('delete_entry', { path: p })
          }
        } else {
          for (const p of paths) {
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
      })
      openContextMenu(entry, actions, event.clientX, event.clientY)
    } catch (err) {
      console.error('Failed to load context menu actions', err)
    }
  }

  const pasteIntoCurrent = async () => {
    const result = await clipboard.paste($current)
    if (!result.ok) {
      showToast(`Paste failed: ${result.error}`)
      return false
    }
    await reloadCurrent()
    return true
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
  })

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
    open(entry)
  }

  const handleRowsMouseDown = (event: MouseEvent) => {
    if (!rowsElRef) return
    if (event.target !== rowsElRef) return
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
    selected.set(new Set())
    anchorIndex.set(null)
    caretIndex.set(null)
    openBlankContextMenu(event.clientX, event.clientY)
  }

  const handleBlankContextAction = async (id: string) => {
    closeBlankContextMenu()
    if (id === 'paste') {
      await pasteIntoCurrent()
    }
  }

  const handleContextSelect = async (id: string) => {
    const entry = $contextMenu.entry
    closeContextMenu()
    await contextActions(id, entry)
  }

  const closeRenameModal = () => {
    renameModalOpen = false
    renameTarget = null
  }

  const confirmRename = async (name: string) => {
    if (!renameTarget) return
    const trimmed = name.trim()
    if (!trimmed) return
    try {
      await invoke('rename_entry', { path: renameTarget.path, newName: trimmed })
      await load(parentPath(renameTarget.path), { recordHistory: false })
    } catch (err) {
      console.error('Failed to rename', err)
    } finally {
      closeRenameModal()
    }
  }

  const closeOpenWith = () => {
    openWithOpen = false
    openWithEntry = null
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
        if (!curr || event.payload === curr) {
          if (refreshTimer) {
            clearTimeout(refreshTimer)
          }
          refreshTimer = setTimeout(() => {
            void load(curr, { recordHistory: false })
          }, 300)
        }
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
  onFocus={handleInputFocus}
  onBlur={handleInputBlur}
  onSubmitPath={submitPath}
  onSearch={() => runSearch(pathInput)}
  onExitSearch={() => void setSearchModeState(false).then(() => blurPathInput())}
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
  bind:renameValue
  onConfirmRename={confirmRename}
  onCancelRename={closeRenameModal}
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
