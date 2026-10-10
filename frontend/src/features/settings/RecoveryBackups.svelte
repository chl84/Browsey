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
  const { overview, loading, restoring, error, notice, restoredPath, restoredName, activity } = model
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

<ModalShell open title={selected ? 'Recover to…' : 'Recovery backups'} modalWidth="640px" modalClass="recovery-modal"
  onClose={() => { if (!$restoring) onClose() }} closeOnEscape={!$restoring} closeOnOverlay={!$restoring}>
  <div class="recovery-content modal-scroll" aria-busy={$loading || $restoring}>
    {#if $error}<div class="pill error" role="alert">{$error}</div>{/if}
    {#if $notice}<p class="muted" role="status">{$notice}</p>{/if}
    {#if $restoredPath}
      <div role="status" class="success">
        <p title={$restoredPath}>Recovered {$restoredName}</p>
        <button type="button" class="secondary" on:click={() => void model.openFolder(recoveredDirectory)}>Open folder</button>
      </div>
    {/if}
    {#if selected}
      <p class="backup-name">{selected.name}</p>
      <p class="muted">Choose a folder for the recovered copy. Existing files and the backup will be kept.</p>
      <FolderDestination bind:value={destination} disabled={$restoring} />
    {:else}
      <p class="muted">Recover to the original location. Recovered backups leave this list.</p>
      {#if $loading && !$overview}<p role="status">Loading backups…</p>{/if}
      {#if $overview?.incomplete}<p class="muted" role="status">Some backups could not be inspected. The list may be incomplete.</p>{/if}
      {#if $overview?.entries.length}
        <ul class="backups modal-scroll" aria-label="Backups">
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
      {:else if $overview && !$overview.incomplete}<p>No backups to recover.</p>{/if}
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
      <button type="button" class="secondary" on:click={() => { selected = null; destination = ''; $error = ''; $notice = '' }}>Back</button>
      <button type="button" class="primary" disabled={!destination} on:click={() => void recover()}>Recover here</button>
    {:else}
      <button type="button" class="secondary" disabled={$loading} on:click={() => void model.refresh()}>Refresh</button>
      <button type="button" class="primary" on:click={onClose}>Close</button>
    {/if}
  </svelte:fragment>
</ModalShell>

<style>
  :global(.recovery-modal) { height: min(600px, 92vh); overflow: hidden; }
  :global(.recovery-modal > header), :global(.recovery-modal > .actions) { flex-shrink: 0; }
  .recovery-content { display: flex; flex-direction: column; flex: 1; gap: var(--modal-gap); min-width: 0; min-height: 0; overflow-wrap: anywhere; }
  .recovery-content > :not(.backups) { flex-shrink: 0; }
  .backup-name { font-weight: 600; }
  .backups { list-style: none; padding-block: 0; padding-inline-start: 0; margin: 0; flex: 1; min-height: 120px; }
  li { display: flex; align-items: center; gap: var(--modal-actions-gap); padding: var(--modal-actions-gap) 0; border-bottom: 1px solid var(--border); }
  .backup-info { display: flex; flex-direction: column; gap: calc(var(--modal-field-gap) / 2); flex: 1; min-width: 0; }
  .backups button { flex-shrink: 0; }
  .recovery-progress { display: flex; flex-direction: column; gap: var(--modal-field-gap); }
  .success { display: flex; align-items: center; flex-wrap: wrap; gap: var(--modal-actions-gap); }
  .success button { flex-shrink: 0; }
  @media (max-width: 480px) { li { align-items: start; flex-direction: column; } }
</style>
