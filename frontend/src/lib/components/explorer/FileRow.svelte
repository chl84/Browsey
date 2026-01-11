<script lang="ts">
  import type { Entry } from '../../explorer/types'

  export let entry: Entry
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
</script>

<button
  class="row"
  style={`grid-template-columns:${gridTemplate};`}
  class:hidden={hidden}
  class:selected={selected}
  type="button"
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
      {entry.starred ? '★' : '☆'}
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
    width: 100%;
    text-align: left;
    border-radius: 10px;
    box-shadow: none;
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

  .col-name {
    display: flex;
    align-items: center;
    gap: 10px;
    font-weight: 500;
    color: var(--fg-strong);
    overflow: hidden;
    min-width: 200px;
  }

  .name {
    font-size: 14px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .col-type,
  .col-modified,
  .col-size {
    color: var(--fg-muted);
    font-size: 13px;
    text-align: left;
    min-width: 100px;
  }

  .col-size {
    font-weight: 600;
    text-align: right;
    min-width: 60px;
  }

  .col-modified {
    min-width: 100px;
  }

  .col-star {
    color: var(--fg-muted);
    font-size: 13px;
    text-align: left;
    min-width: 40px;
  }

  .star-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 28px;
    min-height: 28px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: var(--fg-muted);
    padding: 0px;
    cursor: pointer;
    font-size: 16px;
  }

  .star-btn:hover {
    color: var(--fg-strong);
    border-color: var(--border);
    background: var(--bg);
  }

  .star-btn:focus-visible {
    outline: 2px solid var(--border-accent);
  }

  .star-btn.starred {
    color: #f6d04d;
    animation: star-spin 420ms ease;
  }

  @keyframes star-spin {
    from {
      transform: rotate(0deg) scale(0.9);
    }
    50% {
      transform: rotate(180deg) scale(1.1);
    }
    to {
      transform: rotate(360deg) scale(1);
    }
  }

  .icon {
    width: 20px;
    height: 20px;
    object-fit: contain;
    display: inline-block;
  }
</style>
