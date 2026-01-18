<script lang="ts">
  import type { Entry } from '../types'
  export let open = false
  export let entry: Entry | null = null
  export let count = 1
  export let size: number | null = null
  export let onClose: () => void = () => {}
  export let formatSize: (size?: number | null) => string = () => ''
</script>

{#if open}
  <div
    class="overlay"
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
      class="modal properties-modal"
      role="dialog"
      aria-modal="true"
      tabindex="0"
      on:click|stopPropagation
      on:keydown={(e) => {
        if (e.key === 'Escape') {
          e.preventDefault()
          onClose()
        }
      }}
    >
      <header>Properties</header>

      {#if count === 1 && entry}
        <div class="row"><span class="label">Name</span><span class="value">{entry.name}</span></div>
        <div class="row"><span class="label">Path</span><span class="value break">{entry.path}</span></div>
        <div class="row"><span class="label">Type</span><span class="value">{entry.kind}</span></div>
      {:else}
        <div class="row"><span class="label">Items</span><span class="value">{count}</span></div>
      {/if}

      <div class="row">
        <span class="label">Size</span>
        <span class="value">
          {#if size !== null && size !== undefined}
            {formatSize(size)} ({count} {count === 1 ? 'item' : 'items'})
          {:else}
            — ({count} {count === 1 ? 'item' : 'items'})
          {/if}
        </span>
      </div>

      {#if count === 1 && entry}
        <div class="row">
          <span class="label">Accessed</span>
          <span class="value">{entry.kind === 'file' ? entry.accessed ?? '—' : '—'}</span>
        </div>
        <div class="row"><span class="label">Modified</span><span class="value">{entry.modified ?? '—'}</span></div>
        <div class="row"><span class="label">Created</span><span class="value">{entry.created ?? '—'}</span></div>
      {/if}

      <div class="actions">
        <button type="button" on:click={onClose}>Close</button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Styling is inherited from global modal rules in app.css */
</style>
