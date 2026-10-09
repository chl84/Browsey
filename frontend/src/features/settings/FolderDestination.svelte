<script lang="ts">
  import { homeDir } from '@tauri-apps/api/path'
  import { onMount, onDestroy } from 'svelte'
  import { invoke } from '@/shared/lib/tauri'
  import { getErrorMessage } from '@/shared/lib/error'
  import TextField from '@/shared/ui/TextField.svelte'
  import { parentPath } from '@/features/explorer'

  export let value = ''
  export let disabled = false
  let path = ''
  let current = ''
  let folders: { name: string; path: string; kind: string }[] = []
  let busy = false
  let error = ''
  let request = 0
  let disposed = false

  const browse = async (next: string) => {
    if (disabled || disposed) return
    const id = ++request
    busy = true; error = ''; value = ''
    try {
      const listing = await invoke<{ current: string; entries: typeof folders }>('list_dir', {
        path: next, sort: { field: 'name', direction: 'asc' }, forceRefresh: true,
      })
      if (disposed || id !== request) return
      if (listing.current.startsWith('rclone://')) throw new Error('Choose a local folder for recovery.')
      current = listing.current; path = current; value = current
      folders = listing.entries.filter(entry => entry.kind === 'dir')
    } catch (err) { if (!disposed && id === request) error = getErrorMessage(err) }
    finally { if (!disposed && id === request) busy = false }
  }
  onMount(() => {
    void homeDir().then(home => { if (!disposed) return browse(home) })
      .catch(err => { if (!disposed) error = getErrorMessage(err) })
  })
  onDestroy(() => { disposed = true; ++request })
</script>

<div class="folder-destination" aria-busy={busy}>
  <div class="path-row">
    <button type="button" class="secondary" aria-label="Parent folder" disabled={disabled || busy || !current || parentPath(current) === current} on:click={() => void browse(parentPath(current))}>↑</button>
    <TextField bind:value={path} aria-label="Destination folder" disabled={disabled || busy}
      on:input={() => { value = '' }} on:keydown={(event) => { if (event.key === 'Enter') { event.preventDefault(); void browse(path) } }} />
    <button type="button" class="secondary" disabled={disabled || busy || !path} on:click={() => void browse(path)}>Go</button>
  </div>
  {#if error}<div class="pill error" role="alert">{error}</div>{/if}
  <div class="folders modal-scroll" role="group" aria-label="Destination folders">
    {#if busy}<p class="muted" role="status">Loading folders…</p>
    {:else}
      {#each folders as folder (folder.path)}
        <button type="button" class="folder secondary" disabled={disabled} on:click={() => void browse(folder.path)}>{folder.name}<span aria-hidden="true">›</span></button>
      {/each}
      {#if !folders.length && current && !error}<p class="muted">No subfolders.</p>{/if}
    {/if}
  </div>
</div>

<style>
  .folder-destination { display: flex; flex-direction: column; gap: var(--modal-field-gap); min-width: 0; }
  .path-row {
    --modal-button-padding-y: var(--modal-input-padding-y);
    --modal-button-min-height: var(--modal-input-min-height);
    display: flex;
    gap: var(--modal-field-gap);
    align-items: center;
  }
  .path-row :global(input) { flex: 1; }
  .folders { display: flex; flex-direction: column; gap: var(--modal-field-gap); padding-block: var(--modal-field-gap); padding-inline-start: var(--modal-field-gap); max-height: 200px; border: 1px solid var(--border); }
  .folder { width: 100%; display: flex; justify-content: space-between; gap: var(--modal-field-gap); text-align: left; overflow-wrap: anywhere; }
</style>
