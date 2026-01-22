<script lang="ts">
  import { iconPath } from '../utils'
  import type { Entry } from '../types'

  const readOnlyIcon = iconPath('status/eye-svgrepo-com.svg')

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
</script>

<button
  class="row"
  data-index={index}
  style={`grid-template-columns:${gridTemplate};`}
  style:webkitUserSelect="text"
  class:hidden={hidden}
  class:selected={selected}
  class:cut={cutting}
  class:drop-target={dropActive}
  class:drop-blocked={dropActive && !dropAllowed}
  type="button"
  draggable="true"
  style:userSelect="text"
  on:dblclick={() => onOpen(entry)}
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
    <img class="icon" src={entry.icon} alt="" />
    <span class="name">
      {displayName(entry)}
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
  <div class="col-modified">{entry.modified ?? '—'}</div>
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
    gap: 10px;
    align-items: center;
    padding: 0 12px;
    height: 32px;
    min-height: 32px;
    transition: background 120ms ease, border-color 120ms ease;
    cursor: default;
    box-sizing: border-box;
    border: 1px solid transparent;
    background: transparent;
    width: max-content;
    text-align: left;
    border-radius: 0;
    box-shadow: none;
    font-size: 14px;
    font-weight: 300;
  }

  .row:hover {
    background: var(--bg-hover);
    transform: none !important;
    box-shadow: none !important;
  }

  .row.hidden {
    opacity: 0.55;
  }

  .row.selected {
    background: var(--bg-raised);
    border-color: transparent;
  }

  .row.cut {
    opacity: 0.55;
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

  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .ro-icon {
    width: 14px;
    height: 14px;
    opacity: 0.8;
    flex-shrink: 0;
  }

  .col-type,
  .col-modified,
  .col-size {
    color: var(--fg-muted);
    text-align: left;
    min-width: 100px;
  }

  .col-size {
    font-weight: 300;
    text-align: right;
    min-width: 60px;
  }

  .col-modified {
    min-width: 100px;
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
    background: transparent;
    color: var(--fg-muted);
    padding: 0px;
    cursor: pointer;
    font-size: 16px;
    line-height: 1;
    transform-origin: center center;
  }

  .star-glyph {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    transform-origin: center center;
  }

  .star-btn:hover {
    color: var(--fg-strong);
  }

  .star-btn:focus-visible {
    outline: 2px solid var(--border-accent);
  }

  .star-btn.starred {
    color: var(--accent-warning);
  }

  .icon {
    width: 20px;
    height: 20px;
    object-fit: contain;
    display: inline-block;
  }
</style>
