<script lang="ts">
  import ModalShell from '../../ui/ModalShell.svelte'
  export let open = false
  export let onClose: () => void
</script>

<ModalShell title="Settings" open={open} on:close={onClose}>
  <p>Settings placeholder – coming soon.</p>
</ModalShell>
