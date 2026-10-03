<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import TextField from '@/shared/ui/TextField.svelte'
  import { createUndoStorageModel, describeUndoStorage } from '../undoStorage'

  const { summary, busy, error, refresh, dispose } = createUndoStorageModel()
  onMount(() => { void refresh() })
  onDestroy(dispose)
</script>

<div class="form-label">Undo and recovery backups</div>
<div class="form-control column undo-storage" aria-busy={$busy}>
  <button type="button" class="secondary" disabled={$busy} on:click={() => void refresh()}>
    {$busy ? 'Inspecting backups…' : 'Refresh backup information'}
  </button>
  <div role="status" aria-live="polite">
    {#if $error}
      <div class="error">Could not inspect undo storage: {$error}</div>
      {#if $summary}<small>Previous measurement below is not current.</small>{/if}
    {/if}
    {#if $summary}
      <div>{describeUndoStorage($summary)}</div>
      {#if $summary.incomplete}
        <small>Some entries were not measured because of scan limits, inaccessible paths, or unsafe file types. These are not complete totals.</small>
      {/if}
    {/if}
  </div>
  {#if $summary}
    <TextField value={$summary.directory} readonly aria-label="Undo backup directory" />
  {/if}
  <small>File-content size is not allocated disk usage; hard-linked paths are counted separately. Measurements can change while Browsey runs.</small>
  <small>Undo history holds up to 50 actions for this app session only. This is not a disk-space limit. Recovery markers keep entire sessions across restarts, without restoring undo history.</small>
  <small>Recovery markers can also belong to ongoing work; their presence alone does not mean an operation failed.</small>
  <details>
    <summary>Manual recovery guidance</summary>
    <ol>
      <li>Markers can belong to work still in progress. Finish file operations and close all Browsey windows before manual recovery. Do not launch older Browsey builds that may ignore markers.</li>
      <li>Use the error's backup and destination paths. Marker files contain diagnostic paths, not an automatic restore plan.</li>
      <li>Copy needed data to a separate safe folder without overwriting uncertain destination contents. Verify the recovered files before considering cleanup.</li>
      <li>Do not remove markers just to free space: startup cleanup may delete the entire session after its last marker is removed. Older Browsey builds may not honor markers.</li>
    </ol>
    <small>This panel only inspects storage. It does not restore, resume, delete backups, or clear markers. Unmarked backups may still be needed by a running instance.</small>
    <small>Legacy backups and sibling session lock files are excluded from the measurements.</small>
  </details>
</div>

<style>
  .undo-storage {
    min-width: 0;
    overflow-wrap: anywhere;
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
    padding-left: 1.5rem;
    color: var(--fg-muted);
    line-height: var(--modal-line-height);
  }
  li + li {
    margin-top: var(--settings-control-gap-tight);
  }
</style>
