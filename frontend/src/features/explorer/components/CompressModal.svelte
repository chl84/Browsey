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
  const confirmAndClose = () => {
    onConfirm(value, Number(level))
    onCancel()
  }

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
          type="text"
          bind:this={inputEl}
          bind:value={value}
          on:keydown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              confirmAndClose()
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
        <button type="button" on:click={confirmAndClose}>Create</button>
      </div>
    </div>
  </div>
{/if}
