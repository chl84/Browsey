<script lang="ts">
  import ModalShell from './ModalShell.svelte'

  export let open = false
  export let conflicts: { src: string; target: string; is_dir: boolean }[] = []
  export let onOverwrite: () => void = () => {}
  export let onRenameAll: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <ModalShell
    open={open}
    onClose={onCancel}
    overlayClass="conflict-overlay"
    modalClass="conflict-modal"
  >
    <svelte:fragment slot="header">Items already exist</svelte:fragment>
    <p>{conflicts.length} item{conflicts.length === 1 ? '' : 's'} are already present in the destination.</p>
    <div class="conflicts">
      {#each conflicts as conflict (conflict.target)}
        <div class="row">
          <div class="name">
            {conflict.is_dir ? '📁' : '📄'} {conflict.src}
          </div>
          <div class="target">→ {conflict.target}</div>
        </div>
      {/each}
    </div>
    <div slot="actions">
      <button class="secondary" type="button" on:click={onCancel}>Cancel</button>
      <button class="secondary" type="button" on:click={onRenameAll}>Auto-rename</button>
      <button class="primary" type="button" on:click={onOverwrite}>Overwrite</button>
    </div>
  </ModalShell>
{/if}

<style>
  /* Styling is inherited from global modal rules in app.css */
</style>
