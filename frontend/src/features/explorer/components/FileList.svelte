<script lang="ts">
  import FileListHeader from './FileListHeader.svelte'
  import FileRow from './FileRow.svelte'
  import SelectionBox from '../../../ui/SelectionBox.svelte'
  import ColumnFilterMenu from './ColumnFilterMenu.svelte'
  import ContextMenu from './ContextMenu.svelte'
  import { invoke } from '@tauri-apps/api/core'
  import type { Column, Entry, SortDirection, SortField, FilterOption } from '../types'
  import { nameFilterOptions } from '../filters/nameFilters'

  export let cols: Column[] = []
  export let gridTemplate = ''
  export let rowsEl: HTMLDivElement | null = null
  export let headerEl: HTMLDivElement | null = null
  export let loading = false
  export let filteredEntries: Entry[] = []
  export let visibleEntries: Entry[] = []
  export let columnFilters: {
    name: Set<string>
    type: Set<string>
    modified: Set<string>
    size: Set<string>
  } = { name: new Set(), type: new Set(), modified: new Set(), size: new Set() }
  export let start = 0
  export let offsetY = 0
  export let totalHeight = 0
  export let currentPath: string | null = null
  export let wide = false
  export let selected: Set<string> = new Set()
  export let sortField: SortField = 'name'
  export let sortDirection: SortDirection = 'asc'
  export let isHidden: (entry: Entry) => boolean = () => false
  export let displayName: (entry: Entry) => string = (e) => e.name
  export let formatSize: (size?: number | null) => string = () => ''
  export let formatItems: (count?: number | null) => string = () => ''
  export let clipboardMode: 'copy' | 'cut' = 'copy'
  export let clipboardPaths: Set<string> = new Set()

  export let onRowsScroll: (event: Event) => void = () => {}
  export let onWheel: (event: WheelEvent) => void = () => {}
  export let onRowsKeydown: (event: KeyboardEvent) => void = () => {}
  export let onRowsMousedown: (event: MouseEvent) => void = () => {}
  export let onRowsClick: (event: MouseEvent) => void = () => {}
  export let onRowsContextMenu: (event: MouseEvent) => void = () => {}
  export let onChangeSort: (field: SortField) => void = () => {}
  export let onToggleFilter: (field: SortField, id: string, checked: boolean) => void = () => {}
  export let onResetFilter: (field: SortField) => void = () => {}
  export let onStartResize: (index: number, event: PointerEvent) => void = () => {}
  export let ariaSort: (field: SortField) => 'ascending' | 'descending' | 'none' = () => 'none'
  export let onRowClick: (entry: Entry, absoluteIndex: number, event: MouseEvent) => void = () => {}
  export let onOpen: (entry: Entry) => void = () => {}
  export let onToggleStar: (entry: Entry) => void = () => {}
  export let onContextMenu: (entry: Entry, event: MouseEvent) => void = () => {}
  export let onRowDragStart: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragEnd: (event: DragEvent) => void = () => {}
  export let onRowDragOver: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragEnter: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDrop: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragLeave: (entry: Entry, event: DragEvent) => void = () => {}
  export let selectionActive = false
  export let selectionRect: { x: number; y: number; width: number; height: number } = {
    x: 0,
    y: 0,
    width: 0,
    height: 0,
  }
  export let dragTargetPath: string | null = null
  export let dragAllowed = false
  export let dragging = false

  // Filter UI state (logic application is deferred; we only manage selection/visibility).
  let filterMenuOpen = false
  let filterMenuAnchor: DOMRect | null = null
  let filterMenuField: SortField | null = null
  let filterMenuTitle = 'Filters'
  let filterMenuOptions: FilterOption[] = nameFilterOptions
  $: activeNameFilters = columnFilters.name
  $: activeTypeFilters = columnFilters.type
  $: activeModifiedFilters = columnFilters.modified
  $: activeSizeFilters = columnFilters.size
  let filterCtxOpen = false
  let filterCtxX = 0
  let filterCtxY = 0
  let filterCtxField: SortField | null = null

  const typeLabel = (entry: Entry) => {
    if (entry.ext && entry.ext.length > 0) return entry.ext.toLowerCase()
    if (entry.kind) return entry.kind.toLowerCase()
    return ''
  }

  const bucketModified = (modified?: string | null): string | null => {
    if (!modified) return null
    const iso = modified.replace(' ', 'T')
    const date = new Date(iso)
    if (Number.isNaN(date.getTime())) return null
    const now = new Date()
    const msPerDay = 1000 * 60 * 60 * 24
    const diffDays = Math.floor((now.getTime() - date.getTime()) / msPerDay)
    if (diffDays <= 0) return 'Today'
    if (diffDays === 1) return 'Yesterday'
    if (diffDays < 7) return `${diffDays} days ago`
    if (diffDays < 30) {
      const weeks = Math.floor((diffDays + 6) / 7)
      return weeks === 1 ? '1 week ago' : `${weeks} weeks ago`
    }
    if (diffDays < 365) {
      const months = Math.floor((diffDays + 29) / 30)
      return months === 1 ? '1 month ago' : `${months} months ago`
    }
    const years = Math.floor((diffDays + 364) / 365)
    return years === 1 ? '1 year ago' : `${years} years ago`
  }

  const bucketSize = (size: number): { label: string; rank: number } | null => {
    const KB = 1024
    const MB = 1024 * KB
    const GB = 1024 * MB
    const TB = 1024 * GB
    const buckets: Array<[number, string]> = [
      [10 * KB, '0–10 KB'],
      [100 * KB, '10–100 KB'],
      [MB, '100 KB–1 MB'],
      [10 * MB, '1–10 MB'],
      [100 * MB, '10–100 MB'],
      [GB, '100 MB–1 GB'],
      [10 * GB, '1–10 GB'],
      [100 * GB, '10–100 GB'],
      [TB, '100 GB–1 TB'],
    ]
    for (const [limit, label] of buckets) {
      if (size <= limit) return { label, rank: limit }
    }
    const over = Math.max(1, Math.floor(size / TB))
    return { label: 'Over 1 TB', rank: over * TB }
  }

  const bucketRank = (label: string): number => {
    if (label === 'Today') return 0
    if (label === 'Yesterday') return 1
    const daysMatch = label.match(/^(\d+) days ago$/)
    if (daysMatch) return parseInt(daysMatch[1], 10)
    const weeksMatch = label.match(/^(\d+) weeks ago$/)
    if (weeksMatch) return parseInt(weeksMatch[1], 10) * 7
    if (label === '1 week ago') return 7
    const monthsMatch = label.match(/^(\d+) months ago$/)
    if (monthsMatch) return parseInt(monthsMatch[1], 10) * 30
    if (label === '1 month ago') return 30
    const yearsMatch = label.match(/^(\d+) years ago$/)
    if (yearsMatch) return parseInt(yearsMatch[1], 10) * 365
    if (label === '1 year ago') return 365
    // size buckets
    const sizeOrder = [
      '0–10 KB',
      '10–100 KB',
      '100 KB–1 MB',
      '1–10 MB',
      '10–100 MB',
      '100 MB–1 GB',
      '1–10 GB',
      '10–100 GB',
      '100 GB–1 TB',
      'Over 1 TB',
    ]
    const idx = sizeOrder.indexOf(label)
    if (idx >= 0) return idx
    return Number.MAX_SAFE_INTEGER
  }

  const handleFilterClick = (field: SortField, anchor: DOMRect) => {
    if (field === 'name') {
      filterMenuOpen = true
      filterMenuAnchor = anchor
      filterMenuField = 'name'
      filterMenuTitle = 'Name filters'
      filterMenuOptions = nameFilterOptions
      return
    }
    if (field === 'type') {
      filterMenuField = 'type'
      void handleTypeFilterClick(anchor)
      return
    }
    if (field === 'modified') {
      filterMenuField = 'modified'
      void handleModifiedFilterClick(anchor)
      return
    }
    if (field === 'size') {
      filterMenuField = 'size'
      void handleSizeFilterClick(anchor)
      return
    }
  }

  const handleTypeFilterClick = async (anchor: DOMRect) => {
    filterMenuOpen = true
    filterMenuAnchor = anchor
    filterMenuTitle = 'Type filters'
    // Prefer current result set (search/filter) to stay in sync with visible items.
    const localSet = new Set<string>()
    for (const e of filteredEntries) {
      if (e.hidden) continue
      localSet.add(typeLabel(e))
    }
    if (localSet.size > 0) {
      filterMenuOptions = Array.from(localSet)
        .sort((a, b) => a.localeCompare(b))
        .map((v) => ({ id: `type:${v}`, label: v || 'unknown' }))
      return
    }

    if (!currentPath) return
    try {
      const values = await invoke<string[]>('list_column_values', { path: currentPath, column: 'type' })
      filterMenuOptions = values
        .map((v: string) => ({ id: `type:${v}`, label: v || 'unknown' }))
    } catch (err) {
      console.error('Failed to load type filters', err)
      filterMenuOptions = []
    }
  }

  const handleSizeFilterClick = async (anchor: DOMRect) => {
    filterMenuOpen = true
    filterMenuAnchor = anchor
    filterMenuTitle = 'Size filters'

    const localBuckets = new Map<string, number>()
    for (const e of filteredEntries) {
      if (e.hidden) continue
      if (e.kind !== 'file') continue
      if (typeof e.size === 'number') {
        const bucket = bucketSize(e.size)
        if (bucket) localBuckets.set(bucket.label, bucket.rank)
      }
    }
    if (localBuckets.size > 0) {
      filterMenuOptions = Array.from(localBuckets.entries())
        .sort((a, b) => a[1] - b[1])
        .map(([v]) => ({ id: `size:${v}`, label: v }))
      return
    }

    if (!currentPath) return
    try {
      const values = await invoke<string[]>('list_column_values', { path: currentPath, column: 'size' })
      filterMenuOptions = values.map((v: string) => ({ id: `size:${v}`, label: v }))
    } catch (err) {
      console.error('Failed to load size filters', err)
      filterMenuOptions = []
    }
  }

  const handleModifiedFilterClick = async (anchor: DOMRect) => {
    filterMenuOpen = true
    filterMenuAnchor = anchor
    filterMenuTitle = 'Modified filters'
    const localBuckets = new Map<string, number>()
    for (const e of filteredEntries) {
      if (e.hidden) continue
      const bucket = bucketModified(e.modified)
      if (bucket) {
        const rank = bucketRank(bucket)
        localBuckets.set(bucket, rank)
      }
    }
    if (localBuckets.size > 0) {
      filterMenuOptions = Array.from(localBuckets.entries())
        .sort((a, b) => a[1] - b[1])
        .map(([v]) => ({ id: `modified:${v}`, label: v }))
      return
    }
    if (!currentPath) return
    try {
      const values = await invoke<string[]>('list_column_values', { path: currentPath, column: 'modified' })
      filterMenuOptions = values.map((v: string) => ({ id: `modified:${v}`, label: v }))
    } catch (err) {
      console.error('Failed to load modified filters', err)
      filterMenuOptions = []
    }
  }

  const handleToggleNameFilter = (id: string, checked: boolean) => onToggleFilter('name', id, checked)
  const handleToggleTypeFilter = (id: string, checked: boolean) => onToggleFilter('type', id, checked)
  const handleToggleModifiedFilter = (id: string, checked: boolean) =>
    onToggleFilter('modified', id, checked)
  const handleToggleSizeFilter = (id: string, checked: boolean) => onToggleFilter('size', id, checked)

  const closeFilterMenu = () => {
    filterMenuOpen = false
  }

  const openFilterContextMenu = (field: SortField, event: MouseEvent) => {
    event.preventDefault()
    filterMenuOpen = false
    filterCtxField = field
    filterCtxX = event.clientX
    filterCtxY = event.clientY
    filterCtxOpen = true
  }

  const closeFilterContextMenu = () => {
    filterCtxOpen = false
    filterCtxField = null
  }

  const handleFilterContextSelect = (id: string) => {
    if (id !== 'reset' || !filterCtxField) {
      closeFilterContextMenu()
      return
    }
    onResetFilter(filterCtxField)
    closeFilterContextMenu()
  }

  $: filterActive = {
    name: activeNameFilters.size > 0,
    type: activeTypeFilters.size > 0,
    modified: activeModifiedFilters.size > 0,
    size: activeSizeFilters.size > 0,
    starred: false,
  }
</script>

<section class="list" class:wide={wide}>
  <div
    class="rows"
    bind:this={rowsEl}
    on:scroll={onRowsScroll}
    on:wheel={onWheel}
    on:keydown={onRowsKeydown}
    on:mousedown={onRowsMousedown}
    on:click={onRowsClick}
    on:contextmenu={onRowsContextMenu}
    tabindex="0"
    role="grid"
    aria-label="File list"
  >
    <FileListHeader
      {cols}
      {gridTemplate}
      bind:headerEl
      {sortField}
      {sortDirection}
      {ariaSort}
      onChangeSort={onChangeSort}
      onStartResize={onStartResize}
      onFilterClick={handleFilterClick}
      onFilterContextMenu={openFilterContextMenu}
      filterActive={filterActive}
    />
    {#if !loading && filteredEntries.length === 0}
      <div class="muted">No items here.</div>
    {:else}
      <div class="spacer" style={`height:${totalHeight}px`}>
        <div class="row-viewport" style={`top:${offsetY}px`}>
          {#each visibleEntries as entry, i (entry.path)}
            <FileRow
              {entry}
              index={start + i}
              gridTemplate={gridTemplate}
              hidden={isHidden(entry)}
              selected={selected.has(entry.path)}
              cutting={clipboardMode === 'cut' && clipboardPaths.has(entry.path)}
              dropActive={dragTargetPath === entry.path}
              dropAllowed={dragAllowed && dragTargetPath === entry.path}
              dragging={dragging}
              displayName={displayName}
              {formatSize}
              {formatItems}
              onOpen={onOpen}
              onClick={(event) => onRowClick(entry, start + i, event)}
              onDragStart={(event) => onRowDragStart(entry, event)}
              onDragEnd={onRowDragEnd}
              onDragEnterRow={(event) => onRowDragEnter(entry, event)}
              onDragOverRow={(event) => onRowDragOver(entry, event)}
              onDragLeaveRow={(event) => onRowDragLeave(entry, event)}
              onDropRow={(event) => onRowDrop(entry, event)}
              onToggleStar={onToggleStar}
              onContextMenu={(event) => onContextMenu(entry, event)}
            />
          {/each}
        </div>
      </div>
    {/if}
    <SelectionBox active={selectionActive} rect={selectionRect} />
  </div>
</section>

<ColumnFilterMenu
  open={filterMenuOpen}
  options={filterMenuOptions}
  selected={
    filterMenuField === 'type'
      ? activeTypeFilters
      : filterMenuField === 'modified'
        ? activeModifiedFilters
        : filterMenuField === 'size'
          ? activeSizeFilters
          : activeNameFilters
  }
  anchor={filterMenuAnchor}
  onToggle={(id, checked) => {
    if (id.startsWith('name:')) return handleToggleNameFilter(id, checked)
    if (id.startsWith('type:')) return handleToggleTypeFilter(id, checked)
    if (id.startsWith('modified:')) return handleToggleModifiedFilter(id, checked)
    if (id.startsWith('size:')) return handleToggleSizeFilter(id, checked)
  }}
  onClose={closeFilterMenu}
/>

<ContextMenu
  open={filterCtxOpen}
  x={filterCtxX}
  y={filterCtxY}
  actions={[{ id: 'reset', label: 'Reset' }]}
  onSelect={handleFilterContextSelect}
  onClose={closeFilterContextMenu}
/>

<style>
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
    min-width: 0;
  }

  .rows {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    width: 100%;
    direction: ltr;
    padding-left: 15px;
    padding-right: 20px; /* move scrollbar inward ~8px to keep native resize grip clear */
    padding-bottom: 32px;
    position: relative;
    user-select: none;
    cursor: default;
    scrollbar-gutter: stable;
  }

  .rows::-webkit-scrollbar-corner {
    background: var(--bg);
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

  .muted {
    padding: 20px;
    color: var(--fg-muted);
    text-align: center;
  }
</style>
