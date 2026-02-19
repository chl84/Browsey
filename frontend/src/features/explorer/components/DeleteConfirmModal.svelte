<script lang="ts">
  import ModalShell from '../../../shared/ui/ModalShell.svelte'

  export let open = false
  export let targetLabel = ''
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <ModalShell open={open} onClose={onCancel} overlayClass="danger-overlay">
    <svelte:fragment slot="header">Delete permanently?</svelte:fragment>
    <p class="muted">This cannot be undone.</p>
    <p class="path">{targetLabel}</p>
    <div slot="actions">
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
  </ModalShell>
{/if}

<style>
  /* Styling is inherited from global modal rules in app.css */
</style>
