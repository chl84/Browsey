<script lang="ts">
  import SelectionBox from '../../../ui/SelectionBox.svelte'
  import { iconPath } from '../utils'
  import type { Entry } from '../types'

  export let entries: Entry[] = []
  export let visibleEntries: Entry[] = []
  export let start = 0
  export let offsetY = 0
  export let totalHeight = 0
  export let rowsEl: HTMLDivElement | null = null
  export let selected: Set<string> = new Set()
  export let clipboardMode: 'copy' | 'cut' = 'copy'
  export let clipboardPaths: Set<string> = new Set()
  export let isHidden: (entry: Entry) => boolean = () => false
  export let displayName: (entry: Entry) => string = (e) => e.name
  export let onWheel: (event: WheelEvent) => void = () => {}
  export let selectionActive = false
  export let selectionRect: { x: number; y: number; width: number; height: number } = {
    x: 0,
    y: 0,
    width: 0,
    height: 0,
  }
  export let onOpen: (entry: Entry) => void = () => {}
  export let onRowClick: (entry: Entry, index: number, event: MouseEvent) => void = () => {}
  export let onContextMenu: (entry: Entry, event: MouseEvent) => void = () => {}
  export let onRowDragStart: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragEnd: (event: DragEvent) => void = () => {}
  export let onRowDragEnter: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragOver: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDrop: (entry: Entry, event: DragEvent) => void = () => {}
  export let onRowDragLeave: (entry: Entry, event: DragEvent) => void = () => {}
  export let dragTargetPath: string | null = null
  export let dragAllowed = false
  export let onRowsContextMenu: (event: MouseEvent) => void = () => {}
  export let onRowsClick: (event: MouseEvent) => void = () => {}
  export let onRowsMousedown: (event: MouseEvent) => void = () => {}
  export let onRowsScroll: (event: Event) => void = () => {}
  export let onRowsKeydown: (event: KeyboardEvent) => void = () => {}

  const readOnlyIcon = iconPath('status/eye-svgrepo-com.svg')
  const lockIcon = iconPath('status/padlock.svg')
</script>

<section class="grid-container">
  <div
    class="grid"
    role="grid"
    tabindex="0"
    bind:this={rowsEl}
    style="user-select:none"
    on:scroll={onRowsScroll}
    on:wheel|passive={onWheel}
    on:contextmenu={onRowsContextMenu}
    on:click={onRowsClick}
    on:mousedown={onRowsMousedown}
    on:keydown={onRowsKeydown}
  >
    {#if entries.length === 0}
      <div class="muted">No items here.</div>
    {:else}
      <div class="spacer" style={`height:${totalHeight}px`}>
        <div class="grid-viewport" style={`transform: translateY(${offsetY}px);`}>
          {#each visibleEntries as entry, i (`${entry.path}-${i}`)}
            <button
              class="card"
              class:selected={selected.has(entry.path)}
              class:cut={clipboardMode === 'cut' && clipboardPaths.has(entry.path)}
              class:hidden={isHidden(entry)}
              class:drop-target={dragTargetPath === entry.path}
              class:drop-blocked={dragTargetPath === entry.path && !dragAllowed}
              type="button"
              style="user-select:text"
              data-path={entry.path}
              draggable="true"
              on:dblclick={() => onOpen(entry)}
              on:click={(event) => onRowClick(entry, start + i, event)}
              on:contextmenu={(event) => {
                event.preventDefault()
                event.stopPropagation()
                onContextMenu(entry, event)
              }}
              on:dragstart={(event) => onRowDragStart(entry, event)}
              on:dragend={onRowDragEnd}
              on:dragenter|preventDefault={(event) => onRowDragEnter(entry, event)}
              on:dragover|preventDefault={(event) => onRowDragOver(entry, event)}
              on:dragleave={(event) => onRowDragLeave(entry, event)}
              on:drop|preventDefault={(event) => onRowDrop(entry, event)}
            >
              <img class="icon" src={entry.icon} alt="" draggable="false" />
              <div class="name" title={entry.name}>
                {displayName(entry)}
                {#if entry.readDenied}
                  <img class="ro-icon" src={lockIcon} alt="No read permission" title="No read permission" />
                {/if}
                {#if entry.readOnly}
                  <img class="ro-icon" src={readOnlyIcon} alt="Read-only" title="Read-only" />
                {/if}
              </div>
            </button>
          {/each}
        </div>
      </div>
    {/if}
    <SelectionBox active={selectionActive} rect={selectionRect} />
  </div>
</section>

<style>
  .grid-container {
    --grid-card-width: 128px;
    --grid-card-height: 136px;
    --grid-gap: 6px;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0;
  }

  .grid {
    display: block;
    height: 100%;
    overflow-y: auto;
    overflow-x: hidden;
    min-height: 0;
    position: relative;
    user-select: none;
    cursor: default;
    padding: 12px;
    box-sizing: border-box;
  }

  .grid:focus,
  .grid:focus-visible {
    outline: none;
  }

  .spacer {
    position: relative;
    width: 100%;
  }

  .grid-viewport {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, var(--grid-card-width));
    grid-auto-rows: var(--grid-card-height);
    gap: var(--grid-gap);
    justify-content: start;
    align-content: start;
    width: 100%;
  }

  .muted {
    color: var(--fg-muted);
    grid-column: 1 / -1;
    text-align: center;
    padding: 20px 0;
  }

  .card {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 12px 10px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: 0;
    text-align: center;
    cursor: default;
    color: var(--fg);
    font-size: 14px;
    font-weight: 300;
    position: relative;
    overflow: hidden;
    transition: background 120ms ease, border-color 120ms ease;
  }

  .card:hover {
    background: var(--bg-hover);
  }

  .card.selected {
    background: var(--bg-raised);
    border-color: transparent;
  }

  .card.cut {
    opacity: 0.55;
  }

  .card.drop-target {
    background: var(--drop-allowed-bg);
    border-color: var(--drop-allowed-border);
    box-shadow: var(--drop-allowed-shadow);
  }

  .card.drop-target.drop-blocked {
    background: var(--drop-blocked-bg);
    border-color: var(--drop-blocked-border);
    box-shadow: var(--drop-blocked-shadow);
  }

  .card.hidden {
    opacity: 0.55;
  }

  .card:focus-visible {
    outline: none;
  }

  .icon {
    width: 48px;
    height: 48px;
    object-fit: contain;
    display: block;
  }

  .name {
    font-weight: 600;
    color: var(--fg-strong);
    word-break: break-word;
    line-height: 1.3;
    text-align: center;
    display: -webkit-box;
    display: box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    box-orient: vertical;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
    width: 100%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 10px;
  }

  .ro-icon {
    width: 18px;
    height: 18px;
    opacity: 0.5;
    flex-shrink: 0;
  }
</style>
