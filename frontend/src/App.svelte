<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { clampIndex, clearSelection, selectAllPaths, selectRange } from './selection'

  type Entry = {
    name: string
    path: string
    kind: 'dir' | 'file' | 'link'
    ext?: string | null
    size?: number | null
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
  let history: Location[] = []
  let historyIndex = -1
  let rowsEl: HTMLDivElement | null = null
  let viewportHeight = 0
  let scrollTop = 0
  const rowHeight = 32
  const overscan = 8
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

  const bookmarks = [
    { label: 'Projects', path: '~/projects' },
    { label: 'Downloads', path: '~/Downloads' },
  ]

  const partitions = [
    { label: 'Root', path: '/' },
    { label: 'Temp', path: '/tmp' },
  ]

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

  const sortPayload = () => ({
    field: sortField,
    direction: sortDirection,
  })

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
    if (rowsEl) viewportHeight = rowsEl.clientHeight
  }

  const handleRowsScroll = () => {
    if (!rowsEl) return
    scrollTop = rowsEl.scrollTop
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

  const handleGlobalKeydown = (event: KeyboardEvent) => {
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

  $: {
    const next = rowsEl?.clientHeight ?? 0
    if (next !== viewportHeight) {
      viewportHeight = next
    }
  }

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
        if (h > 0 && h !== viewportHeight) {
          viewportHeight = h
        }
      }
    })
    rowsObserver.observe(rowsEl)
  }

  $: {
    if (rowsEl) {
      setupRowsObserver()
    }
  }

  onMount(() => {
    handleResize()
    window.addEventListener('resize', handleResize)
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
      rowsObserver?.disconnect()
      if (unlistenDirChanged) {
        unlistenDirChanged()
        unlistenDirChanged = null
      }
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
            {place.label}
          </button>
        {/each}
      </div>

      <div class="section">
        <div class="section-title">Bookmarks</div>
        {#each bookmarks as mark}
          <button class="nav" type="button" on:click={() => load(mark.path)}>
            {mark.label}
          </button>
        {/each}
      </div>

      <div class="section">
        <div class="section-title">Partitions</div>
        {#each partitions as part}
          <button class="nav" type="button" on:click={() => load(part.path)}>
            {part.label}
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
              placeholder={searchMode ? 'Search in current folder…' : 'Path…'}
              aria-label={searchMode ? 'Search' : 'Path'}
              on:keydown={(e) => {
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
        <div class="actions">
          <label class="mode-toggle">
            <input
              type="checkbox"
              bind:checked={searchMode}
              aria-label="Toggle search mode"
              on:change={async (e) => {
                await toggleMode(e.currentTarget.checked)
              }}
            />
            <span class="slider">{searchMode ? 'Search' : 'Path'}</span>
          </label>
        </div>
      </header>

      {#if error}
        <div class="error">Error: {error}</div>
      {/if}

      {#if searchActive}
        <div class="pill">Searching: "{filter}"</div>
      {/if}

      <section class="list" class:wide={sidebarCollapsed}>
        <div class="header-row">
          <button
            class="header-btn"
            type="button"
            role="columnheader"
            aria-sort={ariaSort('name')}
            class:active-sort={sortField === 'name'}
            on:click={() => changeSort('name')}
          >
            <span>Name</span>
            <span
              class="sort-icon"
              class:desc={sortField === 'name' && sortDirection === 'desc'}
              class:inactive={sortField !== 'name'}
            >
              ▲
            </span>
          </button>
          <button
            class="header-btn"
            type="button"
            role="columnheader"
            aria-sort={ariaSort('type')}
            class:active-sort={sortField === 'type'}
            on:click={() => changeSort('type')}
          >
            <span>Type</span>
            <span
              class="sort-icon"
              class:desc={sortField === 'type' && sortDirection === 'desc'}
              class:inactive={sortField !== 'type'}
            >
              ▲
            </span>
          </button>
          <button
            class="header-btn"
            type="button"
            role="columnheader"
            aria-sort={ariaSort('modified')}
            class:active-sort={sortField === 'modified'}
            on:click={() => changeSort('modified')}
          >
            <span>Modified</span>
            <span
              class="sort-icon"
              class:desc={sortField === 'modified' && sortDirection === 'desc'}
              class:inactive={sortField !== 'modified'}
            >
              ▲
            </span>
          </button>
          <button
            class="header-btn"
            type="button"
            role="columnheader"
            aria-sort={ariaSort('size')}
            class:active-sort={sortField === 'size'}
            on:click={() => changeSort('size')}
          >
            <span>Size</span>
            <span
              class="sort-icon"
              class:desc={sortField === 'size' && sortDirection === 'desc'}
              class:inactive={sortField !== 'size'}
            >
              ▲
            </span>
          </button>
          <button
            class="header-btn"
            type="button"
            role="columnheader"
            aria-sort={ariaSort('starred')}
            class:active-sort={sortField === 'starred'}
            on:click={() => changeSort('starred')}
          >
            <span>⭐</span>
            <span
              class="sort-icon"
              class:desc={sortField === 'starred' && sortDirection === 'desc'}
              class:inactive={sortField !== 'starred'}
            >
              ▲
            </span>
          </button>
        </div>
        {#if !loading && filteredEntries.length === 0}
          <div class="muted">No items here.</div>
        {:else}
          <div
            class="rows"
            bind:this={rowsEl}
            on:scroll={handleRowsScroll}
            on:keydown={handleRowsKeydown}
            on:click={handleRowsClick}
            tabindex="0"
            role="grid"
            aria-label="File list"
          >
            <div class="spacer" style={`height:${totalHeight}px`}>
              <div class="row-viewport" style={`transform: translateY(${offsetY}px)`}>
                {#each visibleEntries as entry, i (entry.path)}
                  <button
                    class="row"
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
                      {entry.kind === 'file' ? formatSize(entry.size) : 'Folder'}
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
          </div>
        {/if}
      </section>
    </section>
  </div>
</main>
<footer class="statusbar"></footer>

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
    padding: 0 0 8px 0;
  }

  .layout {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 16px;
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
    border-radius: 14px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
    overflow: auto;
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.35);
  }

  .sidebar.collapsed {
    display: none;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .section-title {
    color: var(--fg-muted);
    font-size: 12px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-weight: 700;
  }

  .nav {
    border: 1px solid var(--border-strong);
    border-radius: 10px;
    padding: 10px 12px;
    background: var(--bg);
    color: var(--fg);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease, transform 120ms ease;
  }

  .nav:hover {
    background: var(--bg-hover);
    border-color: #2f333b;
    transform: translateY(-1px);
  }

  .content {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    color: var(--fg);
    flex: 1;
  }

  .left {
    display: flex;
    gap: 12px;
    align-items: center;
    flex: 1;
  }

.path {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
  width: 100%;
  flex: 1;
}

.path-input {
  flex: 1;
  min-width: 240px;
  border: 1px solid var(--border);
  border-radius: 10px;
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

  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .mode-toggle {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    user-select: none;
  }

  .mode-toggle input {
    display: none;
  }

  .slider {
    min-width: 90px;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--bg);
    color: var(--fg);
    font-weight: 600;
    text-align: center;
    transition: background 120ms ease, border-color 120ms ease;
  }

  .mode-toggle input:checked + .slider {
    background: var(--bg-raised);
    border-color: var(--border-accent);
  }

button {
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 10px 14px;
  background: var(--bg-button);
  color: var(--fg);
  font-weight: 600;
  cursor: pointer;
  transition: transform 120ms ease, box-shadow 120ms ease, border-color 120ms ease;
}

button:hover {
  transform: translateY(-1px);
  box-shadow: 0 8px 16px rgba(0, 0, 0, 0.25);
  border-color: var(--border-accent);
}

button:active {
  transform: translateY(0);
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
    overflow: hidden;
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow-x: auto;
  }

.rows {
  flex: 1;
  min-height: 0;
  overflow: auto;
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
    grid-template-columns: 1.4fr 0.6fr 0.6fr 0.3fr 0.2fr;
    gap: 10px;
    align-items: center;
    padding: 0 12px;
    height: 32px;
    min-height: 32px;
    transition: background 120ms ease, border-color 120ms ease;
    cursor: default;
  border: none;
  background: transparent;
  width: 100%;
  text-align: left;
  border-radius: 10px;
  box-shadow: none;
}

  .row:last-child {
    border-bottom: none;
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
    grid-template-columns: 1.4fr 0.6fr 0.6fr 0.3fr 0.2fr;
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

  .header-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    justify-content: flex-start;
    width: 100%;
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

  .header-btn.active-sort {
    color: var(--fg);
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
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.3;
  }

  .col-type,
  .col-modified,
  .col-size,
  .col-star {
    color: var(--fg-muted);
    font-size: 13px;
    text-align: left;
    min-width: 120px;
  }

  .col-size {
    font-weight: 600;
    min-width: 80px;
    text-align: right;
  }

  .col-modified {
    min-width: 100px;
  }

  .col-star {
    text-align: center;
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
    padding: 6px;
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
      grid-template-columns: 1.4fr 0.6fr 0.8fr;
      grid-template-areas:
        'name type size'
        'name modified size';
    }

    .header-row {
      grid-template-columns: 1.4fr 0.6fr 0.8fr;
    }
  }

  .list.wide .row {
    grid-template-columns: 2fr 0.8fr 1fr 0.7fr 0.3fr;
  }

  .list.wide .header-row {
    grid-template-columns: 2fr 0.8fr 1fr 0.7fr 0.3fr;
  }

  .statusbar {
    height: 32px;
    border-top: 1px solid var(--border-strong);
    background: var(--bg-alt);
    border-radius: 12px 12px 0 0;
    margin-top: 12px;
    position: sticky;
    bottom: 0;
    z-index: 1;
  }
</style>
