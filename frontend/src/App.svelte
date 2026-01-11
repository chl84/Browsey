<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { get } from 'svelte/store'
  import { formatItems, formatSelectionLine, formatSize, normalizePath } from './lib/explorer/utils'
  import { createListState } from './lib/explorer/stores/listState'
  import Sidebar from './lib/components/explorer/Sidebar.svelte'
  import Topbar from './lib/components/explorer/Topbar.svelte'
  import BookmarkModal from './lib/components/explorer/BookmarkModal.svelte'
  import FileList from './lib/components/explorer/FileList.svelte'
  import Statusbar from './lib/components/explorer/Statusbar.svelte'
  import Notice from './lib/components/explorer/Notice.svelte'
  import { createExplorerState } from './lib/explorer/state'
  import { createColumnResize } from './lib/explorer/hooks/columnWidths'
  import { createGlobalShortcuts } from './lib/explorer/hooks/shortcuts'
  import { createBookmarkModal } from './lib/explorer/hooks/bookmarkModal'
  import type { Entry, Partition, SortField } from './lib/explorer/types'
  import './lib/explorer/ExplorerLayout.css'

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

  const {
    selected,
    anchorIndex,
    caretIndex,
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

  $: {
    if (mode === 'filter') {
      filter.set(pathInput)
      searchActive.set(pathInput.length > 0)
    }
  }

  const { startResize } = createColumnResize(cols, persistWidths)

  let dirSizeAbort = 0
  let selectedDirBytes = 0
  let selectionText = ''

  const refreshSelectionSizes = async () => {
    const selectedPaths = Array.from($selected)
    const dirs = $entries.filter((e) => $selected.has(e.path) && e.kind === 'dir').map((d) => d.path)
    if (dirs.length === 0) {
      selectedDirBytes = 0
      dirSizeAbort++
      return
    }
    const token = ++dirSizeAbort
    try {
      const result = await invoke<{ total: number }>('dir_sizes', { paths: dirs })
      if (token !== dirSizeAbort) return
      selectedDirBytes = result.total ?? 0
    } catch (err) {
      console.error('Failed to compute dir sizes', err)
      if (token === dirSizeAbort) {
        selectedDirBytes = 0
      }
    }
  }

  $: {
    const selectedEntries = $entries.filter((e) => $selected.has(e.path))
    const files = selectedEntries.filter((e) => e.kind === 'file')
    const dirs = selectedEntries.filter((e) => e.kind === 'dir')
    const fileBytes = files.reduce((sum, f) => sum + (f.size ?? 0), 0)

    if (dirs.length > 0) {
      void refreshSelectionSizes()
    } else {
      selectedDirBytes = 0
    }

    const dirLine = formatSelectionLine(dirs.length, 'folder', selectedDirBytes)
    const fileLine = formatSelectionLine(files.length, 'file', fileBytes)

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
      if (mode === 'address' || (inputFocused && mode === 'address')) {
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
    })
  const { handleGlobalKeydown } = shortcuts

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

<svelte:document on:keydown|capture={handleGlobalKeydown} />
<main class="shell">
  <div class="layout" class:collapsed={sidebarCollapsed}>
    <Sidebar
      places={places}
      bookmarks={bookmarks}
      partitions={partitions}
      collapsed={sidebarCollapsed}
      onPlaceSelect={handlePlace}
      onBookmarkSelect={(path) => void load(path)}
      onRemoveBookmark={(path) => {
        void invoke('remove_bookmark', { path })
        bookmarksStore.update((list) => list.filter((b) => b.path !== path))
      }}
      onPartitionSelect={(path) => void load(path)}
    />

    <section class="content">
      <Topbar
        bind:pathInput
        bind:pathInputEl
        searchMode={$searchMode}
        loading={$loading}
        onFocus={handleInputFocus}
        onBlur={handleInputBlur}
        onSubmitPath={submitPath}
        onSearch={() => runSearch(pathInput)}
        onExitSearch={() => void setSearchModeState(false).then(() => blurPathInput())}
      />

      <Notice message={$error} />

      {#if $searchActive}
        <div class="pill">{mode === 'filter' ? 'Filtering' : 'Searching'}: "{$filter}"</div>
      {/if}

      <FileList
        cols={$cols}
        gridTemplate={$gridTemplate}
        bind:rowsEl={rowsElRef}
        bind:headerEl={headerElRef}
        loading={$loading}
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
        onRowsScroll={handleRowsScroll}
        onWheel={handleWheel}
        onRowsKeydown={rowsKeydownHandler}
        onRowsClick={handleRowsClick}
        onChangeSort={changeSort}
        onStartResize={startResize}
        ariaSort={ariaSort}
        onRowClick={rowClickHandler}
        onOpen={handleOpenEntry}
        onToggleStar={toggleStar}
      />
      <Statusbar {selectionText} />
    </section>
  </div>
</main>

{#if bookmarkModalOpen}
  <BookmarkModal
    open={bookmarkModalOpen}
    entryName={bookmarkCandidate?.name ?? ''}
    bind:bookmarkName
    bind:bookmarkInputEl
    onConfirm={confirmBookmark}
    onCancel={closeBookmarkModal}
  />
{/if}
