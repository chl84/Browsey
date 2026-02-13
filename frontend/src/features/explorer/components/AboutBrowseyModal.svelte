<script lang="ts">
  import ModalShell from '../../../ui/ModalShell.svelte'

  export let open = false
  export let onClose: () => void = () => {}
</script>

{#if open}
  <ModalShell open={open} onClose={onClose} modalWidth="420px">
    <svelte:fragment slot="header">About Browsey</svelte:fragment>

    <section class="section">
      <p class="name">Browsey</p>
      <p class="muted">A minimal file manager focused on speed and clarity.</p>
    </section>

    <section class="section">
      <p class="muted">Built with Rust, Tauri, and Svelte.</p>
    </section>

    <div slot="actions">
      <button type="button" on:click={onClose}>Close</button>
    </div>
  </ModalShell>
{/if}

<style>
  .name {
    font-size: 18px;
    font-weight: 700;
    line-height: 1.2;
  }
</style>
