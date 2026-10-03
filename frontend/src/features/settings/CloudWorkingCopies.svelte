<script lang="ts">
  import ModalShell from '@/shared/ui/ModalShell.svelte'
  import { invoke } from '@/shared/lib/tauri'
  import { getErrorMessage } from '@/shared/lib/error'
  import { listCloudWorkingCopies, uploadCloudWorkingCopy, type CloudWorkingCopy } from '@/features/network'
  import type { ActivityApi } from '@/features/explorer'

  export let activityApi: ActivityApi | null = null

  let open = false
  let loading = false
  let error = ''
  let copies: CloudWorkingCopy[] = []
  let uploading = false
  let message = ''

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
    loading = true
    error = ''
    try { copies = await listCloudWorkingCopies() }
    catch (err) { error = getErrorMessage(err) }
    finally { loading = false }
  }

  const openCopy = async (copy: CloudWorkingCopy, folder = false) => {
    error = ''
    try {
      await invoke('open_entry', { path: folder ? copy.localPath.replace(/[/\\][^/\\]+$/, '') : copy.localPath })
    } catch (err) { error = getErrorMessage(err) }
  }
</script>

<button type="button" class="secondary" on:click={() => { open = true; void refresh() }}>Working copies…</button>
<ModalShell {open} title="Cloud working copies" onClose={() => { if (!uploading) open = false }} closeOnEscape={!uploading} closeOnOverlay={!uploading} modalWidth="760px">
  <p>These local files survive restart and cache clearing. Changes are not uploaded automatically.
    Close your editor before uploading. Copies are kept until you remove them manually.</p>
  <p>Uploads use a unique new filename, never overwrite the cloud original, and require local disk space.
    Incomplete operations are also kept in the storage folder for manual recovery.</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}
  <div class="copies" aria-busy={loading}>
    {#each copies as copy (copy.id)}
      <section>
        <div class="source">{copy.sourcePath}</div>
        <small>{copy.dirty ? 'Locally modified' : 'Unchanged'} · {new Date(copy.createdAt * 1000).toLocaleString()}</small>
        <div class="actions">
          <button type="button" disabled={uploading} on:click={() => void openCopy(copy)}>Open copy</button>
          <button type="button" on:click={() => void openCopy(copy, true)}>Show local folder</button>
          <button type="button" disabled={uploading || !copy.dirty} on:click={() => void upload(copy)}>Upload changes as new file</button>
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
