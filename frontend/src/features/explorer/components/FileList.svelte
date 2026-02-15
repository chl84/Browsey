<script lang="ts">
  import FileListHeader from './FileListHeader.svelte'
  import FileRow from './FileRow.svelte'
  import SelectionBox from '../../../ui/SelectionBox.svelte'
  import FileListColumnFilters from './FileListColumnFilters.svelte'
  import type { Column, Entry, SortDirection, SortField, ListingFacets } from '../types'

  export let cols: Column[] = []
  export let gridTemplate = ''
  export let rowsEl: HTMLDivElement | null = null
  export let headerEl: HTMLDivElement | null = null
  export let loading = false
  export let filterSourceEntries: Entry[] = []
  export let filteredEntries: Entry[] = []
  export let visibleEntries: Entry[] = []
  export let filterValue = ''
  export let columnFilters: {
    name: Set<string>
    type: Set<string>
    modified: Set<string>
    size: Set<string>
  } = { name: new Set(), type: new Set(), modified: new Set(), size: new Set() }
  export let columnFacets: ListingFacets = { name: [], type: [], modified: [], size: [] }
  export let columnFacetsLoading = false
  export let onEnsureColumnFacets: () => void | Promise<void> = () => {}
  export let start = 0
  export let offsetY = 0
  export let totalHeight = 0
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

  let filterControls: {
    handleFilterClick: (field: SortField, anchor: DOMRect) => void
    handleFilterContextMenu: (field: SortField, event: MouseEvent) => void
  } | null = null
  let filterActive: Record<SortField, boolean> = {
    name: false,
    type: false,
    modified: false,
    size: false,
    starred: false,
  }

  const handleFilterClick = (field: SortField, anchor: DOMRect) => {
    filterControls?.handleFilterClick(field, anchor)
  }

  const handleFilterContextMenu = (field: SortField, event: MouseEvent) => {
    filterControls?.handleFilterContextMenu(field, event)
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
      onFilterContextMenu={handleFilterContextMenu}
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

<FileListColumnFilters
  bind:this={filterControls}
  bind:filterActive
  {loading}
  filterValue={filterValue}
  filterSourceEntries={filterSourceEntries}
  {columnFilters}
  {columnFacets}
  columnFacetsLoading={columnFacetsLoading}
  onEnsureColumnFacets={onEnsureColumnFacets}
  onToggleFilter={onToggleFilter}
  onResetFilter={onResetFilter}
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
