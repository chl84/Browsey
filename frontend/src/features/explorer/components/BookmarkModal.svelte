<script lang="ts">
  export let open = false
  export let entryName = ''
  export let bookmarkName = ''
  export let bookmarkInputEl: HTMLInputElement | null = null
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
      <header>Add bookmark</header>
      <p class="muted">Name the bookmark for "{entryName}".</p>
      <input
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
      <div class="actions">
        <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
        <button type="button" on:click={onConfirm}>Add</button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* Styling is inherited from global modal rules in app.css */
</style>
