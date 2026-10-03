<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import TextField from '@/shared/ui/TextField.svelte'
  import { createUndoStorageModel, describeUndoStorage, describeUndoStorageOverview } from '../undoStorage'

  const { summary, busy, error, refresh, dispose } = createUndoStorageModel()
  onMount(() => { void refresh() })
  onDestroy(dispose)
</script>

<div class="form-label backup-label">Recovery backups</div>
<div class="form-control column undo-storage" aria-busy={$busy}>
  <div role="status" aria-live="polite">
    {#if $error}
      <div class="error">Could not inspect undo storage: {$error}</div>
      {#if $summary}<small>Previous measurement below is not current.</small>{/if}
    {/if}
    {#if $summary}
      <div>{describeUndoStorageOverview($summary)}</div>
      {#if $summary.markedSessions > 0}
        <small>{$summary.markedSessions} {$summary.markedSessions === 1 ? 'session' : 'sessions'} with recovery markers. These may belong to ongoing work.</small>
      {/if}
      {#if $summary.incomplete}
        <small>Some entries were not measured because of scan limits, inaccessible paths, or unsafe file types. These are not complete totals.</small>
      {/if}
    {/if}
  </div>
  <button type="button" class="secondary" disabled={$busy} on:click={() => void refresh()}>
    {$busy ? 'Inspecting backups…' : 'Refresh backup information'}
  </button>
  <small>Undo is session-only (up to 50 actions). This panel only inspects backups; it does not restore or delete them.</small>
  <details>
    <summary>Backup details and recovery guidance</summary>
    <div class="backup-details">
      {#if $summary}
        <div>{describeUndoStorage($summary)}</div>
        <TextField value={$summary.directory} readonly aria-label="Undo backup directory" />
      {/if}
      <small>File-content size and filesystem-reported allocation differ. Allocation is not exclusive physical usage on compressed or copy-on-write filesystems; hard-linked paths are counted separately. Measurements can change while Browsey runs.</small>
      <small>The 50-action undo limit is not a disk-space limit. Recovery markers keep entire sessions across restarts, without restoring undo history. Their presence alone does not mean an operation failed.</small>
      <h4>Manual recovery</h4>
      <ol>
        <li>Markers can belong to work still in progress. Finish file operations and close all Browsey windows before manual recovery. Do not launch older Browsey builds that may ignore markers.</li>
        <li>Use the error's backup and destination paths. Marker files contain diagnostic paths, not an automatic restore plan.</li>
        <li>Copy needed data to a separate safe folder without overwriting uncertain destination contents. Verify the recovered files before considering cleanup.</li>
        <li>Do not remove markers just to free space: startup cleanup may delete the entire session after its last marker is removed. Older Browsey builds may not honor markers.</li>
      </ol>
      <small>This panel only inspects storage. It does not restore, resume, delete backups, or clear markers. Unmarked backups may still be needed by a running instance.</small>
      <small>Legacy backups and sibling session lock files are excluded from the measurements.</small>
    </div>
  </details>
</div>

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
  h4 {
    margin: 0;
    font-size: inherit;
  }
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
  ol {
    margin: 0;
    padding-left: 1.5rem;
    color: var(--fg-muted);
    line-height: var(--modal-line-height);
  }
  li + li {
    margin-top: var(--settings-control-gap-tight);
  }
</style>
