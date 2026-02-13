<script lang="ts">
  import { tick } from 'svelte'
  import BinarySlider from '../../../ui/BinarySlider.svelte'
  import CheckboxIndicator from '../../../ui/CheckboxIndicator.svelte'

  type TopbarActionId =
    | 'open-settings'
    | 'open-shortcuts'
    | 'search'
    | 'toggle-hidden'
    | 'refresh'
    | 'about'

  export let open = false
  export let x = 0
  export let y = 0
  export let onClose: () => void = () => {}
  export let onSelect: (id: TopbarActionId) => void = () => {}

  let menuEl: HTMLDivElement | null = null
  let posX = 0
  let posY = 0

  // Visual-only state for now; view mode wiring comes later.
  let gridMode = false
  let showHiddenFiles = false

  const actions: Array<{ id: TopbarActionId; label: string; shortcut?: string }> = [
    { id: 'open-settings', label: 'Settings…', shortcut: 'Ctrl+S' },
    { id: 'open-shortcuts', label: 'Keyboard Shortcuts…' },
    { id: 'search', label: 'Search', shortcut: 'Ctrl+F' },
    { id: 'toggle-hidden', label: 'Show Hidden Files' },
    { id: 'refresh', label: 'Refresh' },
    { id: 'about', label: 'About Browsey' },
  ]

  const select = (id: TopbarActionId) => {
    onSelect(id)
  }

  $: {
    if (open) {
      void tick().then(() => {
        if (!menuEl || typeof window === 'undefined') return
        const rect = menuEl.getBoundingClientRect()
        const margin = 8
        const maxX = Math.max(margin, window.innerWidth - rect.width - margin)
        const maxY = Math.max(margin, window.innerHeight - rect.height - margin)
        posX = Math.max(margin, Math.min(x, maxX))
        posY = Math.max(margin, Math.min(y, maxY))
      })
    } else {
      posX = x
      posY = y
    }
  }
</script>

{#if open}
  <div
    class="menu-overlay"
    role="presentation"
    tabindex="-1"
    on:click={onClose}
    on:keydown={(e) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        onClose()
      }
    }}
  >
    <div
      class="menu"
      role="menu"
      aria-label="Main actions"
      bind:this={menuEl}
      style={`top:${posY}px;left:${posX}px;`}
      tabindex="-1"
      on:click|stopPropagation
      on:contextmenu|preventDefault
      on:keydown={(e) => {
        if (e.key === 'Escape') {
          e.preventDefault()
          onClose()
        }
      }}
    >
      <button
        role="menuitem"
        type="button"
        on:click={() => select(actions[0].id)}
        on:keydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            select(actions[0].id)
          }
        }}
      >
        <span class="label">{actions[0].label}</span>
        <span class="shortcut">{actions[0].shortcut}</span>
      </button>

      <button
        role="menuitem"
        type="button"
        on:click={() => select(actions[1].id)}
        on:keydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            select(actions[1].id)
          }
        }}
      >
        <span class="label">{actions[1].label}</span>
      </button>

      <div class="divider" role="separator" aria-hidden="true"></div>

      <button
        role="menuitem"
        type="button"
        on:click={() => select(actions[2].id)}
        on:keydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            select(actions[2].id)
          }
        }}
      >
        <span class="label">{actions[2].label}</span>
        <span class="shortcut">{actions[2].shortcut}</span>
      </button>

      <div class="slider-row" role="group" aria-label="Toggle List/Grid">
        <span class="slider-label">Toggle List/Grid</span>
        <BinarySlider
          checked={gridMode}
          leftLabel="List"
          rightLabel="Grid"
          ariaLabel="Toggle list or grid view"
          onToggle={(next) => {
            gridMode = next
          }}
        />
      </div>

      <button
        class="check-row"
        role="menuitemcheckbox"
        aria-checked={showHiddenFiles}
        type="button"
        on:click={() => {
          showHiddenFiles = !showHiddenFiles
        }}
      >
        <span class="label">{actions[3].label}</span>
        <span class="check-meta">
          <span class="shortcut">{actions[3].shortcut}</span>
          <CheckboxIndicator checked={showHiddenFiles} />
        </span>
      </button>

      <div class="divider" role="separator" aria-hidden="true"></div>

      <button
        role="menuitem"
        type="button"
        on:click={() => select(actions[4].id)}
        on:keydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            select(actions[4].id)
          }
        }}
      >
        <span class="label">{actions[4].label}</span>
      </button>

      <div class="divider" role="separator" aria-hidden="true"></div>

      <button
        role="menuitem"
        type="button"
        on:click={() => select(actions[5].id)}
        on:keydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            select(actions[5].id)
          }
        }}
      >
        <span class="label">{actions[5].label}</span>
      </button>
    </div>
  </div>
{/if}

<style>
  .menu-overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
  }

  .menu {
    position: absolute;
    min-width: var(--ctx-width);
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-xl);
    border-radius: 0;
    display: flex;
    flex-direction: column;
    padding: var(--ctx-padding);
    gap: var(--ctx-gap);
  }

  button {
    width: 100%;
    text-align: left;
    padding: var(--ctx-item-padding-y) var(--ctx-item-padding-x);
    border: 1px solid transparent;
    background: transparent;
    color: var(--fg);
    border-radius: 0;
    font-size: var(--ctx-item-font-size);
    cursor: default;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  button:hover,
  button:focus-visible {
    background: var(--bg-raised);
    border-color: var(--border);
    outline: none;
  }

  .label {
    min-width: 0;
  }

  .shortcut {
    color: var(--fg-muted);
    font-size: var(--ctx-shortcut-font-size);
  }

  .divider {
    display: block;
    height: 1px;
    width: 100%;
    background: var(--border);
    margin: var(--ctx-divider-margin) 0;
  }

  .slider-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: var(--ctx-item-padding-y) var(--ctx-item-padding-x);
    border: 1px solid transparent;
  }

  .slider-row:hover {
    background: var(--bg-raised);
    border-color: var(--border);
  }

  .slider-label {
    color: var(--fg);
    font-size: var(--ctx-item-font-size);
    line-height: 1.2;
  }

  .check-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: var(--ctx-item-padding-y) var(--ctx-item-padding-x);
    border: 1px solid transparent;
    cursor: default;
  }

  .check-row:hover,
  .check-row:focus-visible {
    background: var(--bg-raised);
    border-color: var(--border);
    outline: none;
  }

  .check-meta {
    display: inline-flex;
    align-items: center;
    gap: 10px;
  }

  .check-row:focus-visible :global(.checkbox-indicator) {
    border-color: var(--border-accent-strong);
  }
</style>
