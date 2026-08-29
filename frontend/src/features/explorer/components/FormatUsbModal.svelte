<script context="module" lang="ts">
  export type UsbFilesystem = 'exfat' | 'fat32' | 'ext4' | 'btrfs'
</script>

<script lang="ts">
  import ModalShell from '@/shared/ui/ModalShell.svelte'

  type UsbFilesystem = 'exfat' | 'fat32' | 'ext4' | 'btrfs'

  export let open = false
  export let volumeLabel = ''
  export let filesystem: UsbFilesystem = 'exfat'
  export let busy = false
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}
</script>

{#if open}
  <ModalShell
    open={open}
    modalWidth="420px"
    onClose={() => {
      if (!busy) onCancel()
    }}
    closeOnEscape={!busy}
    closeOnOverlay={!busy}
    initialFocusSelector="button[data-cancel='1']"
  >
    <svelte:fragment slot="header">Format USB drive?</svelte:fragment>

    <p class="muted">
      This permanently erases all data on “{volumeLabel}”, replaces its partition layout, and uses the entire USB drive.
    </p>
    <label class="field" for="usb-filesystem">
      <span>Filesystem</span>
      <select id="usb-filesystem" bind:value={filesystem} disabled={busy}>
        <option value="exfat">exFAT — compatible with Linux, macOS, and Windows</option>
        <option value="fat32">FAT32 — widest compatibility, 4 GB file limit</option>
        <option value="ext4">ext4 — Linux</option>
        <option value="btrfs">Btrfs — Linux, snapshots and checksums</option>
      </select>
    </label>

    <div slot="actions">
      <button type="button" data-cancel="1" class="secondary" on:click={onCancel} disabled={busy}>Cancel</button>
      <button type="button" class="danger" on:click={onConfirm} disabled={busy}>
        {#if busy}
          Working...
        {:else}
          Format and erase
        {/if}
      </button>
    </div>
  </ModalShell>
{/if}
