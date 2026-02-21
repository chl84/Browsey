<script lang="ts">
  import { tick } from 'svelte'
  import type { ContextAction } from '../context/createContextMenus'

  export let open = false
  export let x = 0
  export let y = 0
  export let actions: ContextAction[] = []
  export let onSelect: (id: string) => void = () => {}
  export let onClose: () => void = () => {}

  let menuEl: HTMLDivElement | null = null
  let submenuEl: HTMLDivElement | null = null
  let posX = 0
  let posY = 0
  let submenuX = 0
  let submenuY = 0
  let submenuAnchor: DOMRect | null = null
  let submenuParentId: string | null = null
  let submenuActions: ContextAction[] = []
  let submenuCloseTimer: ReturnType<typeof setTimeout> | null = null

  const hasChildren = (action: ContextAction) =>
    Array.isArray(action.children) && action.children.length > 0

  const clearSubmenuCloseTimer = () => {
    if (!submenuCloseTimer) return
    clearTimeout(submenuCloseTimer)
    submenuCloseTimer = null
  }

  const closeSubmenu = () => {
    clearSubmenuCloseTimer()
    submenuParentId = null
    submenuActions = []
    submenuAnchor = null
  }

  const scheduleSubmenuClose = () => {
    clearSubmenuCloseTimer()
    submenuCloseTimer = setTimeout(() => {
      closeSubmenu()
    }, 120)
  }

  const positionSubmenu = () => {
    if (!submenuAnchor || !submenuEl || typeof window === 'undefined') return
    const margin = 8
    const rect = submenuEl.getBoundingClientRect()
    let nextX = submenuAnchor.right + 4
    if (nextX + rect.width + margin > window.innerWidth) {
      nextX = submenuAnchor.left - rect.width - 4
    }
    let nextY = submenuAnchor.top
    if (nextY + rect.height + margin > window.innerHeight) {
      nextY = window.innerHeight - rect.height - margin
    }
    submenuX = Math.max(margin, Math.min(nextX, window.innerWidth - rect.width - margin))
    submenuY = Math.max(margin, nextY)
  }

  const openSubmenu = (action: ContextAction, trigger: HTMLButtonElement) => {
    if (!hasChildren(action)) {
      closeSubmenu()
      return
    }
    clearSubmenuCloseTimer()
    submenuParentId = action.id
    submenuActions = action.children ?? []
    submenuAnchor = trigger.getBoundingClientRect()
    submenuX = submenuAnchor.right + 4
    submenuY = submenuAnchor.top
    void tick().then(positionSubmenu)
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

  $: if (!open) {
    closeSubmenu()
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
      on:mouseleave={scheduleSubmenuClose}
    >
      {#each actions as action}
        {#if action.id.startsWith('divider')}
          <div class="divider" role="separator" aria-hidden="true"></div>
        {:else}
          {#if hasChildren(action)}
            <button
              role="menuitem"
              aria-haspopup="menu"
              aria-expanded={submenuParentId === action.id}
              class:dangerous={action.dangerous}
              class:submenu-open={submenuParentId === action.id}
              on:mouseenter={(e) => openSubmenu(action, e.currentTarget as HTMLButtonElement)}
              on:mouseleave={scheduleSubmenuClose}
              on:click={(e) => {
                e.preventDefault()
                openSubmenu(action, e.currentTarget as HTMLButtonElement)
              }}
              on:keydown={(e) => {
                if (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowRight') {
                  e.preventDefault()
                  openSubmenu(action, e.currentTarget as HTMLButtonElement)
                } else if (e.key === 'ArrowLeft') {
                  e.preventDefault()
                  closeSubmenu()
                }
              }}
              type="button"
            >
              <span class="label">{action.label}</span>
              <span class="submenu-arrow" aria-hidden="true">›</span>
            </button>
          {:else}
            <button
              role="menuitem"
              class:dangerous={action.dangerous}
              on:mouseenter={closeSubmenu}
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
        {/if}
      {/each}
    </div>
    {#if submenuActions.length > 0}
      <div
        class="menu submenu"
        role="menu"
        aria-label="Context submenu"
        bind:this={submenuEl}
        style={`top:${submenuY}px;left:${submenuX}px;`}
        tabindex="-1"
        on:mouseenter={clearSubmenuCloseTimer}
        on:mouseleave={scheduleSubmenuClose}
        on:click|stopPropagation
        on:keydown={(e) => {
          if (e.key === 'Escape') {
            e.preventDefault()
            onClose()
          }
        }}
        on:contextmenu|preventDefault
      >
        {#each submenuActions as action}
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
                } else if (e.key === 'ArrowLeft') {
                  e.preventDefault()
                  closeSubmenu()
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
    {/if}
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
    margin: var(--ctx-divider-margin) 0;
  }

  .submenu {
    z-index: 101;
  }

  .submenu-arrow {
    color: var(--fg-muted);
    font-size: var(--ctx-shortcut-font-size);
  }

  button.submenu-open {
    background: var(--bg-raised);
    border-color: var(--border);
  }

  .label {
    flex: 1;
  }

  .shortcut {
    color: var(--fg-muted);
    font-size: var(--ctx-shortcut-font-size);
  }
</style>
