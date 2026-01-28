<script lang="ts">
  import { iconPath } from '../utils'

  export let bookmarks: { label: string; path: string }[] = []
  export let onSelect: (path: string) => void = () => {}
  export let onRemove: (path: string) => void = () => {}

const bookmarkIcon = iconPath('browsey/bookmark.svg')
</script>

<div class="section">
  <div class="section-title">Bookmarks</div>
  {#each bookmarks as mark}
    <div
      class="nav bookmark"
      role="button"
      tabindex="0"
      on:click={() => onSelect(mark.path)}
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
    padding: 5px 12px 5px 22px;
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

  .nav.bookmark {
    position: relative;
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

  .nav-icon {
    width: 18px;
    height: 18px;
    object-fit: contain;
    flex-shrink: 0;
  }

  .remove-bookmark {
    margin-left: auto;
    background: transparent;
    border: none;
    color: var(--fg-muted);
    font-size: var(--font-size-base);
    font-weight: var(--font-weight-base);
    cursor: pointer;
    opacity: 0;
    transition: opacity 120ms ease;
    padding: 0 4px;
    line-height: 1;
  }

  .nav.bookmark:hover .remove-bookmark {
    opacity: 1;
  }
</style>
