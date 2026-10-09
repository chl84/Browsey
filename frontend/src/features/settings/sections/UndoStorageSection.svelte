<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import TextField from '@/shared/ui/TextField.svelte'
  import RecoveryBackups from '../RecoveryBackups.svelte'
  import { createUndoStorageModel, describeUndoStorage, describeUndoStorageOverview } from '../undoStorage'

  const { summary, busy, error, refresh, dispose } = createUndoStorageModel()
  onMount(() => { void refresh() })
  onDestroy(dispose)
  let showBackups = false
</script>

<div class="form-label backup-label">Undo backups</div>
<div class="form-control column undo-storage" aria-busy={$busy}>
  <small>Undo up to 50 actions in the current session. Backups are kept across restarts.</small>
  <div role="status" aria-live="polite">
    {#if $error}
      <div class="error">Could not inspect backups: {$error}</div>
      {#if $summary}<small>Previous measurement is not current.</small>{/if}
    {/if}
    {#if $summary}
      <div>{describeUndoStorageOverview($summary)}</div>
      {#if $summary.incomplete}
        <small>Some backups could not be inspected.</small>
      {/if}
    {/if}
  </div>
  <div class="backup-actions">
    {#if $summary && ($summary.sessions > 0 || $summary.incomplete)}
      <button type="button" class="secondary" on:click={() => { showBackups = true }}>Show backups</button>
    {/if}
    <button type="button" class="secondary" disabled={$busy} on:click={() => void refresh()}>
      {$busy ? 'Inspecting backups…' : 'Refresh'}
    </button>
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
