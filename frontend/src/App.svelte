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
  }

  type Listing = {
    current: string
    entries: Entry[]
  }

  let current = ''
  let entries: Entry[] = []
  let loading = false
  let error = ''
  let filter = ''
  let searchActive = false
  let sidebarCollapsed = false
  let pathInput = ''
  let selected = clearSelection()
  let anchorIndex: number | null = null
  let caretIndex: number | null = null
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
    { label: 'Desktop', path: '~/Desktop' },
    { label: 'Documents', path: '~/Documents' },
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

  const load = async (path?: string) => {
    loading = true
    error = ''
    searchActive = false
    try {
      const result = await invoke<Listing>('list_dir', { path })
      current = result.current
      pathInput = result.current
      entries = result.entries
      // Start watching this directory for changes.
      await invoke('watch_dir', { path: current })
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
    void load(pathInput.trim())
  }

  const isHidden = (entry: Entry) => entry.name.startsWith('.')

  const displayName = (entry: Entry) => {
    if (entry.kind === 'file' && entry.ext) {
      return entry.name.replace(new RegExp(`\\.${entry.ext}$`), '')
    }
    return entry.name
  }

  const runSearch = async () => {
    if (filter.trim().length === 0) {
      searchActive = false
      await load(current)
      return
    }
    loading = true
    error = ''
    searchActive = true
    try {
      const result = await invoke<Entry[]>('search', { path: current, query: filter })
      entries = result
    } catch (err) {
      error = err instanceof Error ? err.message : String(err)
    } finally {
      loading = false
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
    void load()

    // Listen for backend watcher events to refresh the current directory.
    void (async () => {
      unlistenDirChanged = await listen<string>('dir-changed', (event) => {
        if (!current || event.payload === current) {
          if (refreshTimer) {
            clearTimeout(refreshTimer)
          }
          refreshTimer = setTimeout(() => {
            void load(current)
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
    }
  })
</script>

<main class="shell">
  <div class="layout" class:collapsed={sidebarCollapsed}>
    <aside class:collapsed={sidebarCollapsed} class="sidebar">
      <div class="section">
        <div class="section-title">Places</div>
        {#each places as place}
          <button class="nav" type="button" on:click={() => load(place.path)}>
            {place.label}
            <span class="dim">{place.path}</span>
          </button>
        {/each}
      </div>

      <div class="section">
        <div class="section-title">Bookmarks</div>
        {#each bookmarks as mark}
          <button class="nav" type="button" on:click={() => load(mark.path)}>
            {mark.label}
            <span class="dim">{mark.path}</span>
          </button>
        {/each}
      </div>

      <div class="section">
        <div class="section-title">Partitions</div>
        {#each partitions as part}
          <button class="nav" type="button" on:click={() => load(part.path)}>
            {part.label}
            <span class="dim">{part.path}</span>
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
              placeholder="Path..."
              aria-label="Path"
              on:keydown={(e) => e.key === 'Enter' && goToPath()}
            />
          </div>
          {#if loading}
            <span class="pill">Loading…</span>
          {/if}
        </div>
        <div class="actions">
          <input
            class="filter"
            placeholder="Filter..."
            bind:value={filter}
            aria-label="Filter"
            on:keydown={(e) => e.key === 'Enter' && runSearch()}
          />
          <button class="ghost" on:click={() => load(current)}>Refresh</button>
          <button class="primary" on:click={runSearch}>Search</button>
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
          <div class="col-name">Name</div>
          <div class="col-type">Type</div>
          <div class="col-modified">Modified</div>
          <div class="col-size">Size</div>
          <div class="col-star">⭐</div>
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
                    <div class="col-star">☆</div>
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
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  color: var(--fg);
  min-height: 100vh;
}

  .topbar {
    display: flex;
    gap: 12px;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
  }

  .layout {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 16px;
    align-items: start;
    min-height: 0;
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
    position: sticky;
    top: 24px;
    align-self: start;
    max-height: calc(100vh - 48px);
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

.dim {
    color: var(--fg-dim);
    font-size: 12px;
  }

.content {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    color: var(--fg);
  }

  .left {
    display: flex;
    gap: 12px;
    align-items: center;
  }

.path {
  display: flex;
  gap: 8px;
  align-items: center;
  flex-wrap: wrap;
  width: 100%;
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

.filter {
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 10px 12px;
  min-width: 180px;
  background: var(--bg);
  color: var(--fg);
  font-size: 14px;
}

.filter:focus {
  outline: 2px solid var(--border-accent);
  border-color: var(--border-accent-strong);
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

button.ghost {
  background: var(--bg);
  border-color: var(--border);
}

button.primary {
  background: var(--bg-raised);
  color: var(--fg);
  border-color: var(--border-accent);
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
    grid-template-columns: 1.4fr 0.6fr 0.8fr 0.4fr 0.2fr;
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
    grid-template-columns: 1.4fr 0.6fr 0.8fr 0.4fr 0.2fr;
    gap: 10px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border-strong);
    background: var(--bg-alt);
    color: var(--fg-muted);
    font-size: 12px;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }

  .col-name {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 500;
    color: var(--fg-strong);
    overflow: hidden;
  }

  .name {
    font-size: 14px;
    font-weight: 500;
    display: -webkit-box;
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
  }

  .col-size {
    font-weight: 600;
  }

  .col-star {
    text-align: center;
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
