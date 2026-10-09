<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import ModalShell from '@/shared/ui/ModalShell.svelte'
  import ProgressBar from '@/shared/ui/ProgressBar.svelte'
  import { formatSize } from '@/shared/lib/formatSize'
  import { parentPath } from '@/features/explorer'
  import FolderDestination from './FolderDestination.svelte'
  import { createRecoveryBackupsModel, type RecoveryBackup } from './recoveryBackups'

  export let onClose: () => void = () => {}
  const model = createRecoveryBackupsModel()
  const { overview, loading, restoring, error, restoredPath, activity } = model
  let selected: RecoveryBackup | null = null
  let destination = ''
  $: recoveredDirectory = $restoredPath ? parentPath($restoredPath) : ''
  const recoverOriginal = async (backup: RecoveryBackup) => {
    const outcome = await model.restore(backup)
    if (outcome === 'choose-destination') { selected = backup; destination = '' }
  }
  const recover = async () => {
    if (!selected || !destination) return
    await model.restore(selected, destination)
    if ($restoredPath) selected = null
  }
  onMount(() => { void model.refresh() })
  onDestroy(model.dispose)
</script>

<ModalShell open title={selected ? 'Recover to…' : 'Recovery backups'} modalWidth="640px"
  onClose={() => { if (!$restoring) onClose() }} closeOnEscape={!$restoring} closeOnOverlay={!$restoring}>
  <div class="recovery-content">
    {#if $error}<p class="error" role="alert">{$error}</p>{/if}
    {#if $restoredPath}
      <div role="status" class="success">
        <p>Recovered to {$restoredPath}</p>
        <button type="button" class="secondary" on:click={() => void model.openFolder(recoveredDirectory)}>Open folder</button>
      </div>
    {/if}
    {#if selected}
      <p class="backup-name">{selected.name}</p>
      <p class="muted">Choose a folder for the recovered copy. Existing files and the backup will be kept.</p>
      <FolderDestination bind:value={destination} disabled={$restoring} />
    {:else}
      <p class="muted">Recover to the original location. Recovered backups leave this list.</p>
      {#if $loading}<p role="status">Loading backups…</p>{/if}
      {#if $overview?.incomplete}<p class="muted" role="status">Some backups could not be inspected. The list may be incomplete.</p>{/if}
      {#if $overview}
        <ul class="backups" aria-label="Backups">
          {#each $overview.entries as backup (backup.id)}
            <li>
              <div class="backup-info">
                <strong>{backup.name}</strong>
                <small class="muted">{backup.kind === 'dir' ? 'Folder' : 'File'}{backup.bytes !== null ? ` · ${formatSize(backup.bytes)}` : ''}{backup.modifiedAt !== null ? ` · ${new Date(backup.modifiedAt * 1000).toLocaleString()}` : ''}</small>
                {#if backup.blockedReason}<small class="muted">{backup.blockedReason}</small>{/if}
              </div>
              <button type="button" class="secondary" disabled={$loading || $restoring || !!backup.blockedReason}
                aria-label={`Recover ${backup.name}`} on:click={() => void recoverOriginal(backup)}>Recover</button>
            </li>
          {/each}
        </ul>
        {#if !$overview.entries.length && !$overview.incomplete}<p>No backups to recover.</p>{/if}
      {/if}
    {/if}
    {#if $activity}
      <div class="recovery-progress" role="status" aria-live="polite">
        <span>{$activity.label}{ $activity.percent !== null ? ` ${$activity.percent}%` : ''}</span>
        <ProgressBar percent={$activity.percent} label="Backup recovery progress" />
        {#if $activity.detail}<small class="muted">{$activity.detail}</small>{/if}
      </div>
    {/if}
  </div>
  <svelte:fragment slot="actions">
    {#if $restoring}
      <button type="button" class="secondary" disabled={$activity?.cancelling} on:click={() => void model.cancel()}>Cancel</button>
    {:else if selected}
      <button type="button" class="secondary" on:click={() => { selected = null; destination = ''; $error = '' }}>Back</button>
      <button type="button" class="primary" disabled={!destination} on:click={() => void recover()}>Recover here</button>
    {:else}
      <button type="button" class="secondary" disabled={$loading} on:click={() => void model.refresh()}>Refresh</button>
      <button type="button" class="primary" on:click={onClose}>Close</button>
    {/if}
  </svelte:fragment>
</ModalShell>

<style>
  .recovery-content { display: flex; flex-direction: column; gap: var(--settings-control-gap); min-width: 0; overflow-wrap: anywhere; }
  p { margin: 0; }
  .backup-name { font-weight: 600; }
  .backups { list-style: none; padding: 0; margin: 0; max-height: 360px; overflow: auto; }
  li { display: flex; align-items: center; gap: 12px; padding: 12px 0; border-bottom: 1px solid var(--border); }
  .backup-info { display: flex; flex-direction: column; gap: 4px; flex: 1; min-width: 0; }
  .backups button { flex-shrink: 0; }
  .recovery-progress, .success { display: flex; flex-direction: column; gap: 8px; }
  .success button { align-self: start; }
  @media (max-width: 480px) { li { align-items: start; flex-direction: column; } }
</style>
