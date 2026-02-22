<script lang="ts">
  import ModalShell from '../../../shared/ui/ModalShell.svelte'

  export let open = false
  export let entryName = ''
  export let bookmarkName = ''
  export let bookmarkInputEl: HTMLInputElement | null = null
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <ModalShell open={open} onClose={onCancel} initialFocusSelector="input">
    <svelte:fragment slot="header">Add bookmark</svelte:fragment>

    <p class="muted">Name the bookmark for "{entryName}".</p>
    <input
      id="bookmark-name-input"
      autocomplete="off"
      bind:value={bookmarkName}
      bind:this={bookmarkInputEl}
      aria-label="Bookmark name"
      on:keydown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          onConfirm()
        }
      }}
    />

    <div slot="actions">
      <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
      <button type="button" on:click={onConfirm}>Add</button>
    </div>
  </ModalShell>
{/if}

<style>
  /* Styling is inherited from global modal rules in app.css */
</style>
