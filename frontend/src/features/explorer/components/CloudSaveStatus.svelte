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
  let notificationHidden = false
  let notificationKey = ''
  let notificationTimer: ReturnType<typeof setTimeout> | undefined
  let completedSave: CloudWritebackStatus | null = null
  const removed = new Set<string>()
  const updateNotification = (key: string, duration: number) => {
    // Repeated snapshots must not prolong or revive the same notification.
    if (key === notificationKey) return
    notificationKey = key
    clearTimeout(notificationTimer)
    notificationHidden = false
    notificationTimer = duration ? setTimeout(() => { notificationHidden = true }, duration) : undefined
  }
  $: attentionRows = rows.filter(row => ['conflict', 'error', 'unsupported', 'paused'].includes(row.status))
  $: savingRows = rows.filter(row => row.status === 'pending' || row.status === 'uploading')
  $: attention = !!statusError || attentionRows.length > 0
  $: key = attention
    ? JSON.stringify([statusError, attentionRows.map(row => [row.id, row.status, row.message] as const).sort((a, b) => a[0].localeCompare(b[0]))])
    : completedSave && rows.some(row => row.id === completedSave?.id && row.status === 'saved') && !savingRows.length
      ? `saved:${completedSave.sequence}` : ''
  $: updateNotification(key, attention ? 0 : key ? 4000 : 0)
  $: summary = notificationHidden && attention ? cloudSaveSummary(savingRows) : statusError || cloudSaveSummary(rows)
  onMount(() => {
    let disposed = false
    const unlisten: UnlistenFn[] = []
    void (async () => {
      const offRemoved = await listen<string>('cloud-working-copy-removed', event => {
        if (disposed) return
        removed.add(event.payload)
        rows = rows.filter(row => row.id !== event.payload)
        if (completedSave?.id === event.payload) completedSave = null
      })
      if (disposed) { offRemoved(); return }
      unlisten.push(offRemoved)
      const off = await listen<CloudWritebackStatus>('cloud-writeback', event => {
        if (disposed || removed.has(event.payload.id)) return
        const next = mergeCloudSaveStatus(rows, event.payload)
        if (next === rows) return
        rows = next
        // Snapshots and unchanged-copy checks are state, not new save notices.
        // Only a confirmed write (including journal recovery) starts the timer.
        if (event.payload.saveCompleted) completedSave = event.payload
        else if (event.payload.status !== 'saved') completedSave = null
        statusError = ''
      })
      if (disposed) { off(); return }
      unlisten.push(off)
      const initial = await cloudWritebackStatuses()
      if (!disposed) for (const row of initial) { if (!removed.has(row.id)) rows = mergeCloudSaveStatus(rows, row) }
    })().catch(() => { if (!disposed) statusError = 'Status unavailable' })
    return () => { disposed = true; for (const off of unlisten) off(); clearTimeout(notificationTimer) }
  })
</script>
{#if savingRows.length || ((attention || key) && !notificationHidden)}
  <div class="cloud-save-status" class:attention={attention && !notificationHidden}>
    <button type="button" on:click={() => dialog?.show()} aria-label={`Cloud saves: ${summary}`}>
      Cloud saves · <span role="status" aria-live="polite">{summary}</span>
    </button>
    {#if attention && !notificationHidden}
      <button class="dismiss" type="button" aria-label="Dismiss cloud save notification" title="Dismiss notification" on:click={() => { notificationHidden = true }}>
        <span aria-hidden="true">×</span>
      </button>
    {/if}
  </div>
{/if}
<CloudWorkingCopies bind:this={dialog} showTrigger={false} {activityApi} />
<style>
  .cloud-save-status { position: fixed; right: 16px; bottom: 42px; z-index: 5; display: flex; align-items: center; gap: 4px; }
  button { padding: 6px 10px; border: 1px solid var(--border); border-radius: 6px; background: var(--panel, var(--bg)); color: var(--text); font-size: 12px; box-shadow: 0 2px 8px #0002; }
  .attention button { border-color: var(--accent); }
  .dismiss { padding: 6px 8px; }
</style>
