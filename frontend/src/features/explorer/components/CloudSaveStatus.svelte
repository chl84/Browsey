<script lang="ts">
  import { onMount } from 'svelte'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { cloudWritebackStatuses, type CloudWritebackStatus } from '@/features/network'
  import { cloudSaveSummary, mergeCloudSaveStatus } from '@/features/network'
  import { CloudWorkingCopies } from '@/features/settings'
  import type { ActivityApi } from '../hooks/createActivity'
  export let activityApi: ActivityApi | null = null
  let rows: CloudWritebackStatus[] = []
  let dialog: { show: () => void } | undefined
  let statusError = ''
  $: summary = statusError || cloudSaveSummary(rows)
  $: attention = !!statusError || rows.some(row => ['conflict', 'error', 'unsupported', 'paused'].includes(row.status))
  onMount(() => {
    let disposed = false
    let unlisten: UnlistenFn | undefined
    void (async () => {
      const off = await listen<CloudWritebackStatus>('cloud-writeback', event => {
        if (!disposed) { rows = mergeCloudSaveStatus(rows, event.payload); statusError = '' }
      })
      if (disposed) { off(); return }
      unlisten = off
      const initial = await cloudWritebackStatuses()
      if (!disposed) for (const row of initial) rows = mergeCloudSaveStatus(rows, row)
    })().catch(() => { if (!disposed) statusError = 'Status unavailable' })
    return () => { disposed = true; unlisten?.() }
  })
</script>
{#if rows.length || statusError}
  <div class="cloud-save-status" class:attention>
    <button type="button" on:click={() => dialog?.show()} aria-label={`Cloud saves: ${summary}`}>
      Cloud saves · <span role="status" aria-live="polite">{summary}</span>
    </button>
  </div>
{/if}
<CloudWorkingCopies bind:this={dialog} showTrigger={false} {activityApi} />
<style>
  .cloud-save-status { position: fixed; right: 16px; bottom: 42px; z-index: 5; }
  button { padding: 6px 10px; border: 1px solid var(--border); border-radius: 6px; background: var(--panel, var(--bg)); color: var(--text); font-size: 12px; box-shadow: 0 2px 8px #0002; }
  .attention button { border-color: var(--accent); }
</style>
