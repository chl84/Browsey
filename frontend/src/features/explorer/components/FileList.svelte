<script lang="ts">
  import FileListHeader from './FileListHeader.svelte'
  import FileRow from './FileRow.svelte'
  import SelectionBox from '../../../ui/SelectionBox.svelte'
  import ColumnFilterMenu from './ColumnFilterMenu.svelte'
  import ContextMenu from './ContextMenu.svelte'
  import { invoke } from '@tauri-apps/api/core'
  import type { Column, Entry, SortDirection, SortField, FilterOption } from '../types'
  import {
    nameFilterOptions,
    nameBucket,
    nameFilterLabel,
    nameFilterRank,
  } from '../filters/nameFilters'
  import { modifiedBucket, sizeBucket, typeLabel } from '../filters/columnBuckets'

  export let cols: Column[] = []
  export let gridTemplate = ''
  export let rowsEl: HTMLDivElement | null = null
  export let headerEl: HTMLDivElement | null = null
  export let loading = false
  export let filteredEntries: Entry[] = []
  export let visibleEntries: Entry[] = []
  export let filterValue = ''
  export let showHidden = false
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
  let filterMenuRequestSeq = 0
  $: activeNameFilters = columnFilters.name
  $: activeTypeFilters = columnFilters.type
  $: activeModifiedFilters = columnFilters.modified
  $: activeSizeFilters = columnFilters.size
  let filterCtxOpen = false
  let filterCtxX = 0
  let filterCtxY = 0
  let filterCtxField: SortField | null = null

  const entriesForFilterField = (field: 'name' | 'type' | 'modified' | 'size'): Entry[] => {
    const needle = filterValue.trim().toLowerCase()
    let base =
      needle.length === 0
        ? visibleEntries
        : visibleEntries.filter((e) => (e.nameLower ?? e.name.toLowerCase()).includes(needle))

    const hasName = field !== 'name' && columnFilters.name.size > 0
    const hasType = field !== 'type' && columnFilters.type.size > 0
    const hasModified = field !== 'modified' && columnFilters.modified.size > 0
    const hasSize = field !== 'size' && columnFilters.size.size > 0
    if (!hasName && !hasType && !hasModified && !hasSize) return base

    base = base.filter((e) => {
      if (hasName) {
        const bucket = nameBucket(e.nameLower ?? e.name.toLowerCase())
        if (!columnFilters.name.has(bucket)) return false
      }

      if (hasType) {
        const label = typeLabel(e)
        if (!columnFilters.type.has(`type:${label}`)) return false
      }

      if (hasModified) {
        const bucket = modifiedBucket(e.modified)
        if (!bucket) return false
        if (!columnFilters.modified.has(`modified:${bucket.label}`)) return false
      }

      if (hasSize) {
        if (e.kind !== 'file') return false
        if (typeof e.size !== 'number') return false
        const bucket = sizeBucket(e.size)
        if (!bucket) return false
        if (!columnFilters.size.has(`size:${bucket.label}`)) return false
      }

      return true
    })

    return base
  }

  const handleFilterClick = (field: SortField, anchor: DOMRect) => {
    if (field === 'name') {
      void handleNameFilterClick(anchor)
      return
    }
    if (field === 'type') {
      void handleTypeFilterClick(anchor)
      return
    }
    if (field === 'modified') {
      void handleModifiedFilterClick(anchor)
      return
    }
    if (field === 'size') {
      void handleSizeFilterClick(anchor)
      return
    }
  }

  const beginFilterMenuRequest = (field: SortField, title: string, anchor: DOMRect): number => {
    filterMenuRequestSeq += 1
    filterMenuOpen = true
    filterMenuAnchor = anchor
    filterMenuField = field
    filterMenuTitle = title
    filterMenuOptions = []
    return filterMenuRequestSeq
  }

  const isFilterMenuRequestActive = (field: SortField, requestId: number): boolean =>
    filterMenuOpen && filterMenuField === field && filterMenuRequestSeq === requestId

  const applyFilterMenuOptions = (field: SortField, requestId: number, options: FilterOption[]): void => {
    if (!isFilterMenuRequestActive(field, requestId)) return
    filterMenuOptions = options
  }

  const handleNameFilterClick = async (anchor: DOMRect) => {
    const requestId = beginFilterMenuRequest('name', 'Name filters', anchor)

    const optionEntries = entriesForFilterField('name')
    const localBuckets = new Set<string>()
    for (const e of optionEntries) {
      const bucket = nameBucket(e.nameLower ?? e.name.toLowerCase())
      localBuckets.add(bucket)
    }
    if (localBuckets.size > 0) {
      applyFilterMenuOptions(
        'name',
        requestId,
        Array.from(localBuckets)
          .sort((a, b) => nameFilterRank(a) - nameFilterRank(b))
          .map((id) => ({ id, label: nameFilterLabel(id) })),
      )
      return
    }

    if (!currentPath) return
    try {
      const values = await invoke<string[]>('list_column_values', {
        path: currentPath,
        column: 'name',
        includeHidden: showHidden,
      })
      applyFilterMenuOptions(
        'name',
        requestId,
        values
          .sort((a, b) => nameFilterRank(a) - nameFilterRank(b))
          .map((id: string) => ({ id, label: nameFilterLabel(id) })),
      )
    } catch (err) {
      console.error('Failed to load name filters', err)
      applyFilterMenuOptions('name', requestId, [])
    }
  }

  const handleTypeFilterClick = async (anchor: DOMRect) => {
    const requestId = beginFilterMenuRequest('type', 'Type filters', anchor)
    // Prefer current result set (search/filter) to stay in sync with visible items.
    const optionEntries = entriesForFilterField('type')
    const localSet = new Set<string>()
    for (const e of optionEntries) {
      localSet.add(typeLabel(e))
    }
    if (localSet.size > 0) {
      applyFilterMenuOptions(
        'type',
        requestId,
        Array.from(localSet)
          .sort((a, b) => a.localeCompare(b))
          .map((v) => ({ id: `type:${v}`, label: v || 'unknown' })),
      )
      return
    }

    if (!currentPath) return
    try {
      const values = await invoke<string[]>('list_column_values', {
        path: currentPath,
        column: 'type',
        includeHidden: showHidden,
      })
      applyFilterMenuOptions(
        'type',
        requestId,
        values.map((v: string) => ({ id: `type:${v}`, label: v || 'unknown' })),
      )
    } catch (err) {
      console.error('Failed to load type filters', err)
      applyFilterMenuOptions('type', requestId, [])
    }
  }

  const handleSizeFilterClick = async (anchor: DOMRect) => {
    const requestId = beginFilterMenuRequest('size', 'Size filters', anchor)

    const optionEntries = entriesForFilterField('size')
    const localBuckets = new Map<string, number>()
    for (const e of optionEntries) {
      if (e.kind !== 'file') continue
      if (typeof e.size === 'number') {
        const bucket = sizeBucket(e.size)
        if (bucket) localBuckets.set(bucket.label, bucket.rank)
      }
    }
    if (localBuckets.size > 0) {
      applyFilterMenuOptions(
        'size',
        requestId,
        Array.from(localBuckets.entries())
          .sort((a, b) => a[1] - b[1])
          .map(([v]) => ({ id: `size:${v}`, label: v })),
      )
      return
    }

    if (!currentPath) return
    try {
      const values = await invoke<string[]>('list_column_values', {
        path: currentPath,
        column: 'size',
        includeHidden: showHidden,
      })
      applyFilterMenuOptions(
        'size',
        requestId,
        values.map((v: string) => ({ id: `size:${v}`, label: v })),
      )
    } catch (err) {
      console.error('Failed to load size filters', err)
      applyFilterMenuOptions('size', requestId, [])
    }
  }

  const handleModifiedFilterClick = async (anchor: DOMRect) => {
    const requestId = beginFilterMenuRequest('modified', 'Modified filters', anchor)
    const optionEntries = entriesForFilterField('modified')
    const localBuckets = new Map<string, number>()
    for (const e of optionEntries) {
      const bucket = modifiedBucket(e.modified)
      if (bucket) {
        localBuckets.set(bucket.label, bucket.rank)
      }
    }
    if (localBuckets.size > 0) {
      applyFilterMenuOptions(
        'modified',
        requestId,
        Array.from(localBuckets.entries())
          .sort((a, b) => a[1] - b[1])
          .map(([v]) => ({ id: `modified:${v}`, label: v })),
      )
      return
    }
    if (!currentPath) return
    try {
      const values = await invoke<string[]>('list_column_values', {
        path: currentPath,
        column: 'modified',
        includeHidden: showHidden,
      })
      applyFilterMenuOptions(
        'modified',
        requestId,
        values.map((v: string) => ({ id: `modified:${v}`, label: v })),
      )
    } catch (err) {
      console.error('Failed to load modified filters', err)
      applyFilterMenuOptions('modified', requestId, [])
    }
  }

  const handleToggleNameFilter = (id: string, checked: boolean) => onToggleFilter('name', id, checked)
  const handleToggleTypeFilter = (id: string, checked: boolean) => onToggleFilter('type', id, checked)
  const handleToggleModifiedFilter = (id: string, checked: boolean) =>
    onToggleFilter('modified', id, checked)
  const handleToggleSizeFilter = (id: string, checked: boolean) => onToggleFilter('size', id, checked)

  const closeFilterMenu = () => {
    filterMenuRequestSeq += 1
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
