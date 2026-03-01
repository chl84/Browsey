<script lang="ts">
  import { tick } from 'svelte'
  import { iconPath } from '../utils'
  import TextField from '../../../shared/ui/TextField.svelte'

  export let bookmarks: { label: string; path: string }[] = []
  export let onSelect: (path: string) => void = () => {}
  export let onRemove: (path: string) => void = () => {}
  export let dragTargetPath: string | null = null
  export let onDragOver: (path: string, e: DragEvent) => void = () => {}
  export let onDragLeave: (path: string, e: DragEvent) => void = () => {}
  export let onDrop: (path: string, e: DragEvent) => void = () => {}

  const bookmarkIcon = iconPath('browsey/bookmark.svg')
  const normalize = (value: string) => value.trim().toLowerCase()
  const matches = (query: string, mark: { label: string; path: string }) =>
    !query || mark.label.toLowerCase().includes(query) || mark.path.toLowerCase().includes(query)

  let bookmarkFilter = ''
  let showFilter = false
  let filterElement: HTMLInputElement | null = null

  $: normalizedBookmarkFilter = normalize(bookmarkFilter)
  $: filteredBookmarks = bookmarks.filter((mark) => matches(normalizedBookmarkFilter, mark))
  $: hasFilter = normalizedBookmarkFilter.length > 0

  async function toggleFilter() {
    showFilter = !showFilter
    if (showFilter) {
      await tick()
      filterElement?.focus()
      return
    }
    bookmarkFilter = ''
  }
</script>

<div class="section">
  <div class="section-header">
    <div class="section-title">Bookmarks</div>
    <button
      type="button"
      class="filter-toggle"
      class:active={showFilter}
      aria-label={showFilter ? 'Hide bookmark filter' : 'Show bookmark filter'}
      aria-pressed={showFilter}
      on:click={toggleFilter}
    >
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="7" cy="7" r="4.5" />
        <path d="M10.5 10.5L14 14" />
      </svg>
    </button>
  </div>

  {#if showFilter}
    <div class="filter-wrap">
      <TextField
        type="search"
        variant="sidebar"
        className="bookmark-filter"
        placeholder="Filter bookmarks"
        aria-label="Filter bookmarks"
        bind:value={bookmarkFilter}
        bind:element={filterElement}
      />
    </div>
  {/if}

  {#if showFilter && hasFilter && filteredBookmarks.length === 0}
    <div class="empty">No bookmarks match</div>
  {/if}

  {#each filteredBookmarks as mark}
    <div
      class="nav bookmark"
      class:drop-target={dragTargetPath === mark.path}
      role="button"
      tabindex="0"
      on:click={() => onSelect(mark.path)}
      on:dragenter|preventDefault={(e) => onDragOver(mark.path, e)}
      on:dragover|preventDefault={(e) => onDragOver(mark.path, e)}
      on:dragleave={(e) => onDragLeave(mark.path, e)}
      on:drop|preventDefault={(e) => onDrop(mark.path, e)}
      on:keydown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onSelect(mark.path)
        }
      }}
    >
      <img class="nav-icon" src={bookmarkIcon} alt="" />
      <span class="nav-label">{mark.label}</span>
      <span
        class="remove-bookmark"
        role="button"
        tabindex="0"
        aria-label="Remove bookmark"
        on:click={(e) => {
          e.stopPropagation()
          onRemove(mark.path)
        }}
        on:keydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            e.stopPropagation()
            onRemove(mark.path)
          }
        }}
      >
        ×
      </span>
    </div>
  {/each}
</div>

<style>
  .section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .section-title {
    color: var(--fg-muted);
    font-size: 12px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    font-weight: 700;
    padding-left: 10px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .filter-toggle {
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: var(--fg-muted);
    padding: 0;
    margin-right: 6px;
    cursor: default;
  }

  .filter-toggle:hover,
  .filter-toggle.active {
    color: var(--fg);
    background: var(--bg-hover);
  }

  .filter-toggle:focus,
  .filter-toggle:focus-visible {
    outline: none;
  }

  .filter-toggle svg {
    width: 16px;
    height: 16px;
    stroke: currentColor;
    stroke-width: 1.5;
    fill: none;
    stroke-linecap: square;
  }

  .filter-wrap {
    padding: 2px 6px 0;
  }

  .empty {
    color: var(--fg-dim);
    font-size: var(--font-size-small);
    padding: 0 10px;
  }

  .nav {
    border: none;
    border-radius: 0;
    padding: calc((var(--sidebar-row-height) - 18px) / 2) 22px;
    background: transparent;
    color: var(--fg);
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-base);
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 8px;
    cursor: default;
    transition: background 120ms ease;
    transform: none;
    box-shadow: none;
  }

  .nav:hover {
    background: var(--bg-hover);
    transform: none;
    box-shadow: none;
  }

  .nav:active {
    transform: none;
    box-shadow: none;
  }

  .nav:focus,
  .nav:focus-visible {
    outline: none;
  }

  .nav.bookmark {
    position: relative;
  }

  .nav.bookmark.drop-target {
    background: var(--bg-hover);
    outline: 1px solid var(--border-accent);
    outline-offset: -1px;
  }

  .nav-icon {
    width: 18px;
    height: 18px;
    object-fit: contain;
    flex-shrink: 0;
  }

  .remove-bookmark {
    position: absolute;
    top: 50%;
    right: 2px;
    transform: translateY(-50%);
    background: transparent;
    border: none;
    color: var(--fg-muted);
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-base);
    cursor: pointer;
    opacity: 0;
    transition: opacity 120ms ease;
    padding: 2px 6px;
    line-height: 1;
  }

  .nav.bookmark:hover .remove-bookmark {
    opacity: 1;
  }
</style>
