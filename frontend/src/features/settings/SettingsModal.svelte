<script lang="ts">
  import ModalShell from '../../ui/ModalShell.svelte'
  import ComboBox, { type ComboOption } from '../../ui/ComboBox.svelte'
  import { onMount, onDestroy } from 'svelte'

  export let open = false
  export let onClose: () => void

  let filter = ''
  let needle = ''

  // Local placeholder state (not wired to settings yet).
  let startDir = '~'
  let defaultView: 'list' | 'grid' = 'list'
  let foldersFirst = true
  let hiddenFilesLast = false
  let showHidden = true
  let confirmDelete = true

  let theme: 'system' | 'light' | 'dark' = 'system'
  let density: 'cozy' | 'compact' = 'cozy'
  let iconSize = 24

  let archiveName = 'Archive.zip'
  let archiveLevel = 6
  let openDestAfterExtract = true

  let videoThumbs = true
  let ffmpegPath = ''
  let thumbCacheMb = 200
  let thumbTimeoutMs = 3500

  let watcherPollMs = 2000
  let ioConcurrency = 4
  let lazyDirScan = true

  let logLevel: 'error' | 'warn' | 'info' | 'debug' = 'warn'
  let externalTools = ''

  $: needle = filter.trim().toLowerCase()
  const showRow = (...texts: (string | number | boolean | null | undefined)[]) => {
    if (!needle) return true
    return texts.some((t) => {
      if (t === null || t === undefined) return false
      return String(t).toLowerCase().includes(needle)
    })
  }

  const handleWindowKeydown = (e: KeyboardEvent) => {
    if (!open) return
    if (e.key === 'Escape') {
      e.preventDefault()
      onClose()
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleWindowKeydown, { capture: true })
  })

  onDestroy(() => {
    window.removeEventListener('keydown', handleWindowKeydown, { capture: true } as any)
  })
</script>

{#if open}
  <ModalShell
    title={null}
    open={open}
    onClose={onClose}
    modalClass="settings-modal"
    modalWidth="720px"
    initialFocusSelector=".settings-filter"
  >
    <svelte:fragment slot="header">
      <div class="settings-header">
        <h2>Settings</h2>
        <input
          class="settings-filter"
          type="search"
          placeholder="Filter settings"
          bind:value={filter}
        />
      </div>
    </svelte:fragment>

    <div class="settings-panel single">
      <div class="form-rows settings-table">
        <div class="group-heading">General</div><div class="group-spacer"></div>
        {#if showRow('Default view', defaultView)}
          <div class="form-label">Default view</div>
          <div class="form-control radios">
            <label class="radio">
              <input type="radio" name="default-view" value="list" bind:group={defaultView} />
              <span>List</span>
            </label>
            <label class="radio">
              <input type="radio" name="default-view" value="grid" bind:group={defaultView} />
              <span>Grid</span>
            </label>
          </div>
        {/if}

        {#if showRow('Folders first')}
          <div class="form-label">Folders first</div>
          <div class="form-control checkbox">
            <input type="checkbox" bind:checked={foldersFirst} />
            <span>Show folders before files</span>
          </div>
        {/if}

        {#if showRow('Show hidden')}
          <div class="form-label">Show hidden</div>
          <div class="form-control checkbox">
            <input type="checkbox" bind:checked={showHidden} />
            <span>Show hidden files by default</span>
          </div>
        {/if}

        {#if showRow('Hidden files last')}
          <div class="form-label">Hidden files last</div>
          <div class="form-control checkbox">
            <input type="checkbox" bind:checked={hiddenFilesLast} />
            <span>Place hidden items at the end</span>
          </div>
        {/if}

        {#if showRow('Start directory', startDir)}
          <div class="form-label">Start directory</div>
          <div class="form-control">
            <input type="text" bind:value={startDir} placeholder="~ or /path" />
          </div>
        {/if}

        {#if showRow('Confirm delete')}
          <div class="form-label">Confirm delete</div>
          <div class="form-control checkbox">
            <input type="checkbox" bind:checked={confirmDelete} />
            <span>Ask before delete/permanent delete</span>
          </div>
        {/if}

        <div class="group-heading">Appearance</div><div class="group-spacer"></div>
        {#if showRow('Theme', theme)}
          <div class="form-label">Theme</div>
          <div class="form-control">
            <ComboBox
              bind:value={theme}
              options={[
                { value: 'system', label: 'System' },
                { value: 'light', label: 'Light' },
                { value: 'dark', label: 'Dark' },
              ] satisfies ComboOption[]}
            />
          </div>
        {/if}

        {#if showRow('Density', density)}
          <div class="form-label">Density</div>
          <div class="form-control">
            <ComboBox
              bind:value={density}
              options={[
                { value: 'cozy', label: 'Cozy' },
                { value: 'compact', label: 'Compact' },
              ] satisfies ComboOption[]}
            />
          </div>
        {/if}

        {#if showRow('Icon size', String(iconSize))}
          <div class="form-label">Icon size</div>
          <div class="form-control">
            <input type="range" min="16" max="64" bind:value={iconSize} />
            <small>{iconSize}px</small>
          </div>
        {/if}

        <div class="group-heading">Archives</div><div class="group-spacer"></div>
        {#if showRow('Default archive name', archiveName)}
          <div class="form-label">Default archive name</div>
          <div class="form-control">
            <input type="text" bind:value={archiveName} />
          </div>
        {/if}

        {#if showRow('ZIP level', String(archiveLevel))}
          <div class="form-label">ZIP level</div>
          <div class="form-control">
            <input type="range" min="0" max="9" step="1" bind:value={archiveLevel} />
            <small>Level {archiveLevel}</small>
          </div>
        {/if}

        {#if showRow('After extract', openDestAfterExtract ? 'enabled' : 'disabled')}
          <div class="form-label">After extract</div>
          <div class="form-control checkbox">
            <input type="checkbox" bind:checked={openDestAfterExtract} />
            <span>Open destination after extract</span>
          </div>
        {/if}

        {#if showRow('RAR')}
          <div class="form-label">Note</div>
          <div class="form-control">
            <p class="note">RAR compressed entries are currently unsupported (fail fast).</p>
          </div>
        {/if}

        <div class="group-heading">Thumbnails</div><div class="group-spacer"></div>
        {#if showRow('Video thumbs')}
          <div class="form-label">Video thumbs</div>
          <div class="form-control checkbox">
            <input type="checkbox" bind:checked={videoThumbs} />
            <span>Enable video thumbnails (requires ffmpeg)</span>
          </div>
        {/if}

        {#if showRow('FFmpeg path', ffmpegPath)}
          <div class="form-label">FFmpeg path</div>
          <div class="form-control">
            <input type="text" bind:value={ffmpegPath} placeholder="auto-detect if empty" />
          </div>
        {/if}

        {#if showRow('Thumbnail cache size', String(thumbCacheMb))}
          <div class="form-label">Cache size</div>
          <div class="form-control">
            <input type="range" min="50" max="1000" step="50" bind:value={thumbCacheMb} />
            <small>{thumbCacheMb} MB</small>
          </div>
        {/if}

        {#if showRow('Thumbnail timeout', String(thumbTimeoutMs))}
          <div class="form-label">Timeout</div>
          <div class="form-control">
            <input type="range" min="500" max="10000" step="100" bind:value={thumbTimeoutMs} />
            <small>{thumbTimeoutMs} ms</small>
          </div>
        {/if}

        <div class="group-heading">Shortcuts</div><div class="group-spacer"></div>
        <div class="form-label">Search</div>
        <div class="form-control"><span class="key">Ctrl+F</span></div>
        <div class="form-label">Toggle view</div>
        <div class="form-control"><span class="key">Ctrl+G</span></div>
        <div class="form-label">Open settings</div>
        <div class="form-control"><span class="key">Ctrl+S</span></div>
        <div class="form-label">Properties</div>
        <div class="form-control"><span class="key">Ctrl+P</span></div>
        <div class="form-label">Show hidden</div>
        <div class="form-control"><span class="key">Ctrl+H</span></div>
        <div class="form-label"></div>
        <div class="form-control">
          <button type="button" class="secondary">Edit shortcuts (coming soon)</button>
        </div>

        <div class="group-heading">Performance</div><div class="group-spacer"></div>
        {#if showRow('Mounts poll', String(watcherPollMs))}
          <div class="form-label">Mounts poll (ms)</div>
          <div class="form-control">
            <input type="range" min="500" max="10000" step="100" bind:value={watcherPollMs} />
            <small>{watcherPollMs} ms</small>
          </div>
        {/if}

        {#if showRow('IO concurrency', String(ioConcurrency))}
          <div class="form-label">IO concurrency</div>
          <div class="form-control">
            <input type="range" min="1" max="16" step="1" bind:value={ioConcurrency} />
            <small>{ioConcurrency} workers</small>
          </div>
        {/if}

        {#if showRow('Lazy scans')}
          <div class="form-label">Lazy scans</div>
          <div class="form-control checkbox">
            <input type="checkbox" bind:checked={lazyDirScan} />
            <span>Defer deep scans in large folders</span>
          </div>
        {/if}

        <div class="group-heading">Advanced</div><div class="group-spacer"></div>
        {#if showRow('External tools', externalTools)}
          <div class="form-label">External tools</div>
          <div class="form-control column">
            <textarea rows="2" bind:value={externalTools} placeholder="ffmpeg=/usr/bin/ffmpeg"></textarea>
          </div>
        {/if}

        {#if showRow('Log level', logLevel)}
          <div class="form-label">Log level</div>
          <div class="form-control">
            <ComboBox
              bind:value={logLevel}
              options={[
                { value: 'error', label: 'Error' },
                { value: 'warn', label: 'Warn' },
                { value: 'info', label: 'Info' },
                { value: 'debug', label: 'Debug' },
              ] satisfies ComboOption[]}
            />
          </div>
        {/if}
      </div>
    </div>

  </ModalShell>
{/if}

<style>
  /* Inherits global modal styles; light tweaks for tabs and spacing */
  :global(.settings-modal) {
    max-height: 80vh;
    min-height: 420px;
    display: flex;
    flex-direction: column;
  }

  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  .settings-header h2 {
    margin: 0;
    font-size: 18px;
    line-height: 1.4;
  }

  .settings-filter {
    min-width: 200px;
  }

  .settings-panel {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-right: 2px;
  }

  .settings-panel.single {
    padding-right: 2px;
  }

  .group-heading {
    grid-column: 1;
    font-weight: 700;
    color: var(--fg-strong);
    padding-top: 6px;
    text-align: right;
    justify-self: end;
  }

  .group-spacer {
    grid-column: 2;
  }

  .form-rows {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 10px 32px;
    align-items: center;
  }

  .form-label {
    text-align: right;
    font-weight: 600;
    color: var(--fg-muted);
    line-height: 1.4;
  }

  .form-control {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
  }

  .form-control.column {
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
  }

  .form-control input[type='text'],
  .form-control textarea,
  .form-control :global(.combo) {
    width: 100%;
    max-width: 100%;
  }

  .form-control input[type='range'] {
    flex: 1;
  }

  .form-control small {
    color: var(--fg-muted);
  }

  .form-control.checkbox {
    align-items: center;
  }

  .form-control.checkbox span {
    color: var(--fg);
  }

  .form-control.radios {
    gap: 16px;
  }

  .form-control.radios .radio {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    color: var(--fg);
  }

  .key {
    display: inline-flex;
    min-width: 64px;
    justify-content: center;
    padding: 4px 8px;
    border: 1px solid var(--border);
    background: var(--bg);
    border-radius: 0;
    font-size: 12px;
  }
</style>
