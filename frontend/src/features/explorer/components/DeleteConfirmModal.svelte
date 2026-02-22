<script lang="ts">
  import ModalShell from '../../../shared/ui/ModalShell.svelte'

  export let open = false
  export let targetLabel = ''
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <ModalShell
    open={open}
    onClose={onCancel}
    overlayClass="danger-overlay"
    initialFocusSelector="button[data-confirm-delete='1']"
  >
    <svelte:fragment slot="header">Delete permanently?</svelte:fragment>
    <p class="muted">This cannot be undone.</p>
    <p class="path">{targetLabel}</p>
    <div slot="actions">
      <button type="button" class="secondary" on:click={onCancel}>Cancel</button>
      <button
        type="button"
        data-confirm-delete="1"
        class="danger"
        on:click={onConfirm}
      >
        Delete
      </button>
    </div>
  </ModalShell>
{/if}

<style>
  /* Styling is inherited from global modal rules in app.css */
</style>
