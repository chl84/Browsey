<script context="module" lang="ts">
  export type UsbFilesystem = 'exfat' | 'fat32' | 'ext4' | 'btrfs'
</script>

<script lang="ts">
  import ModalShell from '@/shared/ui/ModalShell.svelte'
  import type { UsbFilesystemOption, UsbFormatInfo, UsbFormatResult } from '../services/drives.service'

  type UsbFilesystem = 'exfat' | 'fat32' | 'ext4' | 'btrfs'

  export let open = false
  export let volumeLabel = ''
  export let info: UsbFormatInfo | null = null
  export let filesystem: UsbFilesystem = 'exfat'
  export let label = ''
  export let result: UsbFormatResult | null = null
  export let busy = false
  export let onConfirm: () => void = () => {}
  export let onCancel: () => void = () => {}
  export let onOpen: () => void = () => {}

  const formatBytes = (bytes: number) => {
    if (bytes <= 0) return 'Unknown size'
    const units = ['B', 'KB', 'MB', 'GB', 'TB']
    const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1000)), units.length - 1)
    return `${(bytes / 1000 ** index).toFixed(index >= 3 ? 1 : 0)} ${units[index]}`
  }

  $: availableFilesystems = (info?.filesystems ?? []).filter((item) => item.available) as UsbFilesystemOption[]
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
    <svelte:fragment slot="header">{result ? 'USB drive ready' : 'Format USB drive?'}</svelte:fragment>

    {#if result}
      <p class="muted">
        {result.label || info?.model || volumeLabel} is formatted as {result.filesystem} and ready to use.
      </p>
      <dl class="details">
        <div><dt>Size</dt><dd>{formatBytes(result.sizeBytes)}</dd></div>
        {#if result.mountPath}<div><dt>Mounted at</dt><dd>{result.mountPath}</dd></div>{/if}
      </dl>
    {:else if info}
      <p class="muted">
        This permanently erases all data on “{info.model}” ({formatBytes(info.sizeBytes)}), replaces its partition layout, and uses the entire USB drive.
      </p>
      <label class="field" for="usb-volume-label">
        <span>Volume name <em>(optional)</em></span>
        <input id="usb-volume-label" bind:value={label} maxlength="11" autocomplete="off" disabled={busy} />
      </label>
      <label class="field" for="usb-filesystem">
        <span>Filesystem</span>
        <select id="usb-filesystem" bind:value={filesystem} disabled={busy || availableFilesystems.length === 0}>
          {#each availableFilesystems as option}
            <option value={option.id}>{option.label} — {option.description}</option>
          {/each}
        </select>
      </label>
    {:else}
      <p class="muted">Inspecting the USB drive…</p>
    {/if}

    <div slot="actions">
      {#if result}
        <button type="button" data-cancel="1" class="secondary" on:click={onCancel}>Done</button>
        {#if result.mountPath}<button type="button" on:click={onOpen}>Open USB</button>{/if}
      {:else}
        <button type="button" data-cancel="1" class="secondary" on:click={onCancel} disabled={busy}>Cancel</button>
        <button type="button" class="danger" on:click={onConfirm} disabled={busy || !info || availableFilesystems.length === 0}>
          {#if busy}
            Working...
          {:else}
            Format and erase
          {/if}
        </button>
      {/if}
    </div>
  </ModalShell>
{/if}

<style>
  .details { margin: 0; display: grid; gap: 6px; }
  .details div { display: grid; grid-template-columns: 86px minmax(0, 1fr); gap: 8px; }
  dt { color: var(--fg-muted); }
  dd { margin: 0; overflow-wrap: anywhere; }
  em { color: var(--fg-muted); font-style: normal; font-weight: normal; }
</style>
