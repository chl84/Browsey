<script lang="ts">
  import ModalShell from '@/shared/ui/ModalShell.svelte'
  import { invoke } from '@/shared/lib/tauri'
  import { getErrorMessage } from '@/shared/lib/error'
  import { useDragDrop } from '../file-ops/useDragDrop'
  import type { createActivity } from '../hooks/createActivity'

  export let paths: string[] = []
  export let activityApi: Pick<ReturnType<typeof createActivity>, 'start' | 'requestCancel' | 'clearNow' | 'cleanup'>
  let busy = false
  let error = ''
  let prepared: string[] = []
  let previous: string[] | null = null
  const drag = useDragDrop()

  $: if (paths !== previous) {
    previous = paths
    prepared = []
    error = ''
  }

  const prepare = async () => {
    if (busy || paths.length === 0) return
    busy = true
    error = ''
    const event = `cloud-export-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      await activityApi.start('Downloading for external copy…', event, () => void activityApi.requestCancel(event), { completeOnReply: true })
      prepared = await invoke<string[]>('prepare_cloud_external_copy', { paths, progressEvent: event })
    } catch (err) { error = getErrorMessage(err) }
    finally { busy = false; activityApi.clearNow(); await activityApi.cleanup() }
  }

  const start = (event: DragEvent) => {
    if (!prepared.length || busy) { event.preventDefault(); return }
    drag.start(prepared, event)
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'copy'
  }
</script>

<ModalShell open={paths.length > 0} title="Copy cloud files to another app" modalWidth="560px"
  closeOnEscape={!busy} closeOnOverlay={!busy} onClose={() => { if (!busy) { drag.end(); paths = [] } }}>
  <p>Download protected local copies first, then drag the button below to another app.
    External drag is copy-only and never deletes cloud originals. Local copies remain available in Settings → Cloud → Working copies → Show storage folder.</p>
  {#if error}<p role="alert" class="error">{error}</p>{/if}
  <div class="actions">
    <button type="button" disabled={busy} on:click={() => void prepare()}>{busy ? 'Downloading…' : prepared.length ? 'Download again' : 'Prepare copies'}</button>
    <button type="button" disabled={!prepared.length || busy} draggable={prepared.length > 0 && !busy}
      on:dragstart={start} on:dragend={() => drag.end()}>Drag {prepared.length} prepared {prepared.length === 1 ? 'item' : 'items'}</button>
    <button type="button" disabled={busy} on:click={() => { paths = [] }}>Close</button>
  </div>
</ModalShell>

<style>
  p { line-height: 1.5; }
  .actions { display: flex; flex-wrap: wrap; gap: 8px; justify-content: flex-end; }
  .error { color: var(--danger); overflow-wrap: anywhere; }
  button[draggable='true'] { -webkit-user-drag: element; user-select: none; cursor: grab; }
</style>
