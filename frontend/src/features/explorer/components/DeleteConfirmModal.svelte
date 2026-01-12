<script lang="ts">
  export let open = false
  export let targetLabel = ''
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <div
    class="overlay"
    role="presentation"
    tabindex="-1"
    on:click={onCancel}
    on:keydown={(e) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        onCancel()
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
          onCancel()
        }
      }}
    >
      <header>Delete permanently?</header>
      <p class="muted">This cannot be undone.</p>
      <p class="path">{targetLabel}</p>
      <div class="actions">
        <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
        <button
          type="button"
          class="danger"
          on:click={onConfirm}
          on:keydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              onConfirm()
            }
          }}
        >
          Delete
        </button>
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
    z-index: 40;
  }

  .modal {
    width: min(420px, 90vw);
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

  .muted {
    margin: 0;
    color: var(--fg-muted);
  }

  .path {
    word-break: break-all;
    background: var(--bg-raised);
    border: 1px solid var(--border);
    padding: 8px;
    border-radius: 8px;
    margin: 0;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  button {
    padding: 8px 12px;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--bg-raised);
    color: var(--fg);
    cursor: pointer;
  }

  button.secondary {
    background: transparent;
  }

  button.danger {
    background: #3b1f21;
    border-color: #7a2f36;
    color: #ffb7b7;
  }

  button:hover {
    border-color: var(--border-accent);
  }
</style>
