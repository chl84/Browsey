<script lang="ts">
  export let open = false
  export let path = ''
  export let onClose: () => void = () => {}
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
      <header>Open with…</header>
      <p class="muted">Path:</p>
      <code>{path}</code>
      <p class="muted">Integration pending. Choose app from system picker in a future iteration.</p>
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
    color: var(--fg-muted);
    margin: 0;
  }

  code {
    word-break: break-all;
    background: var(--bg-raised);
    padding: 8px;
    border-radius: 8px;
    border: 1px solid var(--border);
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
