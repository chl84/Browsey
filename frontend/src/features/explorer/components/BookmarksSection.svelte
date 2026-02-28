<script lang="ts">
  import { iconPath } from '../utils'

  export let bookmarks: { label: string; path: string }[] = []
  export let onSelect: (path: string) => void = () => {}
  export let onRemove: (path: string) => void = () => {}
  export let dragTargetPath: string | null = null
  export let onDragOver: (path: string, e: DragEvent) => void = () => {}
  export let onDragLeave: (path: string, e: DragEvent) => void = () => {}
  export let onDrop: (path: string, e: DragEvent) => void = () => {}

const bookmarkIcon = iconPath('browsey/bookmark.svg')
</script>

<div class="section">
  <div class="section-title">Bookmarks</div>
  {#each bookmarks as mark}
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
