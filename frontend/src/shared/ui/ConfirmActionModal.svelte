<script lang="ts">
  import ModalShell from './ModalShell.svelte'

  export let open = false
  export let title = 'Confirm action'
  export let message = ''
  export let confirmLabel = 'Confirm'
  export let cancelLabel = 'Cancel'
  export let danger = false
  export let busy = false
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <ModalShell
    open={open}
    onClose={() => {
      if (!busy) onCancel()
    }}
    closeOnEscape={!busy}
    closeOnOverlay={!busy}
    modalWidth="420px"
    initialFocusSelector={danger ? "button[data-cancel='1']" : "button[data-confirm='1']"}
  >
    <svelte:fragment slot="header">{title}</svelte:fragment>
    {#if message}
      <p class="muted">{message}</p>
    {/if}
    <div slot="actions">
      <button type="button" data-cancel="1" class="secondary" on:click={onCancel} disabled={busy}>{cancelLabel}</button>
      <button
        type="button"
        data-confirm="1"
        class={danger ? 'danger' : ''}
        on:click={onConfirm}
        disabled={busy}
      >
        {#if busy}
          Working...
        {:else}
          {confirmLabel}
        {/if}
      </button>
    </div>
  </ModalShell>
{/if}

<style>
  /* Styling is inherited from global modal rules in app.css */
</style>
