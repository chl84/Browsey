<script lang="ts">
  import ModalShell from '../../ui/ModalShell.svelte'
  import ComboBox, { type ComboOption } from '../../ui/ComboBox.svelte'
  import { onMount, onDestroy } from 'svelte'

  export let open = false
  export let onClose: () => void

  let filter = ''
  let needle = ''

  type SortField = 'name' | 'size' | 'date'
  type SortDirection = 'asc' | 'desc'
  type Theme = 'system' | 'light' | 'dark'
  type Density = 'cozy' | 'compact'
  type LogLevel = 'error' | 'warn' | 'info' | 'debug'

  type Settings = {
    startDir: string
    defaultView: 'list' | 'grid'
    foldersFirst: boolean
    hiddenFilesLast: boolean
    showHidden: boolean
    confirmDelete: boolean
    sortField: SortField
    sortDirection: SortDirection
    theme: Theme
    density: Density
    iconSize: number
    archiveName: string
    archiveLevel: number
    openDestAfterExtract: boolean
    videoThumbs: boolean
    ffmpegPath: string
    thumbCacheMb: number
    thumbTimeoutMs: number
    watcherPollMs: number
    ioConcurrency: number
    lazyDirScan: boolean
    doubleClickMs: number
    singleClickOpen: boolean
    logLevel: LogLevel
    externalTools: string
    highContrast: boolean
    scrollbarWidth: number
  }

  const DEFAULT_SETTINGS: Settings = {
    startDir: '~',
    defaultView: 'list',
    foldersFirst: true,
    hiddenFilesLast: false,
    showHidden: true,
    confirmDelete: true,
    sortField: 'name',
    sortDirection: 'asc',
    theme: 'system',
    density: 'cozy',
    iconSize: 24,
    archiveName: 'Archive.zip',
    archiveLevel: 6,
    openDestAfterExtract: true,
    videoThumbs: true,
    ffmpegPath: '',
    thumbCacheMb: 200,
    thumbTimeoutMs: 3500,
    watcherPollMs: 2000,
    ioConcurrency: 4,
    lazyDirScan: true,
    doubleClickMs: 300,
    singleClickOpen: false,
    logLevel: 'warn',
    externalTools: '',
    highContrast: false,
    scrollbarWidth: 10,
  }

  let settings: Settings = { ...DEFAULT_SETTINGS }

  const onNumberInput = <K extends keyof Settings>(key: K) => (event: Event) => {
    const target = event.currentTarget as HTMLInputElement
    settings = { ...settings, [key]: Number(target.value) }
  }
  const shortcuts = [
    { action: 'Search', keys: 'Ctrl+F' },
    { action: 'Toggle view', keys: 'Ctrl+G' },
    { action: 'Bookmarks', keys: 'Ctrl+B' },
    { action: 'Open console', keys: 'Ctrl+T' },
    { action: 'Copy', keys: 'Ctrl+C' },
    { action: 'Cut', keys: 'Ctrl+X' },
    { action: 'Paste', keys: 'Ctrl+V' },
    { action: 'Select all', keys: 'Ctrl+A' },
    { action: 'Undo', keys: 'Ctrl+Z' },
    { action: 'Redo', keys: 'Ctrl+Y' },
    { action: 'Properties', keys: 'Ctrl+P' },
    { action: 'Show hidden', keys: 'Ctrl+H' },
    { action: 'Open settings', keys: 'Ctrl+S' },
    { action: 'Delete to wastebasket', keys: 'Delete' },
    { action: 'Delete permanently', keys: 'Shift+Delete' },
    { action: 'Rename', keys: 'F2' },
  ]

  $: needle = filter.trim().toLowerCase()
  const rowMatches = (...texts: (string | number | boolean | null | undefined)[]) => {
    if (!needle) return true
    return texts.some((t) => {
      if (t === null || t === undefined) return false
      return String(t).toLowerCase().includes(needle)
    })
  }
  const showRow = rowMatches

  $: filteredShortcuts = shortcuts.filter((s) => rowMatches(s.action, s.keys))

  $: showGeneral =
    rowMatches('general') ||
    showRow('Default view', settings.defaultView) ||
    showRow('Folders first') ||
    showRow('Show hidden') ||
    showRow('Hidden files last') ||
    showRow('Start directory', settings.startDir) ||
    showRow('Confirm delete')

  $: showSorting =
    rowMatches('sorting', 'sort') ||
    showRow('Sort field', settings.sortField) ||
    showRow('Sort direction', settings.sortDirection)

  $: showAppearance =
    rowMatches('appearance', 'theme', 'density', 'icon') ||
    showRow('Theme', settings.theme) ||
    showRow('Density', settings.density) ||
    showRow('Icon size', String(settings.iconSize))

  $: showArchives =
    rowMatches('archives', 'archive', 'zip', 'rar') ||
    showRow('Default archive name', settings.archiveName) ||
    showRow('ZIP level', String(settings.archiveLevel)) ||
    showRow('After extract', settings.openDestAfterExtract ? 'enabled' : 'disabled') ||
    showRow('RAR')

  $: showThumbnails =
    rowMatches('thumbnails', 'thumbs', 'ffmpeg', 'video') ||
    showRow('Video thumbs') ||
    showRow('FFmpeg path', settings.ffmpegPath) ||
    showRow('Thumbnail cache size', String(settings.thumbCacheMb)) ||
    showRow('Thumbnail timeout', String(settings.thumbTimeoutMs))

  $: showShortcuts = filteredShortcuts.length > 0 || rowMatches('shortcuts', 'keys')

  $: showPerformance =
    rowMatches('performance', 'watcher', 'poll', 'io', 'concurrency') ||
    showRow('Mounts poll', String(settings.watcherPollMs)) ||
    showRow('IO concurrency', String(settings.ioConcurrency)) ||
    showRow('Lazy scans')

  $: showInteraction =
    rowMatches('interaction', 'click') ||
    showRow('Double-click speed', String(settings.doubleClickMs)) ||
    showRow('Single click open', settings.singleClickOpen ? 'on' : 'off')

  $: showData =
    rowMatches('data', 'clear', 'cache', 'stars', 'bookmarks', 'recents') ||
    showRow('Clear thumbnail cache') ||
    showRow('Clear stars') ||
    showRow('Clear bookmarks') ||
    showRow('Clear recents')

  $: showAccessibility =
    rowMatches('accessibility', 'contrast', 'scrollbar') ||
    showRow('High contrast') ||
    showRow('Scrollbar width', String(settings.scrollbarWidth))

  $: showAdvanced =
    rowMatches('advanced', 'external tools', 'log') ||
    showRow('External tools', settings.externalTools) ||
    showRow('Log level', settings.logLevel)
  $: hiddenFilesLastDisabled = !settings.showHidden
  $: thumbsDisabled = !settings.videoThumbs

  const clearStore = (target: 'thumb-cache' | 'stars' | 'bookmarks' | 'recents') => {
    console.log(`TODO: clear ${target}`)
  }

  const restoreDefaults = () => {
    settings = { ...DEFAULT_SETTINGS }
    filter = ''
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
        <button type="button" class="restore-btn" on:click={restoreDefaults}>Restore defaults</button>
      </div>
    </svelte:fragment>

    <div class="settings-panel single">
      <div class="form-rows settings-table">
        {#if showGeneral}
          <div class="group-heading">General</div><div class="group-spacer"></div>
          {#if showRow('Default view', settings.defaultView)}
            <div class="form-label">Default view</div>
            <div class="form-control radios">
              <label class="radio">
                <input type="radio" name="default-view" value="list" bind:group={settings.defaultView} />
                <span>List</span>
              </label>
              <label class="radio">
                <input type="radio" name="default-view" value="grid" bind:group={settings.defaultView} />
                <span>Grid</span>
              </label>
            </div>
          {/if}

          {#if showRow('Folders first')}
            <div class="form-label">Folders first</div>
            <div class="form-control checkbox">
              <input type="checkbox" bind:checked={settings.foldersFirst} />
              <span>Show folders before files</span>
            </div>
          {/if}

          {#if showRow('Show hidden')}
            <div class="form-label">Show hidden</div>
            <div class="form-control checkbox">
              <input type="checkbox" bind:checked={settings.showHidden} />
              <span>Show hidden files by default</span>
            </div>
          {/if}

          {#if showRow('Hidden files last')}
            <div class="form-label">Hidden files last</div>
            <div class="form-control checkbox">
              <input type="checkbox" bind:checked={settings.hiddenFilesLast} disabled={hiddenFilesLastDisabled} />
              <span>Place hidden items at the end</span>
              {#if hiddenFilesLastDisabled}
                <small>Enable "Show hidden" to change this</small>
              {/if}
            </div>
          {/if}

          {#if showRow('Start directory', settings.startDir)}
            <div class="form-label">Start directory</div>
            <div class="form-control">
              <input type="text" bind:value={settings.startDir} placeholder="~ or /path" />
            </div>
          {/if}

          {#if showRow('Confirm delete')}
            <div class="form-label">Confirm delete</div>
            <div class="form-control checkbox">
              <input type="checkbox" bind:checked={settings.confirmDelete} />
              <span>Ask before permanent delete</span>
            </div>
          {/if}
        {/if}

        {#if showSorting}
          <div class="group-heading">Sorting</div><div class="group-spacer"></div>
          {#if showRow('Sort field', settings.sortField)}
            <div class="form-label">Default sort field</div>
            <div class="form-control">
              <ComboBox
                bind:value={settings.sortField}
                options={[
                  { value: 'name', label: 'Name' },
                  { value: 'size', label: 'Size' },
                  { value: 'date', label: 'Date modified' },
                ] satisfies ComboOption[]}
              />
            </div>
          {/if}

          {#if showRow('Sort direction', settings.sortDirection)}
            <div class="form-label">Sort direction</div>
            <div class="form-control radios">
              <label class="radio">
                <input type="radio" name="sort-direction" value="asc" bind:group={settings.sortDirection} />
                <span>Ascending</span>
              </label>
              <label class="radio">
                <input type="radio" name="sort-direction" value="desc" bind:group={settings.sortDirection} />
                <span>Descending</span>
              </label>
            </div>
          {/if}
        {/if}

        {#if showAppearance}
          <div class="group-heading">Appearance</div><div class="group-spacer"></div>
          {#if showRow('Theme', settings.theme)}
            <div class="form-label">Theme</div>
            <div class="form-control">
              <ComboBox
                bind:value={settings.theme}
                options={[
                  { value: 'system', label: 'System' },
                  { value: 'light', label: 'Light' },
                  { value: 'dark', label: 'Dark' },
                ] satisfies ComboOption[]}
              />
            </div>
          {/if}

          {#if showRow('Density', settings.density)}
            <div class="form-label">Density</div>
            <div class="form-control">
              <ComboBox
                bind:value={settings.density}
                options={[
                  { value: 'cozy', label: 'Cozy' },
                  { value: 'compact', label: 'Compact' },
                ] satisfies ComboOption[]}
              />
            </div>
          {/if}

          {#if showRow('Icon size', String(settings.iconSize))}
            <div class="form-label">Icon size</div>
            <div class="form-control">
              <input type="range" min="16" max="64" bind:value={settings.iconSize} />
              <small>{settings.iconSize}px</small>
            </div>
          {/if}
        {/if}

        {#if showArchives}
          <div class="group-heading">Archives</div><div class="group-spacer"></div>
          {#if showRow('Default archive name', settings.archiveName)}
            <div class="form-label">Default archive name</div>
            <div class="form-control">
              <input type="text" bind:value={settings.archiveName} />
            </div>
          {/if}

          {#if showRow('ZIP level', String(settings.archiveLevel))}
            <div class="form-label">ZIP level</div>
            <div class="form-control">
              <input type="range" min="0" max="9" step="1" bind:value={settings.archiveLevel} />
              <small>Level {settings.archiveLevel}</small>
            </div>
          {/if}

          {#if showRow('After extract', settings.openDestAfterExtract ? 'enabled' : 'disabled')}
            <div class="form-label">After extract</div>
            <div class="form-control checkbox">
              <input type="checkbox" bind:checked={settings.openDestAfterExtract} />
              <span>Open destination after extract</span>
            </div>
          {/if}

          {#if showRow('RAR')}
            <div class="form-label">Note</div>
            <div class="form-control">
              <p class="note">RAR compressed entries are currently unsupported (fail fast).</p>
            </div>
          {/if}
        {/if}

        {#if showThumbnails}
          <div class="group-heading">Thumbnails</div><div class="group-spacer"></div>
          {#if showRow('Video thumbs')}
            <div class="form-label">Video thumbs</div>
            <div class="form-control checkbox">
              <input type="checkbox" bind:checked={settings.videoThumbs} />
              <span>Enable video thumbnails (requires ffmpeg)</span>
            </div>
          {/if}

          {#if showRow('FFmpeg path', settings.ffmpegPath)}
            <div class="form-label">FFmpeg path</div>
            <div class="form-control">
              <input
                type="text"
                bind:value={settings.ffmpegPath}
                placeholder="auto-detect if empty"
                disabled={thumbsDisabled}
              />
            </div>
          {/if}

          {#if showRow('Thumbnail cache size', String(settings.thumbCacheMb))}
            <div class="form-label">Cache size</div>
            <div class="form-control">
              <input
                type="range"
                min="50"
                max="1000"
                step="50"
                value={settings.thumbCacheMb}
                on:input={onNumberInput('thumbCacheMb')}
                disabled={thumbsDisabled}
              />
              <small>{settings.thumbCacheMb} MB</small>
            </div>
          {/if}

          {#if showRow('Thumbnail timeout', String(settings.thumbTimeoutMs))}
            <div class="form-label">Timeout</div>
            <div class="form-control">
              <input
                type="range"
                min="500"
                max="10000"
                step="100"
                value={settings.thumbTimeoutMs}
                on:input={onNumberInput('thumbTimeoutMs')}
                disabled={thumbsDisabled}
              />
              <small>{settings.thumbTimeoutMs} ms</small>
            </div>
          {/if}
        {/if}

        {#if showShortcuts}
          <div class="group-heading">Shortcuts</div><div class="group-spacer"></div>
          {#each filteredShortcuts as shortcut (shortcut.action)}
            <div class="form-label">{shortcut.action}</div>
            <div class="form-control"><span class="key">{shortcut.keys}</span></div>
          {/each}
          <div class="form-label"></div>
          <div class="form-control">
            <button type="button" class="secondary">Edit shortcuts (coming soon)</button>
          </div>
        {/if}

        {#if showPerformance}
          <div class="group-heading">Performance</div><div class="group-spacer"></div>
          {#if showRow('Mounts poll', String(settings.watcherPollMs))}
            <div class="form-label">Mounts poll (ms)</div>
            <div class="form-control">
              <input
                type="range"
                min="500"
                max="10000"
                step="100"
                value={settings.watcherPollMs}
                on:input={onNumberInput('watcherPollMs')}
              />
              <small>{settings.watcherPollMs} ms</small>
            </div>
          {/if}

          {#if showRow('IO concurrency', String(settings.ioConcurrency))}
            <div class="form-label">IO concurrency</div>
            <div class="form-control">
              <input
                type="range"
                min="1"
                max="16"
                step="1"
                value={settings.ioConcurrency}
                on:input={onNumberInput('ioConcurrency')}
              />
              <small>{settings.ioConcurrency} workers</small>
            </div>
          {/if}

          {#if showRow('Lazy scans')}
            <div class="form-label">Lazy scans</div>
            <div class="form-control checkbox">
              <input type="checkbox" bind:checked={settings.lazyDirScan} />
              <span>Defer deep scans in large folders</span>
            </div>
          {/if}
        {/if}

        {#if showInteraction}
          <div class="group-heading">Interaction</div><div class="group-spacer"></div>
          {#if showRow('Double-click speed', String(settings.doubleClickMs))}
            <div class="form-label">Double-click speed</div>
            <div class="form-control">
              <input
                type="range"
                min="150"
                max="600"
                step="10"
                value={settings.doubleClickMs}
                on:input={onNumberInput('doubleClickMs')}
              />
              <small>{settings.doubleClickMs} ms</small>
            </div>
          {/if}

          {#if showRow('Single click open', settings.singleClickOpen ? 'on' : 'off')}
            <div class="form-label">Single click to open</div>
            <div class="form-control checkbox">
              <input type="checkbox" bind:checked={settings.singleClickOpen} />
              <span>Open items on single click</span>
            </div>
          {/if}
        {/if}

        {#if showData}
          <div class="group-heading">Data</div><div class="group-spacer"></div>
          <div class="form-label">Clear thumbnail cache</div>
          <div class="form-control">
            <button type="button" class="secondary" on:click={() => clearStore('thumb-cache')}>Clear</button>
          </div>
          <div class="form-label">Clear stars</div>
          <div class="form-control">
            <button type="button" class="secondary" on:click={() => clearStore('stars')}>Clear</button>
          </div>
          <div class="form-label">Clear bookmarks</div>
          <div class="form-control">
            <button type="button" class="secondary" on:click={() => clearStore('bookmarks')}>Clear</button>
          </div>
          <div class="form-label">Clear recents</div>
          <div class="form-control">
            <button type="button" class="secondary" on:click={() => clearStore('recents')}>Clear</button>
          </div>
        {/if}

        {#if showAccessibility}
          <div class="group-heading">Accessibility</div><div class="group-spacer"></div>
          {#if showRow('High contrast')}
            <div class="form-label">High contrast</div>
            <div class="form-control checkbox">
              <input type="checkbox" bind:checked={settings.highContrast} />
              <span>Boost contrast for UI elements</span>
            </div>
          {/if}

          {#if showRow('Scrollbar width', String(settings.scrollbarWidth))}
            <div class="form-label">Scrollbar width</div>
            <div class="form-control">
              <input
                type="range"
                min="6"
                max="16"
                step="1"
                value={settings.scrollbarWidth}
                on:input={onNumberInput('scrollbarWidth')}
              />
              <small>{settings.scrollbarWidth} px</small>
            </div>
          {/if}
        {/if}

        {#if showAdvanced}
          <div class="group-heading">Advanced</div><div class="group-spacer"></div>
          {#if showRow('External tools', settings.externalTools)}
            <div class="form-label">External tools</div>
            <div class="form-control column">
              <textarea rows="2" bind:value={settings.externalTools} placeholder="ffmpeg=/usr/bin/ffmpeg"></textarea>
            </div>
          {/if}

          {#if showRow('Log level', settings.logLevel)}
            <div class="form-label">Log level</div>
            <div class="form-control">
              <ComboBox
                bind:value={settings.logLevel}
                options={[
                  { value: 'error', label: 'Error' },
                  { value: 'warn', label: 'Warn' },
                  { value: 'info', label: 'Info' },
                  { value: 'debug', label: 'Debug' },
                ] satisfies ComboOption[]}
              />
            </div>
          {/if}
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
    justify-content: flex-start;
    gap: 8px;
    flex-wrap: wrap;
    margin-bottom: 12px;
  }

  .settings-header h2 {
    margin: 0;
    font-size: 18px;
    line-height: 1.4;
    flex: 0 1 auto;
  }

  .settings-filter {
    min-width: 200px;
    width: 240px;
  }

.restore-btn {
    padding: 8px 10px;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    cursor: pointer;
    font-size: 13px;
    margin-left: 0;
}

  .restore-btn:hover {
    background: var(--bg-hover);
    border-color: var(--border-accent);
  }

  .settings-panel {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-right: 8px;
  }

  .settings-panel.single {
    padding-right: 8px;
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
    grid-template-columns: 150px 1fr;
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
