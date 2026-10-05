<script lang="ts">
  import ModalShell from '../../../shared/ui/ModalShell.svelte'
  import type { DeleteConfirmMode } from '../modals/deleteConfirmModal'

  export let open = false
  export let targetLabel = ''
  export let mode: DeleteConfirmMode = 'default'
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <ModalShell
    open={open}
    onClose={onCancel}
    overlayClass="danger-overlay"
    initialFocusSelector={mode === 'network' || mode === 'network-trash' ? "button[data-cancel-delete='1']" : "button[data-confirm-delete='1']"}
  >
    <svelte:fragment slot="header">{mode === 'network-trash' ? 'Network trash unavailable' : 'Delete permanently?'}</svelte:fragment>
    {#if mode === 'network-trash'}
      <p class="muted">Some network items cannot be moved to trash. Delete those items permanently? Browsey cannot undo this and will not download a backup. Other items will be moved to trash where supported.</p>
    {:else if mode === 'network'}
      <p class="muted">Network items will be deleted directly on the server, without a local backup. Browsey cannot undo this.</p>
    {:else}
      <p class="muted">Items bypass the Wastebasket. Browsey can undo supported local deletions only while this session is running. Cloud and network deletions cannot be undone.</p>
    {/if}
    <p class="path">{targetLabel}</p>
    <div slot="actions">
      <button type="button" data-cancel-delete="1" class="secondary" on:click={onCancel}>Cancel</button>
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
