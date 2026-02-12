<script lang="ts">
  import type { FilterOption } from '../types'

  export let open = false
  export let options: FilterOption[] = []
  export let selected: Set<string> = new Set()
  export let anchor: DOMRect | null = null

  export let onToggle: (id: string, checked: boolean) => void = () => {}
  export let onClose: () => void = () => {}

  const handleBackgroundClick = () => onClose()
  const handleBackgroundKeydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') onClose()
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      onClose()
    }
  }
</script>

{#if open && anchor}
  <div
    class="filter-layer"
    role="button"
    tabindex="0"
    on:click={handleBackgroundClick}
    on:keydown={handleBackgroundKeydown}
  >
    <div
      class="filter-card"
      style={`top:${anchor.bottom + 4}px;left:${anchor.left}px;`}
      role="presentation"
      tabindex="-1"
      on:click|stopPropagation
      on:keydown|stopPropagation
    >
      <div class="options">
        {#each options as opt}
          <label class="option">
            <input
              type="checkbox"
              checked={selected.has(opt.id)}
              on:change={(e) => onToggle(opt.id, (e.target as HTMLInputElement).checked)}
            />
            <span class="text" title={opt.label}>{opt.label}</span>
            {#if opt.description}
              <span class="muted">{opt.description}</span>
            {/if}
          </label>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .filter-layer {
    position: fixed;
    inset: 0;
    z-index: 12;
    background: transparent;
  }

  .filter-card {
    position: absolute;
    min-width: 200px;
    max-width: 260px;
    background: var(--bg);
    color: var(--fg);
    border: 1px solid var(--border);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.18);
    border-radius: 0;
    padding: 8px 10px 10px;
  }

  .options {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 260px;
    overflow-y: auto;
  }

  .option {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    line-height: 1.4;
  }

  .option input[type='checkbox'] {
    accent-color: var(--accent, var(--fg));
    cursor: pointer;
  }

  .text {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .muted {
    grid-column: 2;
    font-size: 12px;
    color: var(--fg-muted);
  }
</style>
