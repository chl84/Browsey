<script lang="ts">
  import ModalShell from '../../ui/ModalShell.svelte'
  export let open = false
  export let onClose: () => void
</script>

{#if open}
  <ModalShell title="Settings" open={open} onClose={onClose}>
    <p>Settings placeholder – coming soon.</p>
  </ModalShell>
{/if}
