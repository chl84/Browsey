<script lang="ts">
  import { tick } from 'svelte'

  export let open = false
  export let value = ''
  export let error = ''
  export let onConfirm: (name: string) => void = () => {}
  export let onCancel: () => void = () => {}

  let inputEl: HTMLInputElement | null = null
  let selectedThisOpen = false

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
      style="--modal-width: 360px;"
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
      <header>New folder</header>
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
        <button type="button" on:click={() => onConfirm(value)}>Create</button>
      </div>
    </div>
  </div>
{/if}
