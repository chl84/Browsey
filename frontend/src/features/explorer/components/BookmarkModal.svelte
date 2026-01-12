<script lang="ts">
  export let open = false
  export let entryName = ''
  export let bookmarkName = ''
  export let bookmarkInputEl: HTMLInputElement | null = null
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <div class="modal-backdrop" role="dialog" aria-modal="true">
    <div class="modal">
      <h2 class="modal-title">Add bookmark</h2>
      <p class="modal-desc">Name the bookmark for "{entryName}".</p>
      <input
        class="modal-input"
        bind:value={bookmarkName}
        bind:this={bookmarkInputEl}
        aria-label="Bookmark name"
        on:keydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            onConfirm()
          } else if (e.key === 'Escape') {
            e.preventDefault()
            onCancel()
          }
        }}
      />
      <div class="modal-actions">
        <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
        <button type="button" on:click={onConfirm}>Add</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10;
  }

  .modal {
    background: var(--bg);
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    padding: 18px;
    width: min(420px, 90vw);
    box-shadow: 0 16px 32px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .modal-title {
    margin: 0;
    font-size: 16px;
    font-weight: 700;
    color: var(--fg);
  }

  .modal-desc {
    margin: 0;
    color: var(--fg-muted);
    font-size: 14px;
  }

  .modal-input {
    width: 100%;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 10px 12px;
    background: var(--bg);
    color: var(--fg);
    font-size: 14px;
  }

  .modal-input:focus {
    outline: 2px solid var(--border-accent);
    border-color: var(--border-accent-strong);
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }

  .modal-actions button {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 12px;
    background: var(--bg-button);
    color: var(--fg);
    cursor: pointer;
  }

  .modal-actions button.secondary {
    background: transparent;
  }
</style>
