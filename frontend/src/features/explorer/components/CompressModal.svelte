<script lang="ts">
  import { tick } from 'svelte'

  export let open = false
  export let value = ''
  export let level: number = 6
  export let error = ''
  export let onConfirm: (name: string, level: number) => void = () => {}
  export let onCancel: () => void = () => {}

  let inputEl: HTMLInputElement | null = null
  let selectedThisOpen = false
  let overlayPointerDown = false

  $: {
    if (!open) {
      selectedThisOpen = false
    } else if (inputEl && !selectedThisOpen) {
      void tick().then(() => {
        if (open && inputEl && !selectedThisOpen) {
          inputEl.select()
          selectedThisOpen = true
        }
      })
    }
  }
</script>

{#if open}
  <div
    class="overlay"
    role="presentation"
    tabindex="-1"
    on:pointerdown={(e) => {
      // Only close if the press started on the overlay itself.
      overlayPointerDown = e.target === e.currentTarget
    }}
    on:click={(e) => {
      if (overlayPointerDown && e.target === e.currentTarget) {
        onCancel()
      }
      overlayPointerDown = false
    }}
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
      <header>Compress</header>
      {#if error}
        <div class="pill error">{error}</div>
      {/if}
      <label class="field">
        <span>Archive name</span>
        <input
          bind:this={inputEl}
          bind:value={value}
          on:keydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              onConfirm(value, level)
            } else if (e.key === 'Escape') {
              e.preventDefault()
              onCancel()
            }
          }}
        />
      </label>
      <label class="field">
        <span>Compression level</span>
        <input type="range" min="0" max="9" step="1" bind:value={level} />
        <div class="muted">0 = store only, 9 = maximum compression</div>
      </label>
      <div class="actions">
        <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
        <button type="button" on:click={() => onConfirm(value, Number(level))}>Create</button>
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
    gap: 12px;
  }

  header {
    font-weight: 700;
    font-size: 16px;
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

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field span {
    font-weight: 600;
  }

  .muted {
    color: var(--fg-muted);
    font-size: 12px;
  }

  input[type='text'],
  input:not([type]) {
    padding: 10px 12px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    border-radius: 8px;
  }

  input:focus {
    outline: 2px solid var(--border-accent);
  }

  input[type='range'] {
    width: 100%;
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
