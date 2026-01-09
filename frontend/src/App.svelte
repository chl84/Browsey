<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'

  type Entry = {
    name: string
    path: string
    kind: 'dir' | 'file'
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
  let rowsEl: HTMLDivElement | null = null
  let viewportHeight = 0
  let scrollTop = 0
  const rowHeight = 32
  const overscan = 8
  let unlistenDirChanged: UnlistenFn | null = null
  let refreshTimer: ReturnType<typeof setTimeout> | null = null

  const places = [
    { label: 'Hjem', path: '~' },
    { label: 'Desktop', path: '~/Desktop' },
    { label: 'Documents', path: '~/Documents' },
  ]

  const bookmarks = [
    { label: 'Prosjekter', path: '~/projects' },
    { label: 'Downloads', path: '~/Downloads' },
  ]

  const partitions = [
    { label: 'Root', path: '/' },
    { label: 'Tmp', path: '/tmp' },
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

  const isHidden = (entry: Entry) => entry.name.startsWith('.')

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
            <button class="ghost" on:click={goHome} title="Hjem">Hjem</button>
            <button class="ghost" on:click={goUp} title="Opp">Opp</button>
            <span class="crumb">{current || '...'}</span>
          </div>
          {#if loading}
            <span class="pill">Laster…</span>
          {/if}
        </div>
        <div class="actions">
          <input
            class="filter"
            placeholder="Filtrer..."
            bind:value={filter}
            aria-label="Filtrer"
            on:keydown={(e) => e.key === 'Enter' && runSearch()}
          />
          <button class="ghost" on:click={() => load(current)}>Oppdater</button>
          <button class="primary" on:click={runSearch}>Søk</button>
        </div>
      </header>

      {#if error}
        <div class="error">Feil: {error}</div>
      {/if}

      {#if searchActive}
        <div class="pill">Søker: "{filter}"</div>
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
          <div class="muted">Ingen elementer her.</div>
        {:else}
          <div
            class="rows"
            bind:this={rowsEl}
            on:scroll={handleRowsScroll}
          >
            <div class="spacer" style={`height:${totalHeight}px`}>
              <div class="row-viewport" style={`transform: translateY(${offsetY}px)`}>
                {#each visibleEntries as entry (entry.path)}
                  <button class="row" class:hidden={isHidden(entry)} type="button" on:click={() => open(entry)}>
                    <div class="col-name">
                      <img class="icon" src={entry.icon} alt="" />
                      <span class="name">{entry.name}</span>
                    </div>
                    <div class="col-type">{entry.kind === 'dir' ? 'Folder' : 'File'}</div>
                    <div class="col-modified">{entry.modified ?? '—'}</div>
                    <div class="col-size">
                      {entry.kind === 'file' ? formatSize(entry.size) : 'Mappe'}
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

<style>
.shell {
  width: 100%;
  max-width: none;
  margin: 0;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  color: #e5e7eb;
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
  }

  .layout.collapsed {
    grid-template-columns: 1fr;
  }

  .sidebar {
    background: #0d0f13;
    border: 1px solid #1f242c;
    border-radius: 14px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 70vh;
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
    color: #9ca3af;
    font-size: 12px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-weight: 700;
  }

  .nav {
    border: 1px solid #1f242c;
    border-radius: 10px;
    padding: 10px 12px;
    background: #0f1115;
    color: #e5e7eb;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    cursor: pointer;
    transition: background 120ms ease, border-color 120ms ease, transform 120ms ease;
  }

  .nav:hover {
    background: #141820;
    border-color: #2f333b;
    transform: translateY(-1px);
  }

  .dim {
    color: #6b7280;
    font-size: 12px;
  }

  .content {
    display: flex;
    flex-direction: column;
    gap: 12px;
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
}

.crumb {
  font-weight: 600;
  color: #e5e7eb;
  background: #1b1e24;
  border-radius: 12px;
  padding: 6px 10px;
  border: 1px solid #2a2e35;
}

.pill {
  background: #1b1e24;
  color: #cbd5e1;
  padding: 6px 10px;
  border-radius: 999px;
  font-size: 12px;
  font-weight: 600;
  border: 1px solid #2a2e35;
}

  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

.filter {
  border: 1px solid #2a2e35;
  border-radius: 10px;
  padding: 10px 12px;
  min-width: 180px;
  background: #0f1115;
  color: #e5e7eb;
  font-size: 14px;
}

.filter:focus {
  outline: 2px solid #3f444c;
  border-color: #4b5563;
}

button {
  border: 1px solid #2a2e35;
  border-radius: 10px;
  padding: 10px 14px;
  background: #12151b;
  color: #e5e7eb;
  font-weight: 600;
  cursor: pointer;
  transition: transform 120ms ease, box-shadow 120ms ease, border-color 120ms ease;
}

button:hover {
  transform: translateY(-1px);
  box-shadow: 0 8px 16px rgba(0, 0, 0, 0.25);
  border-color: #3f444c;
}

button:active {
  transform: translateY(0);
}

button.ghost {
  background: #0f1115;
  border-color: #2a2e35;
}

button.primary {
  background: #1b1e24;
  color: #e5e7eb;
  border-color: #3f444c;
}

.error {
  background: #1b1e24;
  border: 1px solid #3f444c;
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
}

.rows {
  max-height: calc(100vh - 220px);
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
    cursor: pointer;
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

  .row:focus-visible {
    outline: 2px solid #3f444c;
  }

  .header-row {
    display: grid;
    grid-template-columns: 1.4fr 0.6fr 0.8fr 0.4fr 0.2fr;
    gap: 10px;
    padding: 6px 12px;
    border-bottom: 1px solid #1f242c;
    background: #0d0f13;
    color: #9ca3af;
    font-size: 12px;
    letter-spacing: 0.02em;
    text-transform: uppercase;
  }

  .col-name {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 500;
    color: #f3f4f6;
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
    color: #cbd5e1;
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
    color: #94a3b8;
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
</style>
