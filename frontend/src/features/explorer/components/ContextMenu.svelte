<script lang="ts">
  import { tick } from 'svelte'
  type ContextAction = { id: string; label: string; shortcut?: string; dangerous?: boolean }

  export let open = false
  export let x = 0
  export let y = 0
  export let actions: ContextAction[] = []
  export let onSelect: (id: string) => void = () => {}
  export let onClose: () => void = () => {}

  let menuEl: HTMLDivElement | null = null
  let posX = 0
  let posY = 0

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
      aria-label="Context menu"
      bind:this={menuEl}
      style={`top:${posY}px;left:${posX}px;`}
      tabindex="-1"
      on:keydown={(e) => {
        if (e.key === 'Escape') {
          e.preventDefault()
          onClose()
        }
      }}
      on:click|stopPropagation
      on:contextmenu|preventDefault
    >
      {#each actions as action}
        {#if action.id.startsWith('divider')}
          <div class="divider" role="separator" aria-hidden="true"></div>
        {:else}
          <button
            role="menuitem"
            class:dangerous={action.dangerous}
            on:click={() => onSelect(action.id)}
            on:keydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') {
                e.preventDefault()
                onSelect(action.id)
              }
            }}
            type="button"
          >
            <span class="label">{action.label}</span>
            {#if action.shortcut}
              <span class="shortcut">{action.shortcut}</span>
            {/if}
          </button>
        {/if}
      {/each}
    </div>
  </div>
{/if}

<style>
  .menu-overlay {
    position: fixed;
    inset: 0;
    z-index: 20;
  }

  .menu {
    position: absolute;
    min-width: 180px;
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: var(--shadow-xl);
    border-radius: 0;
    display: flex;
    flex-direction: column;
    padding: 6px;
    gap: 4px;
  }

  button {
    width: 100%;
    text-align: left;
    padding: 8px 10px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--fg);
    border-radius: 0;
    font-size: 14px;
    cursor: pointer;
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

  button.dangerous {
    color: var(--accent-danger);
  }

  button.dangerous:hover,
  button.dangerous:focus-visible {
    background: var(--bg-hover);
    border-color: var(--border);
  }

  .divider {
    display: block;
    height: 1px;
    width: 100%;
    background: var(--border);
    margin: 4px 0;
  }

  .label {
    flex: 1;
  }

  .shortcut {
    color: var(--fg-muted);
    font-size: 12px;
  }
</style>
