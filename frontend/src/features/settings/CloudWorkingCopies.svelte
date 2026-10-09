<script lang="ts">
  import ModalShell from '@/shared/ui/ModalShell.svelte'
  import ConfirmActionModal from '@/shared/ui/ConfirmActionModal.svelte'
  import ComboBox, { type ComboOption } from '@/shared/ui/ComboBox.svelte'
  import { formatSize } from '@/shared/lib/formatSize'
  import { invoke } from '@/shared/lib/tauri'
  import { getErrorMessage } from '@/shared/lib/error'
  import { cloudWorkingCopyOverview, removeCloudWorkingCopies, uploadCloudWorkingCopy, type CloudWorkingCopy, type CloudWorkingCopyDetails } from '@/features/network'
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
  let copies: CloudWorkingCopyDetails[] = []
  let storageBytes = 0
  let storageIncomplete = false
  let retainedEntries = 0
  let cleanupIds: string[] = []
  let cleanupDescription = ''
  let cleanupSkipped: Array<{ id: string; sourcePath: string; reason: string }> = []
  const removed = new Set<string>()
  let uploading = false
  let message = ''
  let refreshId = 0
  const latest = new Map<string, CloudWritebackStatus>()
  const applyStatus = (copy: CloudWorkingCopyDetails) => {
    const row = latest.get(copy.id)
    return row ? { ...copy, saveStatus: row.status, saveMessage: row.message, ...(row.status === 'saved' ? { dirty: false } : row.status === 'pending' || row.status === 'uploading' ? { dirty: true } : {}) } : copy
  }

  onMount(() => {
    let disposed = false
    const unlisten: UnlistenFn[] = []
    void listen<string>('cloud-working-copy-removed', event => {
      if (disposed) return
      removed.add(event.payload)
      latest.delete(event.payload)
      copies = copies.filter(copy => copy.id !== event.payload)
      if (open && !uploading) void refresh()
    }).then(off => { if (disposed) off(); else unlisten.push(off) }).catch(() => {})
    void listen<CloudWritebackStatus>('cloud-writeback', event => {
      if (disposed || removed.has(event.payload.id)) return
      const row = event.payload
      if ((latest.get(row.id)?.sequence ?? -1) >= row.sequence) return
      latest.set(row.id, row)
      if (open) copies = copies.map(copy => copy.id === row.id ? applyStatus(copy) : copy)
    }).then(off => { if (disposed) off(); else unlisten.push(off) }).catch(() => {})
    return () => { disposed = true; for (const off of unlisten) off(); ++refreshId }
  })

  const canRemove = (copy: CloudWorkingCopyDetails) => !copy.dirty && !copy.cleanupBlockedReason
    && !['pending', 'uploading', 'conflict', 'error'].includes(copy.saveStatus ?? 'manual')
  $: savedCopies = copies.filter(copy => copy.saveStatus === 'saved' && canRemove(copy))
  const copyOptions = (copy: CloudWorkingCopyDetails): ComboOption[] => [
    { value: 'open', label: 'Open copy' },
    { value: 'folder', label: 'Show local folder' },
    { value: 'save', label: 'Save to original', disabled: !copy.dirty && !['error', 'manual'].includes(copy.saveStatus ?? 'manual') },
    { value: 'upload', label: 'Save as new file', disabled: !copy.dirty },
    { value: 'automatic', label: copy.autoSave ? 'Pause automatic saving' : 'Resume automatic saving', disabled: copy.saveStatus === 'unsupported' },
    { value: 'remove', label: 'Remove local copy…', disabled: !canRemove(copy) },
  ]
  const runCopyAction = (copy: CloudWorkingCopyDetails, action: string) => {
    if (uploading || loading || cleanupIds.length || copyOptions(copy).find(option => option.value === action)?.disabled) return
    switch (action) {
      case 'open': void openCopy(copy); break
      case 'folder': void openCopy(copy, true); break
      case 'save': void saveOriginal(copy); break
      case 'upload': void upload(copy); break
      case 'automatic': void toggleAutomatic(copy); break
      case 'remove': requestCleanup([copy]); break
    }
  }
  $: storageOptions = [
    { value: 'cleanup', label: 'Clean up saved copies…', disabled: !savedCopies.length },
    { value: 'storage', label: 'Show storage folder' },
    { value: 'refresh', label: 'Refresh' },
  ] satisfies ComboOption[]
  const runStorageAction = (action: string) => {
    if (uploading || loading || cleanupIds.length) return
    switch (action) {
      case 'cleanup': if (savedCopies.length) requestCleanup(savedCopies); break
      case 'storage': void showStorage(); break
      case 'refresh': void refresh(); break
    }
  }
  const requestCleanup = (selected: CloudWorkingCopyDetails[]) => {
    cleanupIds = selected.map(copy => copy.id)
    cleanupDescription = selected.length === 1 ? selected[0].sourcePath : `${selected.length} saved working copies`
  }
  const cleanup = async () => {
    if (uploading || !cleanupIds.length) return
    uploading = true; error = ''; message = ''; cleanupSkipped = []
    const sources = new Map(copies.map(copy => [copy.id, copy.sourcePath]))
    try {
      const result = await removeCloudWorkingCopies(cleanupIds, true)
      for (const id of result.removedIds) { removed.add(id); latest.delete(id) }
      copies = copies.filter(copy => !removed.has(copy.id))
      cleanupSkipped = result.skipped.map(row => ({ ...row, sourcePath: sources.get(row.id) ?? row.id }))
      message = `${result.removedIds.length} local working ${result.removedIds.length === 1 ? 'copy moved' : 'copies moved'} to the trash. Cloud files were kept.`
      await refresh()
    } catch (err) { error = getErrorMessage(err); await refresh() }
    finally { uploading = false; cleanupIds = [] }
  }

  const saveOriginal = async (copy: CloudWorkingCopy) => {
    if (uploading) return
    uploading = true; error = ''; message = ''
    const event = `cloud-edit-save-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      await activityApi?.start('Saving edited cloud file…', event, () => void activityApi?.requestCancel(event), { completeOnReply: true })
      const saved = await saveCloudWorkingCopy(copy.id, event)
      message = saved.dirty ? 'Saved to the original cloud file. Newer local edits are waiting to save.' : 'Saved to the original cloud file.'
      await refresh()
    } catch (err) { error = getErrorMessage(err); await refresh() }
    finally { uploading = false; activityApi?.clearNow(); await activityApi?.cleanup() }
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
      await activityApi?.start('Checking and uploading edited file…', event, () => void activityApi?.requestCancel(event), { completeOnReply: true })
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
      const next = await cloudWorkingCopyOverview()
      if (id === refreshId) {
        copies = next.copies.filter(copy => !removed.has(copy.id)).map(copy =>
          (latest.get(copy.id)?.sequence ?? -1) > (started.get(copy.id) ?? -1) ? applyStatus(copy) : copy)
        storageBytes = next.storageBytes
        storageIncomplete = next.incomplete
        retainedEntries = next.retainedEntries
      }
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
<ModalShell {open} title="Cloud working copies" onClose={() => { if (!uploading && !cleanupIds.length) open = false }} closeOnEscape={!uploading && !cleanupIds.length} closeOnOverlay={!uploading && !cleanupIds.length} modalWidth="760px">
  <p>Newly opened cloud files save changes back to the original automatically after the editor finishes saving.
    Keep Browsey running and wait for “Saved to cloud” before closing it.</p>
  <p>Conflicts stop automatic saving and keep your local edits. Earlier working copies stay manual until you choose otherwise.
    Local copies survive restart and cache clearing. Pause automatic saving before editing if you want to upload only a new file.</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if message}<p role="status">{message}</p>{/if}
  <p class="storage" role="status">Local storage: {storageIncomplete ? 'at least ' : ''}{formatSize(storageBytes)} of file contents.
    Removed copies go to the trash; empty the trash separately to reclaim disk space.</p>
  {#if retainedEntries}<p>{retainedEntries} staging or recovery {retainedEntries === 1 ? 'entry is' : 'entries are'} also included and kept. Use “Show storage folder” to inspect them.</p>{/if}
  {#if cleanupSkipped.length}
    <div role="alert"><p>These copies were kept:</p><ul>{#each cleanupSkipped as row}<li>{row.sourcePath}: {row.reason}</li>{/each}</ul></div>
  {/if}
  <div class="copies" aria-busy={loading}>
    {#each copies as copy (copy.id)}
      <section>
        <div class="copy-heading">
          <div class="copy-details">
            <div class="source">{copy.sourcePath}</div>
            <small>{copy.dirty ? 'Locally modified' : 'Unchanged'} · {formatSize(copy.storageBytes)} · {new Date(copy.createdAt * 1000).toLocaleString()}</small>
            <div class="save-state" role="status">{cloudSaveLabel(copy.saveStatus)}</div>
          </div>
          <div class="copy-actions">
            <ComboBox options={copyOptions(copy)} value="" placeholder="Actions…" ariaLabel={`Actions for ${copy.sourcePath}`}
              resetOnSelect fixedDropdown disabled={loading || uploading || cleanupIds.length > 0}
              on:change={event => runCopyAction(copy, event.detail)} />
          </div>
        </div>
        {#if copy.saveMessage}<p>{copy.saveMessage}</p>{/if}
        {#if copy.cleanupBlockedReason}<small>Kept: {copy.cleanupBlockedReason}</small>{/if}
        {#if copy.uploadedPath}<small>Last uploaded: {copy.uploadedPath}</small>{/if}
      </section>
    {:else}
      <p>{loading ? 'Loading…' : 'No working copies yet.'}</p>
    {/each}
  </div>
  <div slot="actions" class="footer">
    <div class="storage-actions">
      <ComboBox options={storageOptions} value="" placeholder="Storage actions…" ariaLabel="Storage actions"
        resetOnSelect fixedDropdown disabled={loading || uploading || cleanupIds.length > 0}
        on:change={event => runStorageAction(event.detail)} />
    </div>
    <button type="button" class="secondary" disabled={uploading || cleanupIds.length > 0} on:click={() => { open = false }}>Close</button>
  </div>
</ModalShell>
<ConfirmActionModal open={cleanupIds.length > 0} title="Remove local working copies?"
  message={`Close these files in your editor before continuing: ${cleanupDescription}. Browsey will recheck each copy and move only unchanged copies without save problems to the trash. Cloud files are kept.`}
  confirmLabel="Files closed — move to trash" danger busy={uploading}
  onConfirm={() => void cleanup()} onCancel={() => { cleanupIds = [] }} />

<style>
  p { line-height: 1.5; }
  .copies { max-height: 45vh; overflow: auto; }
  section { padding: 12px 0; border-bottom: 1px solid var(--border); }
  .source { overflow-wrap: anywhere; margin-bottom: 4px; }
  small { color: var(--muted); }
  .copy-heading { display: flex; align-items: flex-start; justify-content: space-between; flex-wrap: wrap; gap: 12px; }
  .copy-details { flex: 1; min-width: 180px; }
  .copy-actions { width: 140px; flex-shrink: 0; }
  .footer { display: flex; align-items: center; justify-content: flex-end; flex-wrap: wrap; gap: var(--modal-actions-gap); }
  .storage-actions { width: 180px; }
  .error { color: var(--danger); }
</style>
