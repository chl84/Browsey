<script lang="ts">
  import ModalShell from '@/shared/ui/ModalShell.svelte'
  import { invoke } from '@/shared/lib/tauri'
  import { getErrorMessage } from '@/shared/lib/error'
  import { listCloudWorkingCopies, uploadCloudWorkingCopy, type CloudWorkingCopy } from '@/features/network'
  import type { ActivityApi } from '@/features/explorer'
  import { onMount } from 'svelte'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { saveCloudWorkingCopy, setCloudWorkingCopyAutoSave, type CloudWritebackStatus } from '@/features/network'
  import { cloudSaveLabel } from '@/features/network'

  export let activityApi: ActivityApi | null = null
  export let showTrigger = true
  export const show = () => { open = true; void refresh() }

  let open = false
  let loading = false
  let error = ''
  let copies: CloudWorkingCopy[] = []
  let uploading = false
  let message = ''
  let refreshId = 0
  const latest = new Map<string, CloudWritebackStatus>()
  const applyStatus = (copy: CloudWorkingCopy) => {
    const row = latest.get(copy.id)
    return row ? { ...copy, saveStatus: row.status, saveMessage: row.message, ...(row.status === 'saved' ? { dirty: false } : row.status === 'pending' || row.status === 'uploading' ? { dirty: true } : {}) } : copy
  }

  onMount(() => {
    let disposed = false
    let unlisten: UnlistenFn | undefined
    void listen<CloudWritebackStatus>('cloud-writeback', event => {
      if (disposed) return
      const row = event.payload
      if ((latest.get(row.id)?.sequence ?? -1) >= row.sequence) return
      latest.set(row.id, row)
      if (open) copies = copies.map(copy => copy.id === row.id ? applyStatus(copy) : copy)
    }).then(off => { if (disposed) off(); else unlisten = off }).catch(() => {})
    return () => { disposed = true; unlisten?.(); ++refreshId }
  })

  const saveOriginal = async (copy: CloudWorkingCopy) => {
    if (uploading) return
    uploading = true; error = ''; message = ''
    try {
      const saved = await saveCloudWorkingCopy(copy.id)
      message = saved.dirty ? 'Saved to the original cloud file. Newer local edits are waiting to save.' : 'Saved to the original cloud file.'
      await refresh()
    } catch (err) { error = getErrorMessage(err); await refresh() }
    finally { uploading = false }
  }
  const toggleAutomatic = async (copy: CloudWorkingCopy) => {
    if (uploading) return
    uploading = true; error = ''
    try { await setCloudWorkingCopyAutoSave(copy.id, !copy.autoSave); await refresh() }
    catch (err) { error = getErrorMessage(err) }
    finally { uploading = false }
  }

  const upload = async (copy: CloudWorkingCopy) => {
    if (uploading) return
    uploading = true
    error = ''
    message = ''
    const event = `cloud-edit-upload-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      await activityApi?.start('Checking and uploading edited file…', event, () => void activityApi?.requestCancel(event))
      const result = await uploadCloudWorkingCopy(copy.id, event)
      message = `${result.sourceChanged ? 'The cloud original changed. ' : ''}Saved as a new file: ${result.path}. The original and working copy were kept.`
      await refresh()
    } catch (err) { error = getErrorMessage(err) }
    finally { uploading = false; activityApi?.clearNow(); await activityApi?.cleanup() }
  }

  const showStorage = async () => {
    try {
      const path = await invoke<string>('cloud_working_copy_storage_path')
      await invoke('open_entry', { path })
    } catch (err) { error = getErrorMessage(err) }
  }

  const refresh = async () => {
    const id = ++refreshId
    const started = new Map([...latest].map(([key, row]) => [key, row.sequence]))
    loading = true
    try {
      const next = await listCloudWorkingCopies()
      if (id === refreshId) copies = next.map(copy =>
        (latest.get(copy.id)?.sequence ?? -1) > (started.get(copy.id) ?? -1) ? applyStatus(copy) : copy)
    }
    catch (err) { if (id === refreshId) error = getErrorMessage(err) }
    finally { if (id === refreshId) loading = false }
  }

  const openCopy = async (copy: CloudWorkingCopy, folder = false) => {
    error = ''
    try {
      await invoke('open_entry', { path: folder ? copy.localPath.replace(/[/\\][^/\\]+$/, '') : copy.localPath })
    } catch (err) { error = getErrorMessage(err) }
  }
</script>

{#if showTrigger}<button type="button" class="secondary" on:click={show}>Working copies…</button>{/if}
<ModalShell {open} title="Cloud working copies" onClose={() => { if (!uploading) open = false }} closeOnEscape={!uploading} closeOnOverlay={!uploading} modalWidth="760px">
  <p>Newly opened cloud files save changes back to the original automatically after the editor finishes saving.
    Keep Browsey running and wait for “Saved to cloud” before closing it.</p>
  <p>Conflicts stop automatic saving and keep your local edits. Earlier working copies stay manual until you choose otherwise.
    Local copies survive restart and cache clearing. Pause automatic saving before editing if you want to upload only a new file.</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}
  <div class="copies" aria-busy={loading}>
    {#each copies as copy (copy.id)}
      <section>
        <div class="source">{copy.sourcePath}</div>
        <small>{copy.dirty ? 'Locally modified' : 'Unchanged'} · {new Date(copy.createdAt * 1000).toLocaleString()}</small>
        <div class="save-state" role="status">{cloudSaveLabel(copy.saveStatus)}</div>
        {#if copy.saveMessage}<p>{copy.saveMessage}</p>{/if}
        <div class="actions">
          <button type="button" disabled={uploading} on:click={() => void openCopy(copy)}>Open copy</button>
          <button type="button" on:click={() => void openCopy(copy, true)}>Show local folder</button>
          <button type="button" disabled={uploading || (!copy.dirty && !['error', 'manual'].includes(copy.saveStatus ?? 'manual'))} on:click={() => void saveOriginal(copy)}>Save to original</button>
          <button type="button" disabled={uploading || !copy.dirty} on:click={() => void upload(copy)}>Save as new file</button>
          {#if copy.saveStatus !== 'unsupported'}<button type="button" disabled={uploading} on:click={() => void toggleAutomatic(copy)}>{copy.autoSave ? 'Pause automatic saving' : 'Resume automatic saving'}</button>{/if}
        </div>
        {#if copy.uploadedPath}<small>Last uploaded: {copy.uploadedPath}</small>{/if}
      </section>
    {:else}
      <p>{loading ? 'Loading…' : 'No working copies yet.'}</p>
    {/each}
  </div>
  <div class="footer">
    <button type="button" on:click={() => void showStorage()}>Show storage folder</button>
    <button type="button" disabled={loading || uploading} on:click={() => void refresh()}>Refresh</button>
    <button type="button" disabled={uploading} on:click={() => { open = false }}>Close</button>
  </div>
</ModalShell>

<style>
  p { line-height: 1.5; }
  .copies { max-height: 45vh; overflow: auto; }
  section { padding: 12px 0; border-bottom: 1px solid var(--border); }
  .source { overflow-wrap: anywhere; margin-bottom: 4px; }
  small { color: var(--muted); }
  .actions, .footer { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }
  .footer { justify-content: flex-end; }
  .error { color: var(--danger); }
</style>
