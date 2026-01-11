<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { clampIndex, clearSelection, selectAllPaths, selectRange } from './selection'

  type Entry = {
    name: string
    path: string
    kind: 'dir' | 'file' | 'link'
    ext?: string | null
    size?: number | null
    items?: number | null
    modified?: string | null
    starred?: boolean
    icon?: string
  }

  type Listing = {
    current: string
    entries: Entry[]
  }

  type SortField = 'name' | 'type' | 'modified' | 'size' | 'starred'
  type SortDirection = 'asc' | 'desc'

  type Location =
    | { type: 'dir'; path: string }
    | { type: 'recent' }
    | { type: 'starred' }
    | { type: 'trash' }

  let current = ''
  let entries: Entry[] = []
  let loading = false
  let error = ''
  let filter = ''
  let searchMode = false
  let searchActive = false
  let sidebarCollapsed = false
  let pathInput = ''
  let sortField: SortField = 'name'
  let sortDirection: SortDirection = 'asc'
  let selected = clearSelection()
  let anchorIndex: number | null = null
  let caretIndex: number | null = null
  type Column = {
    key: string
    label: string
    sort: SortField
    width: number
    min: number
    align?: 'left' | 'right' | 'center'
    resizable?: boolean
    sortable?: boolean
  }

  let cols: Column[] = [
    { key: 'name', label: 'Name', sort: 'name', width: 320, min: 220, align: 'left' },
    { key: 'type', label: 'Type', sort: 'type', width: 120, min: 80 },
    { key: 'modified', label: 'Modified', sort: 'modified', width: 90, min: 80 },
    { key: 'size', label: 'Size', sort: 'size', width: 90, min: 70, align: 'right' },
    { key: 'star', label: '', sort: 'starred', width: 80, min: 60, resizable: false, sortable: false },
  ]

  let gridTemplate = cols.map((c) => `${Math.max(c.width, c.min)}px`).join(' ')
  let resizeState: { index: number; startX: number; startWidth: number } | null = null
  let history: Location[] = []
  let historyIndex = -1
  let rowsEl: HTMLDivElement | null = null
  let headerEl: HTMLDivElement | null = null
  let pathInputEl: HTMLInputElement | null = null
  let viewportHeight = 0
  let scrollTop = 0
  let pendingDeltaY = 0
  let wheelRaf: number | null = null
  const rowHeight = 32
  const overscan = 8
  const wheelScale = 0.7
  let unlistenDirChanged: UnlistenFn | null = null
  let refreshTimer: ReturnType<typeof setTimeout> | null = null
  let rowsObserver: ResizeObserver | null = null
  const headerHeight = () => headerEl?.offsetHeight ?? 0

  const updateViewportHeight = () => {
    const containerHeight = rowsEl?.clientHeight ?? 0
    const next = Math.max(0, containerHeight - headerHeight())
    if (next !== viewportHeight) {
      viewportHeight = next
    }
  }

  const places = [
    { label: 'Home', path: '~' },
    { label: 'Recent', path: '' },
    { label: 'Starred', path: '' },
    { label: 'Network', path: '' },
    { label: 'Wastebasket', path: 'trash://' },
  ]

  let bookmarks = [
  ]

  type Partition = {
    label: string
    path: string
    fs?: string
    removable?: boolean
  }

  let partitions: Partition[] = []
  let partitionsPoll: ReturnType<typeof setInterval> | null = null
  let lastMountPaths: string[] = []
  let bookmarkModalOpen = false
  let bookmarkName = ''
  let bookmarkCandidate: Entry | null = null
  let bookmarkInputEl: HTMLInputElement | null = null

  const iconPath = (file: string) => `/icons/scalable/${file}`
  const navIcon = (label: string) => {
    switch (label) {
      case 'Home':
        return iconPath('devices/computer.svg')
      case 'Recent':
        return iconPath('places/folder-documents.svg')
      case 'Starred':
        return iconPath('places/user-starred.svg')
      case 'Network':
        return iconPath('places/folder-remote.svg')
      case 'Wastebasket':
        return iconPath('status/user-trash-full.svg')
      default:
        return iconPath('devices/drive-harddisk.svg')
    }
  }

  const partitionIcon = (part: Partition) =>
    part.removable ? iconPath('devices/drive-removable-media.svg') : iconPath('devices/drive-harddisk.svg')

  const normalizePath = (p: string) => {
    if (!p) return ''
    const trimmed = p.replace(/\/+$/, '')
    return trimmed === '' ? '/' : trimmed
  }

  const isUnderMount = (path: string, mount: string) => {
    if (!path || !mount) return false
    const p = normalizePath(path)
    const m = normalizePath(mount)
    return p === m || p.startsWith(`${m}/`)
  }

  const parentPath = (path: string) => {
    if (!path || path === '/') return '/'
    const trimmed = path.replace(/\/+$/, '')
    const idx = trimmed.lastIndexOf('/')
    if (idx <= 0) return '/'
    return trimmed.slice(0, idx)
  }

  const formatSize = (size?: number | null) => {
    if (size === null || size === undefined) return ''
    if (size < 1024) return `${size} B`
    const units = ['KB', 'MB', 'GB', 'TB']
    let value = size / 1024
    let u = 0
    while (value >= 1024 && u < units.length - 1) {
      value /= 1024
      u++
    }
    return `${value.toFixed(1)} ${units[u]}`
  }

  const formatItems = (count?: number | null) => {
    if (count === null || count === undefined) return ''
    const suffix = count === 1 ? 'item' : 'items'
    return `${count} ${suffix}`
  }

  const formatSelectionLine = (count: number, noun: string, bytes?: number) => {
    if (count === 0) return ''
    const sizePart = bytes && bytes > 0 ? ` (${formatSize(bytes)})` : ''
    const suffix = count === 1 ? noun : `${noun}s`
    return `${count} ${suffix} selected${sizePart}`
  }

  const sortPayload = () => ({
    field: sortField,
    direction: sortDirection,
  })

  $: gridTemplate = cols.map((c) => `${Math.max(c.width, c.min)}px`).join(' ')

  const startResize = (index: number, event: PointerEvent) => {
    event.preventDefault()
    event.stopPropagation()
    resizeState = {
      index,
      startX: event.clientX,
      startWidth: cols[index].width,
    }
    window.addEventListener('pointermove', handleResizeMove)
    window.addEventListener('pointerup', handleResizeEnd, { once: true })
  }

  const handleResizeMove = (event: PointerEvent) => {
    if (!resizeState) return
    const delta = event.clientX - resizeState.startX
    cols = cols.map((c, i) =>
      i === resizeState!.index
        ? { ...c, width: Math.max(c.min, resizeState!.startWidth + delta) }
        : c
    )
  }

  const handleResizeEnd = () => {
    resizeState = null
    window.removeEventListener('pointermove', handleResizeMove)
    void persistWidths()
  }

  let dirSizeAbort = 0
  let selectedDirBytes = 0
  let selectionText = ''

  const refreshSelectionSizes = async () => {
    const selectedPaths = Array.from(selected)
    const dirs = entries.filter((e) => selected.has(e.path) && e.kind === 'dir').map((d) => d.path)
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
    const selectedEntries = entries.filter((e) => selected.has(e.path))
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

  const persistWidths = async () => {
    try {
      await invoke('store_column_widths', { widths: cols.map((c) => c.width) })
    } catch (err) {
      console.error('Failed to store widths', err)
    }
  }

  const loadSavedWidths = async () => {
    try {
      const saved = await invoke<number[] | null>('load_saved_column_widths')
      if (saved && Array.isArray(saved)) {
        cols = cols.map((c, i) =>
          saved[i] !== undefined ? { ...c, width: Math.max(c.min, saved[i]) } : c
        )
      }
    } catch (err) {
      console.error('Failed to load widths', err)
    }
  }

  const ariaSort = (field: SortField) =>
    sortField === field ? (sortDirection === 'asc' ? 'ascending' : 'descending') : 'none'

  const changeSort = (field: SortField) => {
    if (sortField === field) {
      sortDirection = sortDirection === 'asc' ? 'desc' : 'asc'
    } else {
      sortField = field
      sortDirection = 'asc'
    }
    void refreshForSort()
  }

  const sameLocation = (a?: Location, b?: Location) => {
    if (!a || !b) return false
    if (a.type !== b.type) return false
    if (a.type === 'dir' && b.type === 'dir') {
      return a.path === b.path
    }
    return true
  }

  const pushHistory = (loc: Location) => {
    const last = history[historyIndex]
    if (sameLocation(last, loc)) return
    history = history.slice(0, historyIndex + 1)
    history.push(loc)
    historyIndex = history.length - 1
  }

  const load = async (path?: string, opts: { recordHistory?: boolean } = {}) => {
    const { recordHistory = true } = opts
    loading = true
    error = ''
    searchActive = false
    try {
      const result = await invoke<Listing>('list_dir', { path, sort: sortPayload() })
      current = result.current
      pathInput = result.current
      entries = result.entries
      resetScrollPosition()
      if (recordHistory) {
        pushHistory({ type: 'dir', path: current })
      }
      // Start watching this directory for changes.
      await invoke('watch_dir', { path: current })
    } catch (err) {
      error = err instanceof Error ? err.message : String(err)
    } finally {
      loading = false
    }
  }

  const loadRecent = async (recordHistory = true) => {
    loading = true
    error = ''
    searchActive = false
    try {
      const result = await invoke<Entry[]>('list_recent', { sort: sortPayload() })
      current = 'Recent'
      pathInput = ''
      entries = result
      resetScrollPosition()
      if (recordHistory) {
        pushHistory({ type: 'recent' })
      }
    } catch (err) {
      error = err instanceof Error ? err.message : String(err)
    } finally {
      loading = false
    }
  }

  const loadStarred = async (recordHistory = true) => {
    loading = true
    error = ''
    searchActive = false
    try {
      const result = await invoke<Entry[]>('list_starred', { sort: sortPayload() })
      current = 'Starred'
      pathInput = ''
      entries = result
      resetScrollPosition()
      if (recordHistory) {
        pushHistory({ type: 'starred' })
      }
    } catch (err) {
      error = err instanceof Error ? err.message : String(err)
    } finally {
      loading = false
    }
  }

  const loadTrash = async (recordHistory = true) => {
    loading = true
    error = ''
    searchActive = false
    try {
      const result = await invoke<Listing>('list_trash', { sort: sortPayload() })
      current = 'Trash'
      pathInput = ''
      entries = result.entries
      resetScrollPosition()
      if (recordHistory) {
        pushHistory({ type: 'trash' })
      }
    } catch (err) {
      error = err instanceof Error ? err.message : String(err)
    } finally {
      loading = false
    }
  }

  const open = (entry: Entry) => {
    if (entry.kind === 'dir') {
      void load(entry.path)
    } else {
      void invoke('open_entry', { path: entry.path })
    }
  }

  const toggleStar = async (entry: Entry) => {
    try {
      const newState = await invoke<boolean>('toggle_star', { path: entry.path })
      if (current === 'Starred' && !newState) {
        entries = entries.filter((e) => e.path !== entry.path)
      } else {
        entries = entries.map((e) =>
          e.path === entry.path ? { ...e, starred: newState } : e
        )
      }
    } catch (err) {
      error = err instanceof Error ? err.message : String(err)
    }
  }

  const goUp = () => {
    searchActive = false
    void load(parentPath(current))
  }

  const goHome = () => {
    searchActive = false
    void load(undefined)
  }

  const goToPath = () => {
    if (!pathInput.trim()) return
    if (pathInput.trim() !== current) {
      void load(pathInput.trim())
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

  const isHidden = (entry: Entry) => entry.name.startsWith('.')

  const displayName = (entry: Entry) => {
    if (entry.kind === 'file' && entry.ext) {
      return entry.name.replace(new RegExp(`\\.${entry.ext}$`), '')
    }
    return entry.name
  }

  const runSearch = async () => {
    loading = true
    error = ''
    try {
      const needle = pathInput.trim()
      if (needle.length === 0) {
        searchActive = false
        await load(current, { recordHistory: false })
      } else {
        filter = needle
        const result = await invoke<Entry[]>('search', { path: current, query: needle, sort: sortPayload() })
        entries = result
        resetScrollPosition()
        searchActive = true
      }
    } catch (err) {
      error = err instanceof Error ? err.message : String(err)
    } finally {
      loading = false
    }
  }

  const toggleMode = async (checked: boolean) => {
    searchMode = checked
    if (!searchMode) {
      filter = ''
      searchActive = false
      pathInput = current
      await load(current, { recordHistory: false })
    }
  }

  $: filteredEntries =
    filter.trim().length === 0
      ? entries
      : entries.filter((e) => e.name.toLowerCase().includes(filter.trim().toLowerCase()))

  const handleResize = () => {
    if (typeof window === 'undefined') return
    sidebarCollapsed = window.innerWidth < 700
    if (rowsEl) updateViewportHeight()
  }

  const handleRowsScroll = () => {
    if (!rowsEl) return
    const effectiveTop = Math.max(0, rowsEl.scrollTop - headerHeight())
    scrollTop = effectiveTop
  }

  const handleWheel = (event: WheelEvent) => {
    if (!rowsEl) return
    pendingDeltaY += event.deltaY * wheelScale
    if (wheelRaf !== null) return
    wheelRaf = requestAnimationFrame(() => {
      rowsEl!.scrollTop += pendingDeltaY
      pendingDeltaY = 0
      wheelRaf = null
    })
  }

  const handleRowsKeydown = (event: KeyboardEvent) => {
    const key = event.key.toLowerCase()
    if (event.ctrlKey && key === 'a') {
      event.preventDefault()
      event.stopPropagation()
      selected = selectAllPaths(filteredEntries)
      anchorIndex = 0
      caretIndex = filteredEntries.length > 0 ? filteredEntries.length - 1 : null
    } else if (key === 'escape') {
      event.preventDefault()
      event.stopPropagation()
      selected = clearSelection()
      anchorIndex = null
      caretIndex = null
    } else if ((key === 'arrowdown' || key === 'arrowup') && filteredEntries.length > 0) {
      event.preventDefault()
      event.stopPropagation()
      const delta = key === 'arrowdown' ? 1 : -1
      // If nothing selected, start at first/last depending on direction.
      const current = caretIndex ?? anchorIndex ?? (delta > 0 ? 0 : filteredEntries.length - 1)
      const next = clampIndex(current + delta, filteredEntries)

      if (event.shiftKey) {
        const anchor = anchorIndex ?? current
        const rangeSet = selectRange(filteredEntries, anchor, next)
        selected = rangeSet
        anchorIndex = anchor
        caretIndex = next
      } else {
        selected = new Set([filteredEntries[next].path])
        anchorIndex = next
        caretIndex = next
      }
      if (caretIndex !== null) {
        ensureRowVisible(caretIndex)
      }
    }
  }

  const handleRowsClick = (event: MouseEvent) => {
    // Clear selection only when clicking empty space in the rows container.
    if (event.target === rowsEl && selected.size > 0) {
      selected = clearSelection()
      anchorIndex = null
      caretIndex = null
    }
  }

  const refreshForSort = async () => {
    if (searchActive && searchMode) {
      await runSearch()
      return
    }

    if (current === 'Recent') {
      await loadRecent(false)
    } else if (current === 'Starred') {
      await loadStarred(false)
    } else if (current === 'Trash') {
      await loadTrash(false)
    } else {
      await load(current, { recordHistory: false })
    }
  }

  const handleRowClick = (entry: Entry, absoluteIndex: number, event: MouseEvent) => {
    event.stopPropagation()
    const isToggle = event.ctrlKey || event.metaKey
    const isRange = event.shiftKey && anchorIndex !== null

    if (isRange && anchorIndex !== null) {
      const rangeSet = selectRange(filteredEntries, anchorIndex, absoluteIndex)
      if (isToggle) {
        const merged = new Set(selected)
        rangeSet.forEach((p) => merged.add(p))
        selected = merged
      } else {
        selected = rangeSet
      }
      caretIndex = absoluteIndex
    } else if (isToggle) {
      const next = new Set(selected)
      if (next.has(entry.path)) {
        next.delete(entry.path)
      } else {
        next.add(entry.path)
      }
      selected = next
      anchorIndex = absoluteIndex
      caretIndex = absoluteIndex
    } else {
      selected = new Set([entry.path])
      anchorIndex = absoluteIndex
      caretIndex = absoluteIndex
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
    if (historyIndex <= 0) return
    historyIndex -= 1
    await navigateTo(history[historyIndex], false)
  }

  const goForward = async () => {
    if (historyIndex < 0 || historyIndex >= history.length - 1) return
    historyIndex += 1
    await navigateTo(history[historyIndex], false)
  }

  const isEditableTarget = (target: EventTarget | null) => {
    if (!(target instanceof HTMLElement)) return false
    const tag = target.tagName.toLowerCase()
    return (
      target.isContentEditable ||
      tag === 'input' ||
      tag === 'textarea' ||
      tag === 'select'
    )
  }

  const focusPathInput = () => {
    if (pathInputEl) {
      pathInputEl.focus()
      pathInputEl.select()
    }
  }

  const handleGlobalKeydown = async (event: KeyboardEvent) => {
    const key = event.key.toLowerCase()
    if (bookmarkModalOpen) return
    if ((event.ctrlKey || event.metaKey) && key === 'f') {
      event.preventDefault()
      event.stopPropagation()
      if (!searchMode) {
        pathInput = ''
        await toggleMode(true)
      }
      focusPathInput()
      return
    }

    if ((event.ctrlKey || event.metaKey) && key === 'b') {
      if (isEditableTarget(event.target)) return
      event.preventDefault()
      event.stopPropagation()
      const selectedPaths = Array.from(selected)
      if (selectedPaths.length === 1) {
        const entry = entries.find((e) => e.path === selectedPaths[0])
        if (entry && entry.kind === 'dir') {
          await openBookmarkModal(entry)
        }
      }
      return
    }

    if (event.key !== 'Backspace') return
    if (event.ctrlKey || event.metaKey || event.altKey) return
    if (isEditableTarget(event.target)) return

    event.preventDefault()
    event.stopPropagation()
    if (event.shiftKey) {
      void goForward()
    } else {
      void goBack()
    }
  }

  $: updateViewportHeight()

  $: totalHeight = filteredEntries.length * rowHeight
  $: visibleCount =
    Math.ceil((viewportHeight || 0) / rowHeight) + overscan * 2
  $: start = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan)
  $: end = Math.min(filteredEntries.length, start + visibleCount)
  $: visibleEntries = filteredEntries.slice(start, end)
  $: offsetY = start * rowHeight

  const setupRowsObserver = () => {
    if (!rowsEl || typeof ResizeObserver === 'undefined') return
    rowsObserver?.disconnect()
    rowsObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const h = entry.contentRect.height
        if (h > 0) {
          updateViewportHeight()
        }
      }
    })
    rowsObserver.observe(rowsEl)
  }

  $: {
    if (rowsEl) {
      setupRowsObserver()
      updateViewportHeight()
    }
  }

  const resetScrollPosition = () => {
    scrollTop = 0
    if (rowsEl) {
      rowsEl.scrollTo({ top: 0 })
    }
  }

  const ensureRowVisible = (index: number) => {
    if (!rowsEl) return
    const headerOffset = headerHeight()
    const viewport = viewportHeight
    const currentTop = scrollTop
    const currentBottom = currentTop + viewport
    const rowTop = index * rowHeight
    const rowBottom = rowTop + rowHeight
    let nextScroll: number | null = null

    if (rowTop < currentTop) {
      nextScroll = headerOffset + rowTop
    } else if (rowBottom > currentBottom) {
      nextScroll = headerOffset + rowBottom - viewport
    }

    if (nextScroll !== null) {
      rowsEl.scrollTo({ top: nextScroll })
    }
  }

  const openBookmarkModal = async (entry: Entry) => {
    bookmarkCandidate = entry
    bookmarkName = entry.name
    bookmarkModalOpen = true
    await tick()
    if (bookmarkInputEl) {
      bookmarkInputEl.focus()
      bookmarkInputEl.select()
    }
  }

  const closeBookmarkModal = () => {
    bookmarkModalOpen = false
    bookmarkCandidate = null
    bookmarkName = ''
  }

  const confirmBookmark = () => {
    if (!bookmarkCandidate) return
    const label = bookmarkName.trim() || bookmarkCandidate.name
    const path = bookmarkCandidate.path
    // Avoid duplicate paths
    if (!bookmarks.some((b) => normalizePath(b.path) === normalizePath(path))) {
      void invoke('add_bookmark', { label, path })
      bookmarks = [...bookmarks, { label, path }]
    }
    closeBookmarkModal()
  }

  const loadPartitions = async () => {
    try {
      const result = await invoke<Partition[]>('list_mounts')
      partitions = result
      const nextPaths = result.map((p) => normalizePath(p.path))
      const removedMount = lastMountPaths.find((p) => !nextPaths.includes(p))
      lastMountPaths = nextPaths

      if (removedMount && isUnderMount(current, removedMount)) {
        error = 'Volume disconnected; returning to Home'
        void load(undefined)
      }
    } catch (err) {
      console.error('Failed to load mounts', err)
    }
  }

  const loadBookmarks = async () => {
    try {
      const rows = await invoke<{ label: string; path: string }[]>('get_bookmarks')
      bookmarks = rows
    } catch (err) {
      console.error('Failed to load bookmarks', err)
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
    window.addEventListener('keydown', handleGlobalKeydown)
    void load()

    // Listen for backend watcher events to refresh the current directory.
    void (async () => {
      unlistenDirChanged = await listen<string>('dir-changed', (event) => {
        if (!current || event.payload === current) {
          if (refreshTimer) {
            clearTimeout(refreshTimer)
          }
          refreshTimer = setTimeout(() => {
            void load(current, { recordHistory: false })
          }, 300)
        }
      })
    })()
    return () => {
      window.removeEventListener('resize', handleResize)
      window.removeEventListener('keydown', handleGlobalKeydown)
      if (refreshTimer) {
        clearTimeout(refreshTimer)
        refreshTimer = null
      }
      if (wheelRaf !== null) {
        cancelAnimationFrame(wheelRaf)
        wheelRaf = null
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

<main class="shell">
  <div class="layout" class:collapsed={sidebarCollapsed}>
    <aside class:collapsed={sidebarCollapsed} class="sidebar">
          <div class="section">
            <div class="section-title">Places</div>
            {#each places as place}
              <button class="nav" type="button" on:click={() => handlePlace(place.label, place.path)}>
                <img class="nav-icon" src={navIcon(place.label)} alt="" />
                <span class="nav-label">{place.label}</span>
              </button>
            {/each}
          </div>

        <div class="section">
          <div class="section-title">Bookmarks</div>
          {#each bookmarks as mark}
            <div class="nav bookmark" role="button" tabindex="0" on:click={() => load(mark.path)} on:keydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                load(mark.path)
              }
            }}>
              <img class="nav-icon" src={navIcon(mark.label)} alt="" />
              <span class="nav-label">{mark.label}</span>
              <span
                class="remove-bookmark"
                role="button"
                tabindex="0"
                aria-label="Remove bookmark"
                on:click={(e) => {
                  e.stopPropagation()
                  void invoke('remove_bookmark', { path: mark.path })
                  bookmarks = bookmarks.filter((b) => b.path !== mark.path)
                }}
                on:keydown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault()
                    e.stopPropagation()
                    void invoke('remove_bookmark', { path: mark.path })
                    bookmarks = bookmarks.filter((b) => b.path !== mark.path)
                  }
                }}
              >
                ×
              </span>
            </div>
          {/each}
        </div>

          <div class="section">
            <div class="section-title">Partitions</div>
            {#each partitions as part}
              <button class="nav" type="button" on:click={() => load(part.path)}>
                <img class="nav-icon" src={partitionIcon(part)} alt="" />
                <span class="nav-label">{part.label}</span>
              </button>
            {/each}
          </div>

    </aside>

    <section class="content">
      <header class="topbar">
        <div class="left">
          <div class="path">
            <input
              class="path-input"
              bind:value={pathInput}
              bind:this={pathInputEl}
              placeholder={searchMode ? 'Search in current folder…' : 'Path…'}
              aria-label={searchMode ? 'Search' : 'Path'}
              on:keydown={(e) => {
                if (e.key === 'Escape' && searchMode) {
                  e.preventDefault()
                  e.stopPropagation()
                  void toggleMode(false).then(() => focusPathInput())
                  return
                }
                if (e.key === 'Enter' && !searchMode) {
                  goToPath()
                } else if (e.key === 'Enter' && searchMode) {
                  runSearch()
                }
              }}
            />
          </div>
          {#if loading}
            <span class="pill">Loading…</span>
          {/if}
        </div>
      </header>

      {#if error}
        <div class="error">Error: {error}</div>
      {/if}

      {#if searchActive}
        <div class="pill">Searching: "{filter}"</div>
      {/if}

      <section class="list" class:wide={sidebarCollapsed}>
        <div
          class="rows"
          bind:this={rowsEl}
          on:scroll={handleRowsScroll}
          on:wheel|passive={handleWheel}
          on:keydown={handleRowsKeydown}
          on:click={handleRowsClick}
          tabindex="0"
          role="grid"
          aria-label="File list"
        >
          <div class="header-row" bind:this={headerEl} style={`grid-template-columns:${gridTemplate};`}>
            {#each cols as col, idx}
              <div class="header-cell">
                {#if col.sortable === false}
                  <div
                    class="header-btn inert"
                    class:align-right={col.align === 'right'}
                    role="columnheader"
                    aria-sort="none"
                  >
                    {#if col.label}<span>{col.label}</span>{/if}
                  </div>
                {:else}
                  <button
                    class="header-btn"
                    class:align-right={col.align === 'right'}
                    type="button"
                    role="columnheader"
                    aria-sort={ariaSort(col.sort)}
                    class:active-sort={sortField === col.sort}
                    on:click={() => changeSort(col.sort)}
                  >
                    <span>{col.label}</span>
                    <span
                      class="sort-icon"
                      class:desc={sortField === col.sort && sortDirection === 'desc'}
                      class:inactive={sortField !== col.sort}
                    >
                      ▲
                    </span>
                  </button>
                {/if}
                {#if col.resizable !== false && idx < cols.length - 1}
                  <span class="resizer" on:pointerdown={(e) => startResize(idx, e)}></span>
                {/if}
              </div>
            {/each}
          </div>
          {#if !loading && filteredEntries.length === 0}
            <div class="muted">No items here.</div>
          {:else}
            <div class="spacer" style={`height:${totalHeight}px`}>
              <div class="row-viewport" style={`transform: translateY(${offsetY}px)`}>
                {#each visibleEntries as entry, i (entry.path)}
                  <button
                    class="row"
                    style={`grid-template-columns:${gridTemplate};`}
                    class:hidden={isHidden(entry)}
                    class:selected={selected.has(entry.path)}
                    type="button"
                    on:dblclick={() => open(entry)}
                    on:keydown={(e) => {
                      if (e.key === 'Enter') {
                        e.preventDefault()
                        e.stopPropagation()
                        open(entry)
                      }
                    }}
                    on:click={(e) => handleRowClick(entry, start + i, e)}
                  >
                    <div class="col-name">
                      <img class="icon" src={entry.icon} alt="" />
                      <span class="name">{displayName(entry)}</span>
                    </div>
                    <div class="col-type">
                      {entry.kind === 'dir'
                        ? 'Folder'
                        : entry.kind === 'link'
                          ? 'Link'
                          : entry.ext && entry.ext.length > 0
                            ? `.${entry.ext}`
                            : 'File'}
                    </div>
                    <div class="col-modified">{entry.modified ?? '—'}</div>
                    <div class="col-size">
                      {entry.kind === 'file'
                        ? formatSize(entry.size)
                        : entry.kind === 'dir'
                          ? formatItems(entry.items)
                          : ''}
                    </div>
                    <div class="col-star">
                      <span
                        class="star-btn"
                        class:starred={entry.starred}
                        role="button"
                        tabindex="0"
                        aria-label={entry.starred ? 'Unstar' : 'Star'}
                        on:click={(e) => {
                          e.stopPropagation()
                          toggleStar(entry)
                        }}
                        on:keydown={(e) => {
                          if (e.key === 'Enter' || e.key === ' ') {
                            e.preventDefault()
                            e.stopPropagation()
                            toggleStar(entry)
                          }
                        }}
                      >
                        {entry.starred ? '★' : '☆'}
                      </span>
                    </div>
                  </button>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      </section>
      <footer class="statusbar">
        {#if selectionText}
          <span class="status-text">{selectionText}</span>
        {/if}
      </footer>
    </section>
  </div>
</main>

{#if bookmarkModalOpen}
  <div class="modal-backdrop" role="dialog" aria-modal="true">
    <div class="modal">
      <h2 class="modal-title">Add bookmark</h2>
      <p class="modal-desc">Name the bookmark for "{bookmarkCandidate?.name}".</p>
      <input
        class="modal-input"
        bind:value={bookmarkName}
        bind:this={bookmarkInputEl}
        aria-label="Bookmark name"
        on:keydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            confirmBookmark()
          } else if (e.key === 'Escape') {
            e.preventDefault()
            closeBookmarkModal()
          }
        }}
      />
      <div class="modal-actions">
        <button type="button" class="secondary" on:click={closeBookmarkModal}>Cancel</button>
        <button type="button" on:click={confirmBookmark}>Add</button>
      </div>
    </div>
  </div>
{/if}

<style>
.shell {
  width: 100%;
  max-width: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 16px;
  color: var(--fg);
  min-height: 100%;
  height: 100vh;
  overflow: hidden;
}

  .topbar {
    display: flex;
    gap: 12px;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    position: sticky;
    top: 0;
    z-index: 2;
    background: var(--bg);
    padding: 0;
  }

  .layout {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 0;
    align-items: stretch;
    min-height: 0;
    height: 100%;
  }

  .layout.collapsed {
    grid-template-columns: 1fr;
  }

  .sidebar {
    background: var(--bg-alt);
    border: 1px solid var(--border-strong);
    border-radius: 0;
    padding: 5px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    overflow: auto;
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.35);
    scrollbar-width: none; /* hide scrollbar */
    -ms-overflow-style: none; /* IE/Edge */
  }

  .sidebar::-webkit-scrollbar {
    display: none;
  }

  .sidebar.collapsed {
    display: none;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .section-title {
    color: var(--fg-muted);
    font-size: 12px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-weight: 700;
    padding-left: 10px;
  }

  .nav {
    border: none;
    border-radius: 10px;
    padding: 5px 12px 5px 22px;
    background: transparent;
    color: var(--fg);
    font-size: 14px;
    font-weight: 500;
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 8px;
    cursor: default;
    transition: background 120ms ease;
    transform: none;
    box-shadow: none;
  }

  .nav.bookmark {
    position: relative;
  }

  .nav:hover {
    background: var(--bg-hover);
    transform: none;
    box-shadow: none;
  }

  .nav:active {
    transform: none;
    box-shadow: none;
  }

  .nav-icon {
    width: 18px;
    height: 18px;
    object-fit: contain;
    flex-shrink: 0;
  }

  .remove-bookmark {
    margin-left: auto;
    background: transparent;
    border: none;
    color: var(--fg-muted);
    font-size: 14px;
    cursor: pointer;
    opacity: 0;
    transition: opacity 120ms ease;
    padding: 0 4px;
    line-height: 1;
  }

  .nav.bookmark:hover .remove-bookmark {
    opacity: 1;
  }

  .content {
    display: flex;
    flex-direction: column;
    gap: 0;
    min-height: 0;
    min-width: 0;
    color: var(--fg);
    flex: 1;
  }

  .left {
    display: flex;
    gap: 12px;
    align-items: center;
    flex: 1;
    min-width: 0;
  }

.path {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
  width: 100%;
  flex: 1;
  min-width: 0;
}

.path-input {
  flex: 1;
  min-width: 0;
  width: 100%;
  border: 1px solid var(--border);
  border-radius: 0;
  padding: 10px 12px;
  background: var(--bg);
  color: var(--fg);
  font-size: 14px;
}

.path-input:focus {
  outline: 2px solid var(--border-accent);
  border-color: var(--border-accent-strong);
}

.pill {
  background: var(--bg-raised);
  color: var(--fg-pill);
  padding: 6px 10px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
  border: 1px solid var(--border);
}

button {
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 10px 14px;
  background: var(--bg-button);
  color: var(--fg);
  font-weight: 600;
  cursor: pointer;
  transition: background 120ms ease, border-color 120ms ease;
}

button:hover {
  border-color: var(--border-accent);
}

button:active {
}

.error {
  background: var(--bg-raised);
  border: 1px solid var(--border-accent);
  color: #fca5a5;
  padding: 12px 14px;
  border-radius: 12px;
  font-weight: 600;
}

  .list {
    background: transparent;
    border: none;
    border-radius: 0;
    box-shadow: none;
    overflow: auto;
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    min-width: 0;
  }

.rows {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  width: 100%;
  direction: ltr; /* place scrollbar on the right */
  padding-left: 12px;
  padding-right: 12px;
  padding-bottom: 32px; /* keep rows clear of the status bar */
}

.spacer {
  position: relative;
}

.row-viewport {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
}

  .row {
    display: grid;
    gap: 10px;
    align-items: center;
    padding: 0 12px;
    height: 32px;
    min-height: 32px;
    transition: background 120ms ease, border-color 120ms ease;
    cursor: default;
    box-sizing: border-box;
    border: 1px solid transparent;
    background: transparent;
    width: 100%;
    text-align: left;
    border-radius: 10px;
    box-shadow: none;
}

  .row:hover {
    background: #161a20;
    transform: none !important;
    box-shadow: none !important;
  }

  .row.hidden {
    opacity: 0.55;
  }

  .row.selected {
    background: #1c2027;
    border: 1px solid var(--border-accent);
  }

  .row:focus-visible {
    outline: 2px solid var(--border-accent);
  }

  .header-row {
    display: grid;
    gap: 10px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border-strong);
    background: var(--bg-alt);
    color: var(--fg-muted);
    font-size: 12px;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .header-cell {
    display: flex;
    align-items: center;
    position: relative;
    gap: 6px;
    min-width: 0;
    flex: 1 1 0;
  }

  .header-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    justify-content: flex-start;
    flex: 1 1 auto;
    min-width: 0;
    height: 100%;
    border: none;
    background: transparent;
    color: inherit;
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    cursor: pointer;
    padding: 0;
    text-align: left;
  }

  .header-btn.align-right {
    justify-content: flex-end;
    text-align: right;
    margin-right: -10px;
    padding-right: 10px;
  }
  .header-btn.inert {
    cursor: default;
    pointer-events: none;
  }

  .header-btn.active-sort {
    color: var(--fg);
  }

  .resizer {
    flex: 0 0 10px;
    min-width: 10px;
    align-self: stretch;
    cursor: col-resize;
    display: inline-block;
    margin-left: 2px;
    border-radius: 4px;
    transition: background 120ms ease;
    position: relative;
    z-index: 2;
  }

  .resizer:hover {
    background: var(--border);
  }

  .header-btn:focus-visible {
    outline: 2px solid var(--border-accent);
    border-radius: 8px;
    outline-offset: 2px;
  }

  .sort-icon {
    font-size: 11px;
    opacity: 0.8;
    display: inline-flex;
    align-items: center;
    transition: transform 120ms ease;
  }

  .sort-icon.inactive {
    opacity: 0.35;
  }

  .sort-icon.desc {
    transform: rotate(180deg);
  }

  .col-name {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 500;
    color: var(--fg-strong);
    overflow: hidden;
    min-width: 200px;
  }

  .name {
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .col-type,
  .col-modified,
  .col-size {
    color: var(--fg-muted);
    font-size: 13px;
    text-align: left;
    min-width: 100px;
  }

  .col-size {
    font-weight: 600;
    text-align: right;
    min-width: 60px;
  }

  .col-modified {
    min-width: 100px;
  }

  .col-star {
    color: var(--fg-muted);
    font-size: 13px;
    text-align: left;
    min-width: 40px;
  }

  .star-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 28px;
    min-height: 28px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--fg-muted);
    padding: 0px;
    cursor: pointer;
    font-size: 16px;
  }

  .star-btn:hover {
    color: var(--fg-strong);
    border-color: var(--border);
    background: var(--bg);
  }

  .star-btn:focus-visible {
    outline: 2px solid var(--border-accent);
  }

  .star-btn.starred {
    color: #f6d04d;
    animation: star-spin 420ms ease;
  }

  @keyframes star-spin {
    from { transform: rotate(0deg) scale(0.9); }
    50% { transform: rotate(180deg) scale(1.1); }
    to { transform: rotate(360deg) scale(1); }
  }

  .muted {
    padding: 20px;
    color: var(--fg-muted);
    text-align: center;
}

  .icon {
    width: 20px;
    height: 20px;
    object-fit: contain;
    display: inline-block;
  }

  @media (max-width: 640px) {
    .row {
    grid-template-columns: 1.4fr 1fr 0.8fr;
      grid-template-areas:
        'name type size'
        'name modified size';
    }

    .header-row {
      grid-template-columns: 1.4fr 0.8fr 0.6fr;
    }
  }

  .list.wide .row {
    grid-template-columns: 1.6fr 140px 90px 110px 80px;
  }

  .list.wide .header-row {
    grid-template-columns: 1.6fr 180px 180px 140px 90px;
  }

.statusbar {
  height: 32px;
  border-top: 1px solid var(--border-strong);
  background: var(--bg-alt);
  border-radius: 0;
  margin-top: 0;
  position: sticky;
  bottom: 0;
  z-index: 1;
  display: flex;
  align-items: center;
  padding: 0 12px;
  gap: 8px;
}

.status-text {
  color: var(--fg-muted);
  font-size: 12px;
}

.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 10;
}

.modal {
  background: var(--bg);
  border: 1px solid var(--border-strong);
  border-radius: 12px;
  padding: 18px;
  width: min(420px, 90vw);
  box-shadow: 0 16px 32px rgba(0, 0, 0, 0.45);
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.modal-title {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
  color: var(--fg);
}

.modal-desc {
  margin: 0;
  color: var(--fg-muted);
  font-size: 14px;
}

.modal-input {
  width: 100%;
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 10px 12px;
  background: var(--bg);
  color: var(--fg);
  font-size: 14px;
}

.modal-input:focus {
  outline: 2px solid var(--border-accent);
  border-color: var(--border-accent-strong);
}

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.modal-actions button {
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 8px 12px;
  background: var(--bg-button);
  color: var(--fg);
  cursor: pointer;
}

.modal-actions button.secondary {
  background: transparent;
}
</style>
