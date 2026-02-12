<script lang="ts">
  import FileListHeader from './FileListHeader.svelte'
  import FileRow from './FileRow.svelte'
  import SelectionBox from '../../../ui/SelectionBox.svelte'
  import ColumnFilterMenu from './ColumnFilterMenu.svelte'
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
  let filterMenuTitle = 'Filters'
  let filterMenuOptions: FilterOption[] = nameFilterOptions
  let activeNameFilters: Set<string> = new Set()
  let activeTypeFilters: Set<string> = new Set()

  const handleFilterClick = (field: SortField, anchor: DOMRect) => {
    if (field === 'name') {
      filterMenuOpen = true
      filterMenuAnchor = anchor
      filterMenuTitle = 'Name filters'
      filterMenuOptions = nameFilterOptions
      return
    }
    if (field === 'type') {
      void handleTypeFilterClick(anchor)
      return
    }
  }

  const handleTypeFilterClick = async (anchor: DOMRect) => {
    if (!currentPath) return
    filterMenuOpen = true
    filterMenuAnchor = anchor
    filterMenuTitle = 'Type filters'
    try {
      const values = await invoke<string[]>('list_column_values', { path: currentPath, column: 'type' })
      filterMenuOptions = values.map((v: string) => ({ id: `type:${v}`, label: v || 'Unknown' }))
    } catch (err) {
      console.error('Failed to load type filters', err)
      filterMenuOptions = []
    }
  }

  const handleToggleNameFilter = (id: string, checked: boolean) => {
    const next = new Set(activeNameFilters)
    if (checked) {
      next.add(id)
    } else {
      next.delete(id)
    }
    activeNameFilters = next
  }

  const handleToggleTypeFilter = (id: string, checked: boolean) => {
    const next = new Set(activeTypeFilters)
    if (checked) {
      next.add(id)
    } else {
      next.delete(id)
    }
    activeTypeFilters = next
  }

  const closeFilterMenu = () => {
    filterMenuOpen = false
  }

  const isFilterActive = (field: SortField) => {
    if (field === 'name') return activeNameFilters.size > 0
    if (field === 'type') return activeTypeFilters.size > 0
    return false
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
      isFilterActive={isFilterActive}
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
  selected={activeNameFilters}
  anchor={filterMenuAnchor}
  onToggle={(id, checked) => {
    if (id.startsWith('name:')) return handleToggleNameFilter(id, checked)
    if (id.startsWith('type:')) return handleToggleTypeFilter(id, checked)
  }}
  onClose={closeFilterMenu}
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
