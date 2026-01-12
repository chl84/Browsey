<script lang="ts">
  import FileListHeader from './FileListHeader.svelte'
  import FileRow from './FileRow.svelte'
  import type { Column, Entry, SortDirection, SortField } from '../../explorer/types'

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
  export let wide = false
  export let selected: Set<string> = new Set()
  export let sortField: SortField = 'name'
  export let sortDirection: SortDirection = 'asc'
  export let isHidden: (entry: Entry) => boolean = () => false
  export let displayName: (entry: Entry) => string = (e) => e.name
  export let formatSize: (size?: number | null) => string = () => ''
  export let formatItems: (count?: number | null) => string = () => ''

  export let onRowsScroll: (event: Event) => void = () => {}
  export let onWheel: (event: WheelEvent) => void = () => {}
  export let onRowsKeydown: (event: KeyboardEvent) => void = () => {}
  export let onRowsClick: (event: MouseEvent) => void = () => {}
  export let onRowsContextMenu: (event: MouseEvent) => void = () => {}
  export let onChangeSort: (field: SortField) => void = () => {}
  export let onStartResize: (index: number, event: PointerEvent) => void = () => {}
  export let ariaSort: (field: SortField) => 'ascending' | 'descending' | 'none' = () => 'none'
  export let onRowClick: (entry: Entry, absoluteIndex: number, event: MouseEvent) => void = () => {}
  export let onOpen: (entry: Entry) => void = () => {}
  export let onToggleStar: (entry: Entry) => void = () => {}
  export let onContextMenu: (entry: Entry, event: MouseEvent) => void = () => {}
</script>

<section class="list" class:wide={wide}>
  <div
    class="rows"
    bind:this={rowsEl}
    on:scroll={onRowsScroll}
    on:wheel|passive={onWheel}
    on:keydown={onRowsKeydown}
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
    />
    {#if !loading && filteredEntries.length === 0}
      <div class="muted">No items here.</div>
    {:else}
      <div class="spacer" style={`height:${totalHeight}px`}>
        <div class="row-viewport" style={`transform: translateY(${offsetY}px)`}>
          {#each visibleEntries as entry, i (entry.path)}
            <FileRow
              {entry}
              gridTemplate={gridTemplate}
              hidden={isHidden(entry)}
              selected={selected.has(entry.path)}
              displayName={displayName}
              {formatSize}
              {formatItems}
              onOpen={onOpen}
              onClick={(event) => onRowClick(entry, start + i, event)}
              onToggleStar={onToggleStar}
              onContextMenu={(event) => onContextMenu(entry, event)}
            />
          {/each}
        </div>
      </div>
    {/if}
  </div>
</section>

<style>
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
    direction: ltr;
    padding-left: 12px;
    padding-right: 12px;
    padding-bottom: 32px;
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
