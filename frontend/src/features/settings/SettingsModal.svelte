<script lang="ts">
  import ModalShell from '../../ui/ModalShell.svelte'
  import { onMount, onDestroy } from 'svelte'

  export let open = false
  export let onClose: () => void

  const tabs = [
    { id: 'general', label: 'General' },
    { id: 'appearance', label: 'Appearance' },
    { id: 'archives', label: 'Archives' },
    { id: 'thumbnails', label: 'Thumbnails' },
    { id: 'shortcuts', label: 'Shortcuts' },
    { id: 'performance', label: 'Performance' },
    { id: 'advanced', label: 'Advanced' },
  ] as const

  type TabId = (typeof tabs)[number]['id']
  let activeTab: TabId = 'general'
  let wasOpen = false

  // Local placeholder state (not wired to settings yet).
  let startDir = '~'
  let defaultView: 'list' | 'grid' = 'list'
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

  $: {
    if (open && !wasOpen) {
      activeTab = 'general'
    }
    wasOpen = open
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
    title="Settings"
    open={open}
    onClose={onClose}
    modalClass="settings-modal"
    modalWidth="720px"
    initialFocusSelector=".tabs button"
  >
    <div class="tabs">
      {#each tabs as tab}
        <button
          type="button"
          class:selected={activeTab === tab.id}
          on:click={() => (activeTab = tab.id)}
        >
          {tab.label}
        </button>
      {/each}
    </div>

    {#if activeTab === 'general'}
      <section class="section">
        <h3>General</h3>
        <div class="form-grid">
          <label>
            <span>Default view</span>
            <select bind:value={defaultView}>
              <option value="list">List</option>
              <option value="grid">Grid</option>
            </select>
          </label>
          <label>
            <span>Start directory</span>
            <input type="text" bind:value={startDir} placeholder="~ or /path" />
          </label>
          <label class="inline">
            <input type="checkbox" bind:checked={showHidden} />
            <span>Show hidden files by default</span>
          </label>
          <label class="inline">
            <input type="checkbox" bind:checked={confirmDelete} />
            <span>Ask before delete/permanent delete</span>
          </label>
        </div>
      </section>
    {:else if activeTab === 'appearance'}
      <section class="section">
        <h3>Appearance</h3>
        <div class="form-grid">
          <label>
            <span>Theme</span>
            <select bind:value={theme}>
              <option value="system">System</option>
              <option value="light">Light</option>
              <option value="dark">Dark</option>
            </select>
          </label>
          <label>
            <span>Density</span>
            <select bind:value={density}>
              <option value="cozy">Cozy</option>
              <option value="compact">Compact</option>
            </select>
          </label>
          <label>
            <span>Icon size</span>
            <input type="range" min="16" max="64" bind:value={iconSize} />
            <small>{iconSize}px</small>
          </label>
        </div>
      </section>
    {:else if activeTab === 'archives'}
      <section class="section">
        <h3>Archives</h3>
        <div class="form-grid">
          <label>
            <span>Default archive name</span>
            <input type="text" bind:value={archiveName} />
          </label>
          <label>
            <span>ZIP level</span>
            <input type="number" min="0" max="9" step="1" bind:value={archiveLevel} />
          </label>
          <label class="inline">
            <input type="checkbox" bind:checked={openDestAfterExtract} />
            <span>Open destination after extract</span>
          </label>
          <p class="note">RAR compressed entries are currently unsupported (fail fast).</p>
        </div>
      </section>
    {:else if activeTab === 'thumbnails'}
      <section class="section">
        <h3>Thumbnails</h3>
        <div class="form-grid">
          <label class="inline">
            <input type="checkbox" bind:checked={videoThumbs} />
            <span>Enable video thumbnails (requires ffmpeg)</span>
          </label>
          <label>
            <span>FFmpeg path (optional)</span>
            <input type="text" bind:value={ffmpegPath} placeholder="auto-detect if empty" />
          </label>
          <label>
            <span>Thumbnail cache size (MB)</span>
            <input type="number" min="50" max="1000" step="50" bind:value={thumbCacheMb} />
          </label>
          <label>
            <span>Thumbnail timeout (ms)</span>
            <input type="number" min="500" max="10000" step="100" bind:value={thumbTimeoutMs} />
          </label>
        </div>
      </section>
    {:else if activeTab === 'shortcuts'}
      <section class="section">
        <h3>Shortcuts</h3>
        <div class="shortcuts-list">
          <div class="row"><span class="key">Ctrl+F</span><span>Search</span></div>
          <div class="row"><span class="key">Ctrl+G</span><span>Toggle view</span></div>
          <div class="row"><span class="key">Ctrl+S</span><span>Open settings</span></div>
          <div class="row"><span class="key">Ctrl+P</span><span>Properties</span></div>
          <div class="row"><span class="key">Ctrl+H</span><span>Show hidden</span></div>
          <button type="button" class="secondary">Edit shortcuts (coming soon)</button>
        </div>
      </section>
    {:else if activeTab === 'performance'}
      <section class="section">
        <h3>Performance</h3>
        <div class="form-grid">
          <label>
            <span>Mounts poll interval (ms)</span>
            <input type="number" min="500" max="10000" step="100" bind:value={watcherPollMs} />
          </label>
          <label>
            <span>IO concurrency</span>
            <input type="number" min="1" max="16" bind:value={ioConcurrency} />
          </label>
          <label class="inline">
            <input type="checkbox" bind:checked={lazyDirScan} />
            <span>Defer deep scans in large folders</span>
          </label>
        </div>
      </section>
    {:else if activeTab === 'advanced'}
      <section class="section">
        <h3>Advanced</h3>
        <div class="form-grid">
          <label>
            <span>External tools</span>
            <textarea rows="2" bind:value={externalTools} placeholder="ffmpeg=/usr/bin/ffmpeg"></textarea>
          </label>
          <label>
            <span>Log level</span>
            <select bind:value={logLevel}>
              <option value="error">Error</option>
              <option value="warn">Warn</option>
              <option value="info">Info</option>
              <option value="debug">Debug</option>
            </select>
          </label>
        </div>
      </section>
    {/if}

    <div slot="actions">
      <button type="button" on:click={onClose}>Close</button>
    </div>
  </ModalShell>
{/if}

<style>
  /* Inherits global modal styles; light tweaks for tabs and spacing */
  :global(.settings-modal) {
    max-height: 80vh;
    min-height: 420px;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .section h3 {
    margin: 0;
    font-size: 14px;
  }

  .section p {
    margin: 0;
    color: var(--fg-dim, #888);
    font-size: 13px;
  }
</style>
