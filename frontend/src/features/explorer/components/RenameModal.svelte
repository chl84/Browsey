<script lang="ts">
  export let open = false
  export let entryName = ''
  export let value = ''
  export let error = ''
  export let onConfirm: (name: string) => void = () => {}
  export let onCancel: () => void = () => {}
  let inputEl: HTMLInputElement | null = null

  $: if (open && inputEl) {
    inputEl.focus()
    inputEl.select()
  }
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
      <header>Rename</header>
      <p class="muted">{entryName}</p>
      {#if error}
        <div class="pill error">{error}</div>
      {/if}
      <input
        bind:this={inputEl}
        bind:value={value}
        on:keydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            onConfirm(value)
          } else if (e.key === 'Escape') {
            e.preventDefault()
            onCancel()
          }
        }}
      />
      <div class="actions">
        <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
        <button type="button" on:click={() => onConfirm(value)}>Rename</button>
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
    width: min(360px, 90vw);
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

  .pill {
    display: inline-flex;
    padding: 6px 10px;
    border-radius: 999px;
    border: 1px solid var(--border-accent);
    background: var(--bg-alt);
    color: var(--fg);
    font-weight: 700;
    font-size: 12px;
  }

  .pill.error {
    border-color: #c0392b;
    background: rgba(192, 57, 43, 0.15);
    color: #fceaea;
  }

  input {
    padding: 10px 12px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    border-radius: 8px;
  }

  input:focus {
    outline: 2px solid var(--border-accent);
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

  button:hover {
    border-color: var(--border-accent);
  }
</style>
