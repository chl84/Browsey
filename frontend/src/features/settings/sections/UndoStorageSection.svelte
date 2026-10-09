<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import TextField from '@/shared/ui/TextField.svelte'
  import ConfirmActionModal from '@/shared/ui/ConfirmActionModal.svelte'
  import RecoveryBackups from '../RecoveryBackups.svelte'
  import { createUndoStorageModel, describeUndoStorage, describeUndoStorageOverview } from '../undoStorage'

  const { summary, busy, error, deleting, message, refresh, deleteAll, dispose } = createUndoStorageModel()
  onMount(() => { void refresh() })
  onDestroy(dispose)
  let showBackups = false
  let confirmDelete = false
</script>

<div class="form-label backup-label">Undo backups</div>
<div class="form-control column undo-storage" aria-busy={$busy}>
  <small>Undo up to 50 actions in the current session. Backups are kept across restarts.</small>
  <div role="status" aria-live="polite">
    {#if $error}
      <div class="error">{$error}</div>
      {#if $summary}<small>Previous measurement is not current.</small>{/if}
    {/if}
    {#if $message}<div>{$message}</div>{/if}
    {#if $summary}
      <div>{describeUndoStorageOverview($summary)}</div>
      {#if $summary.incomplete}
        <small>Some backups could not be inspected.</small>
      {/if}
    {/if}
  </div>
  <div class="backup-actions">
    <button type="button" class="secondary" disabled={$deleting} on:click={() => { showBackups = true }}>Show all</button>
    <button type="button" class="danger" disabled={$busy} on:click={() => { $error = ''; confirmDelete = true }}>Delete all</button>
    {#if $busy}<small role="status">{$deleting ? 'Deleting backups…' : 'Inspecting backups…'}</small>{/if}
  </div>
  <details>
    <summary>Advanced details</summary>
    <div class="backup-details">
      {#if $summary}
        <div>{describeUndoStorage($summary)}</div>
        <TextField value={$summary.directory} readonly aria-label="Undo backup directory" />
      {/if}
      <small>Stored backups have no automatic expiry. Startup cleanup only removes completely empty sessions. The 50-action limit applies to undo history, not storage.</small>
      <small>Recover restores to the original location. If that is unavailable, Recover to… lets you choose another folder. Completed backups from this instance are available immediately; active file operations and other running instances are unavailable. Recovery does not recreate undo history or reverse the original operation.</small>
      <small>Storage measurements may be partial. Filesystem allocation can differ from file sizes, especially on compressed or copy-on-write filesystems. Legacy backup folders are excluded.</small>
      <small>Verified recovered backups leave the list. Backup data and protection markers remain for undo. Older builds may delete backups on startup; recover needed files before downgrading.</small>
    </div>
  </details>
</div>

{#if showBackups}
  <RecoveryBackups onClose={() => { showBackups = false; void refresh() }} />
{/if}

<ConfirmActionModal open={confirmDelete} title="Delete all backups?"
  message={`This permanently deletes stored backups and clears undo and redo history. Backups in use by another Browsey instance are kept.${$error ? `\n\nLast error: ${$error}` : ''}`}
  confirmLabel="Delete all" danger busy={$deleting}
  onConfirm={() => { void deleteAll().then(done => { if (done) confirmDelete = false }) }}
  onCancel={() => { confirmDelete = false }} />

<style>
  .undo-storage {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .backup-label {
    align-self: start;
  }
  .backup-details {
    display: flex;
    flex-direction: column;
    gap: var(--settings-control-gap);
    padding-top: var(--settings-control-gap);
  }
  .backup-actions { display: flex; gap: var(--settings-control-gap); flex-wrap: wrap; }
  details {
    width: 100%;
  }
  summary {
    cursor: pointer;
  }
  summary:focus-visible {
    outline: var(--focus-ring-width) solid var(--focus-ring-color);
    outline-offset: var(--focus-ring-offset);
  }
</style>
