<script lang="ts">
  import type { Entry } from '../../explorer/types'
  export let open = false
  export let entry: Entry | null = null
  export let size: number | null = null
  export let onClose: () => void = () => {}
  export let formatSize: (size?: number | null) => string = () => ''
</script>

{#if open && entry}
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
      class="modal"
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
      <div class="row"><span class="label">Name</span><span class="value">{entry.name}</span></div>
      <div class="row"><span class="label">Path</span><span class="value break">{entry.path}</span></div>
      <div class="row"><span class="label">Type</span><span class="value">{entry.kind}</span></div>
      <div class="row">
        <span class="label">Size</span>
        <span class="value">{size ? formatSize(size) : '—'}</span>
      </div>
      <div class="row">
        <span class="label">Accessed</span>
        <span class="value">{entry.kind === 'file' ? entry.accessed ?? '—' : '—'}</span>
      </div>
      <div class="row"><span class="label">Modified</span><span class="value">{entry.modified ?? '—'}</span></div>
      <div class="row"><span class="label">Created</span><span class="value">{entry.created ?? '—'}</span></div>
      <div class="actions">
        <button type="button" on:click={onClose}>Close</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: grid;
    place-items: center;
    z-index: 30;
  }

  .modal {
    width: min(440px, 90vw);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 16px;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  header {
    font-weight: 700;
    font-size: 16px;
  }

  .row {
    display: grid;
    grid-template-columns: 120px 1fr;
    gap: 10px;
    align-items: center;
  }

  .label {
    color: var(--fg-muted);
  }

  .value {
    color: var(--fg);
  }

  .break {
    word-break: break-all;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
  }

  button {
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-raised);
    color: var(--fg);
    cursor: pointer;
  }

  button:hover {
    border-color: var(--border-accent);
  }
</style>
