<script lang="ts">
  import { iconPath as assetIconPath } from '../utils'
  import { iconPath as iconPathById } from '../icons'
  import type { Entry } from '../types'

  const readOnlyIcon = assetIconPath('status/eye-svgrepo-com.svg')
  const lockIcon = assetIconPath('status/padlock.svg')

  export let entry: Entry
  export let index = 0
  export let gridTemplate = ''
  export let hidden = false
  export let selected = false
  export let displayName: (entry: Entry) => string = (e) => e.name
  export let formatSize: (size?: number | null) => string = () => ''
  export let formatItems: (count?: number | null) => string = () => ''
  export let onOpen: (entry: Entry) => void = () => {}
  export let onClick: (event: MouseEvent) => void = () => {}
  export let onToggleStar: (entry: Entry) => void = () => {}
  export let onContextMenu: (event: MouseEvent) => void = () => {}
  export let onDragStart: (event: DragEvent) => void = () => {}
  export let onDragEnd: (event: DragEvent) => void = () => {}
  export let onDragOverRow: (event: DragEvent) => void = () => {}
  export let onDragEnterRow: (event: DragEvent) => void = () => {}
  export let onDropRow: (event: DragEvent) => void = () => {}
  export let onDragLeaveRow: (event: DragEvent) => void = () => {}
  export let dropActive = false
  export let dropAllowed = false
  export let cutting = false
  export let dragging = false
</script>

<button
  class="row"
  class:dragging={dragging}
  data-index={index}
  style={`grid-template-columns:${gridTemplate};`}
  class:hidden={hidden}
  class:selected={selected}
  class:cut={cutting}
  class:drop-target={dropActive}
  class:drop-blocked={dropActive && !dropAllowed}
  type="button"
  draggable="true"
  on:keydown={(e) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      e.stopPropagation()
      onOpen(entry)
    }
  }}
  on:click={onClick}
  on:contextmenu={(event) => {
    event.preventDefault()
    event.stopPropagation()
    const target = event.currentTarget as HTMLElement
    target.focus()
    onContextMenu(event)
  }}
  on:dragstart={onDragStart}
  on:dragend={onDragEnd}
  on:dragenter|preventDefault={onDragEnterRow}
  on:dragover|preventDefault={onDragOverRow}
  on:dragleave={onDragLeaveRow}
  on:drop|preventDefault={onDropRow}
>
  <div class="col-name">
    <img class="icon" src={iconPathById(entry.iconId)} alt="" />
    <span class="name">
      {displayName(entry)}
      {#if entry.readDenied}
        <img class="ro-icon" src={lockIcon} alt="No read permission" title="No read permission" />
      {/if}
      {#if entry.readOnly}
        <img class="ro-icon" src={readOnlyIcon} alt="Read-only" title="Read-only" />
      {/if}
    </span>
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
  <div class="col-modified">
    {#if entry.modified}
      {#if entry.modified.includes(' ')}
        <span class="mod-date">{entry.modified.split(' ')[0]}</span>
        <span class="mod-time">{entry.modified.slice(entry.modified.indexOf(' ') + 1)}</span>
      {:else}
        {entry.modified}
      {/if}
    {:else}
      —
    {/if}
  </div>
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
        onToggleStar(entry)
      }}
      on:keydown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          e.stopPropagation()
          onToggleStar(entry)
        }
      }}
    >
      <span class="star-glyph">{entry.starred ? '★' : '☆'}</span>
    </span>
  </div>
</button>

<style>
  .row {
    display: grid;
    gap: var(--list-header-gap);
    align-items: center;
    padding:
      var(--row-padding-y)
      var(--list-header-padding-right)
      var(--row-padding-y)
      var(--list-header-padding-left);
    height: var(--row-height);
    min-height: var(--row-height);
    transition: none;
    cursor: default;
    box-sizing: border-box;
    border: 1px solid transparent;
    background: transparent;
    width: max-content;
    text-align: left;
    border-radius: 0;
    box-shadow: none;
    font-size: var(--row-font-size, var(--font-size-base));
    font-weight: var(--font-weight-base);
  }

  .row:hover {
    background: var(--bg-hover);
    transform: none !important;
    box-shadow: none !important;
    z-index: 1;
  }

  :global(.is-scrolling) .row:hover:not(.selected):not(.drop-target) {
    background: transparent;
    z-index: auto;
  }

  .row.hidden {
    opacity: 0.55;
  }

  .row.selected {
    background: var(--selection-fill);
    border-color: transparent;
    box-shadow: none;
  }

  .row.cut {
    opacity: 0.55;
  }

  .row.dragging * {
    pointer-events: none;
  }

  .row.dragging {
    pointer-events: auto;
  }

  .row.drop-target {
    background: var(--drop-allowed-bg);
    border-color: var(--drop-allowed-border);
    box-shadow: none;
    position: relative;
  }

  .row.drop-target.drop-blocked {
    background: var(--drop-blocked-bg);
    border-color: var(--drop-blocked-border);
    box-shadow: none;
  }

  .row:focus-visible {
    outline: none;
  }

  .col-name {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--fg-strong);
    overflow: hidden;
    min-width: 200px;
  }

  .col-name .icon {
    width: var(--list-icon-size, var(--icon-size));
    height: var(--list-icon-size, var(--icon-size));
    object-fit: contain;
  }

  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  .ro-icon {
    width: 18px;
    height: 18px;
    opacity: 0.5;
    flex-shrink: 0;
  }

  /* pull lock + eye closer together */
  .name .ro-icon + .ro-icon {
    margin-left: -4px;
  }

  .col-type,
  .col-modified,
  .col-size {
    color: var(--fg-muted);
    text-align: left;
    min-width: 100px;
  }

  .col-size {
    font-weight: var(--font-weight-base);
    text-align: right;
    min-width: 60px;
  }

  .col-modified {
    min-width: 100px;
    display: flex;
    align-items: center;
    gap: 4px;
    justify-content: space-between;
  }

  .mod-time {
    font-size: var(--font-size-small);
    line-height: 1.2;
    text-align: right;
    flex-shrink: 0;
  }

  .col-star {
    color: var(--fg-muted);
    text-align: center;
    min-width: 25px;
    display: flex;
    justify-content: center;
  }

  .star-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 0;
    background: transparent;
    color: var(--fg-muted);
    padding: 0px;
    cursor: pointer;
    font-size: 16px;
    line-height: 1;
    transform-origin: center center;
    transition: color 240ms ease, transform 140ms ease;
  }

  .star-glyph {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    transform-origin: center center;
    opacity: 0.55;
    transition: opacity 240ms ease;
  }

  .star-btn:hover {
    color: var(--fg-strong);
  }

  .star-btn:focus-visible {
    outline: var(--focus-ring-width) solid var(--focus-ring-color);
    outline-offset: var(--focus-ring-offset);
    border-radius: 0;
  }

  .star-btn.starred {
    color: var(--accent-warning);
  }

  .star-btn.starred .star-glyph {
    opacity: 1;
  }

  .icon {
    width: 20px;
    height: 20px;
    object-fit: contain;
    display: inline-block;
  }
</style>
