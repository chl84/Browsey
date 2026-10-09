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
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="folders" role="group" aria-label="Destination folders">
    {#if busy}<p class="muted" role="status">Loading folders…</p>
    {:else}
      {#each folders as folder (folder.path)}
        <button type="button" class="folder" disabled={disabled} on:click={() => void browse(folder.path)}>{folder.name}<span aria-hidden="true">›</span></button>
      {/each}
      {#if !folders.length && current && !error}<p class="muted">No subfolders.</p>{/if}
    {/if}
  </div>
</div>

<style>
  .folder-destination { min-width: 0; }
  .path-row { display: flex; gap: var(--settings-control-gap); align-items: center; }
  .path-row :global(input) { flex: 1; }
  .folders { margin-top: var(--settings-control-gap); max-height: 200px; overflow: auto; border: 1px solid var(--border); }
  .folder { width: 100%; display: flex; justify-content: space-between; gap: 12px; text-align: left; padding: 8px 12px; overflow-wrap: anywhere; background: transparent; border: 0; color: var(--fg); cursor: pointer; }
  .folder:hover, .folder:focus-visible { background: var(--bg-raised); }
  p { margin: 8px 12px; }
</style>
