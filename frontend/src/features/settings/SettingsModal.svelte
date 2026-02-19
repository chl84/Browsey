<script lang="ts">
  import ModalShell from '../../shared/ui/ModalShell.svelte'
  import ConfirmActionModal from '../../shared/ui/ConfirmActionModal.svelte'
  import ComboBox, { type ComboOption } from '../../shared/ui/ComboBox.svelte'
  import { getErrorMessage } from '@/shared/lib/error'
  import { onMount, onDestroy } from 'svelte'
  import type { DefaultSortField, Density } from '../explorer/types'
  import {
    keyboardEventToAccelerator,
    type ShortcutBinding,
    type ShortcutCommandId,
  } from '../shortcuts/keymap'

  export let open = false
  export let onClose: () => void
  export let initialFilter = ''
  export let defaultViewValue: 'list' | 'grid' = 'list'
  export let showHiddenValue = true
  export let hiddenFilesLastValue = false
  export let foldersFirstValue = true
  export let confirmDeleteValue = true
  export let densityValue: Density = 'cozy'
  export let archiveNameValue = 'Archive'
  export let archiveLevelValue = 6
  export let openDestAfterExtractValue = true
  export let videoThumbsValue = true
  export let hardwareAccelerationValue = true
  export let ffmpegPathValue = ''
  export let thumbCacheMbValue = 300
  export let mountsPollMsValue = 8000
  export let doubleClickMsValue = 300
  export let sortFieldValue: DefaultSortField = 'name'
  export let sortDirectionValue: 'asc' | 'desc' = 'asc'
  export let startDirValue = '~'
  export let shortcuts: ShortcutBinding[] = []
  export let onChangeDefaultView: (value: 'list' | 'grid') => void = () => {}
  export let onToggleShowHidden: (value: boolean) => void = () => {}
  export let onToggleHiddenFilesLast: (value: boolean) => void = () => {}
  export let onToggleFoldersFirst: (value: boolean) => void = () => {}
  export let onToggleConfirmDelete: (value: boolean) => void = () => {}
  export let onChangeSortField: (value: typeof sortFieldValue) => void = () => {}
  export let onChangeSortDirection: (value: typeof sortDirectionValue) => void = () => {}
  export let onChangeStartDir: (value: string) => void = () => {}
  export let onChangeDensity: (value: Density) => void = () => {}
  export let onChangeArchiveName: (value: string) => void = () => {}
  export let onChangeArchiveLevel: (value: number) => void = () => {}
  export let onToggleOpenDestAfterExtract: (value: boolean) => void = () => {}
  export let onToggleVideoThumbs: (value: boolean) => void = () => {}
  export let onToggleHardwareAcceleration: (value: boolean) => void = () => {}
  export let onChangeFfmpegPath: (value: string) => void = () => {}
  export let onChangeThumbCacheMb: (value: number) => void = () => {}
  export let onChangeMountsPollMs: (value: number) => void = () => {}
  export let onChangeDoubleClickMs: (value: number) => void = () => {}
  export let onClearThumbCache: () => Promise<void> | void = () => {}
  export let onClearStars: () => Promise<void> | void = () => {}
  export let onClearBookmarks: () => Promise<void> | void = () => {}
  export let onClearRecents: () => Promise<void> | void = () => {}
  export let onChangeShortcut: (
    commandId: ShortcutCommandId,
    accelerator: string,
  ) => Promise<void> | void = () => {}

  let filter = ''
  let seededInitialFilter = false
  let needle = ''

  type SortField = DefaultSortField
  type SortDirection = 'asc' | 'desc'
  type LogLevel = 'error' | 'warn' | 'info' | 'debug'
  type DataClearTarget = 'thumb-cache' | 'stars' | 'bookmarks' | 'recents'

  type Settings = {
    startDir: string
    defaultView: 'list' | 'grid'
    foldersFirst: boolean
    hiddenFilesLast: boolean
    showHidden: boolean
    confirmDelete: boolean
    sortField: SortField
    sortDirection: SortDirection
    density: Density
    iconSize: number
    archiveName: string
    archiveLevel: number
    openDestAfterExtract: boolean
    videoThumbs: boolean
    hardwareAcceleration: boolean
    ffmpegPath: string
    thumbCacheMb: number
    mountsPollMs: number
    doubleClickMs: number
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
    density: 'cozy',
    iconSize: 24,
    archiveName: 'Archive',
    archiveLevel: 6,
    openDestAfterExtract: false,
    videoThumbs: true,
    hardwareAcceleration: true,
    ffmpegPath: '',
    thumbCacheMb: 300,
    mountsPollMs: 8000,
    doubleClickMs: 300,
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
  let shortcutCaptureId: ShortcutCommandId | null = null
  let shortcutCaptureBusy = false
  let shortcutCaptureError = ''

  $: needle = filter.trim().toLowerCase()

  $: if (open && !seededInitialFilter) {
    filter = initialFilter
    seededInitialFilter = true
  }
  $: if (!open) {
    seededInitialFilter = false
  }

  $: if (settings.defaultView !== defaultViewValue) {
    settings = { ...settings, defaultView: defaultViewValue }
  }
  $: if (settings.showHidden !== showHiddenValue) {
    settings = { ...settings, showHidden: showHiddenValue }
  }
  $: if (settings.hiddenFilesLast !== hiddenFilesLastValue) {
    settings = { ...settings, hiddenFilesLast: hiddenFilesLastValue }
  }
  $: if (settings.foldersFirst !== foldersFirstValue) {
    settings = { ...settings, foldersFirst: foldersFirstValue }
  }
  $: if (settings.startDir !== startDirValue) {
    settings = { ...settings, startDir: startDirValue }
  }
  $: if (settings.confirmDelete !== confirmDeleteValue) {
    settings = { ...settings, confirmDelete: confirmDeleteValue }
  }
  $: if (settings.sortField !== sortFieldValue) {
    settings = { ...settings, sortField: sortFieldValue }
  }
  $: if (settings.sortDirection !== sortDirectionValue) {
    settings = { ...settings, sortDirection: sortDirectionValue }
  }
  $: if (settings.density !== densityValue) {
    settings = { ...settings, density: densityValue }
  }
  $: if (settings.archiveName !== archiveNameValue) {
    settings = { ...settings, archiveName: archiveNameValue }
  }
  $: if (settings.archiveLevel !== archiveLevelValue) {
    settings = { ...settings, archiveLevel: archiveLevelValue }
  }
  $: if (settings.openDestAfterExtract !== openDestAfterExtractValue) {
    settings = { ...settings, openDestAfterExtract: openDestAfterExtractValue }
  }
  $: if (settings.videoThumbs !== videoThumbsValue) {
    settings = { ...settings, videoThumbs: videoThumbsValue }
  }
  $: if (settings.hardwareAcceleration !== hardwareAccelerationValue) {
    settings = { ...settings, hardwareAcceleration: hardwareAccelerationValue }
  }
  $: if (settings.ffmpegPath !== ffmpegPathValue) {
    settings = { ...settings, ffmpegPath: ffmpegPathValue }
  }
  $: if (settings.thumbCacheMb !== thumbCacheMbValue) {
    settings = { ...settings, thumbCacheMb: thumbCacheMbValue }
  }
  $: if (settings.mountsPollMs !== mountsPollMsValue) {
    settings = { ...settings, mountsPollMs: mountsPollMsValue }
  }
  $: if (settings.doubleClickMs !== doubleClickMsValue) {
    settings = { ...settings, doubleClickMs: doubleClickMsValue }
  }

  const rowTexts = (
    ...parts: (string | number | boolean | null | undefined | (string | number | boolean | null | undefined)[])[]
  ) => {
    const out: string[] = []
    const push = (p: any) => {
      if (p === null || p === undefined) return
      if (Array.isArray(p)) {
        p.forEach(push)
      } else {
        out.push(String(p).toLowerCase())
      }
    }
    parts.forEach(push)
    return out
  }

  const rowMatches = (n: string, texts: string[]) => {
    if (!n) return true
    return texts.some((t) => t.includes(n))
  }

  $: defaultViewTexts = rowTexts('default view', 'list', 'grid', settings.defaultView)
  $: foldersFirstTexts = rowTexts('folders first', 'show folders before files')
  $: showHiddenTexts = rowTexts('show hidden', 'show hidden files by default')
  $: hiddenFilesLastTexts = rowTexts('hidden files last', 'place hidden items at the end')
  $: startDirTexts = rowTexts('start directory', settings.startDir, '~ or /path')
  $: confirmDeleteTexts = rowTexts('confirm delete', 'ask before permanent delete')

  $: sortFieldTexts = rowTexts('default sort field', 'sort field', 'name', 'type', 'modified', 'size', settings.sortField)
  $: sortDirectionTexts = rowTexts('sort direction', 'ascending', 'descending', settings.sortDirection)

  $: densityTexts = rowTexts('density', 'cozy', 'compact', settings.density)
  $: iconSizeTexts = rowTexts('icon size', `${settings.iconSize}px`, settings.iconSize)

  $: archiveNameTexts = rowTexts('default archive name', `${settings.archiveName}.zip`)
  $: archiveLevelTexts = rowTexts('zip level', `level ${settings.archiveLevel}`, settings.archiveLevel)
  $: afterExtractTexts = rowTexts('after extract', 'open destination after extract', settings.openDestAfterExtract ? 'enabled' : 'disabled')
  $: rarNoteTexts = rowTexts('note', 'rar compressed entries are currently unsupported (fail fast)', 'rar')

  $: videoThumbsTexts = rowTexts('video thumbs', 'enable video thumbnails', 'requires ffmpeg', settings.videoThumbs ? 'on' : 'off')
  $: ffmpegPathTexts = rowTexts('ffmpeg path', settings.ffmpegPath || 'auto-detect if empty', 'ffmpeg')
  $: thumbCacheTexts = rowTexts('cache size', 'thumbnail cache size', `${settings.thumbCacheMb} mb`)

  $: sortedShortcuts = [...shortcuts].sort((a, b) =>
    a.label.localeCompare(b.label, undefined, { sensitivity: 'base' }),
  )
  $: filteredShortcuts = sortedShortcuts.filter((s) =>
    rowMatches(needle, rowTexts('shortcuts', 'keys', s.label, s.accelerator)),
  )
  $: shortcutMidpoint = Math.ceil(filteredShortcuts.length / 2)
  $: shortcutColumns = [
    filteredShortcuts.slice(0, shortcutMidpoint),
    filteredShortcuts.slice(shortcutMidpoint),
  ]

  $: mountsPollTexts = rowTexts('mounts poll', 'watcher poll', `${settings.mountsPollMs} ms`)
  $: hardwareAccelerationTexts = rowTexts(
    'hardware acceleration',
    'gpu',
    'software rendering',
    settings.hardwareAcceleration ? 'enabled' : 'disabled',
    'restart required'
  )

  $: doubleClickTexts = rowTexts('double-click speed', `${settings.doubleClickMs} ms`, 'double click speed')
  $: clearThumbTexts = rowTexts('clear thumbnail cache', 'clear', 'thumbnail cache')
  $: clearStarsTexts = rowTexts('clear stars', 'clear')
  $: clearBookmarksTexts = rowTexts('clear bookmarks', 'clear')
  $: clearRecentsTexts = rowTexts('clear recents', 'clear')

  $: highContrastTexts = rowTexts('high contrast', 'boost contrast for ui elements')
  $: scrollbarWidthTexts = rowTexts('scrollbar width', `${settings.scrollbarWidth} px`, settings.scrollbarWidth)

  $: externalToolsTexts = rowTexts('external tools', settings.externalTools, 'ffmpeg=/usr/bin/ffmpeg')
  $: logLevelTexts = rowTexts('log level', 'error', 'warn', 'info', 'debug', settings.logLevel)

  $: showGeneral = rowMatches(needle, [
    ...defaultViewTexts,
    ...foldersFirstTexts,
    ...showHiddenTexts,
    ...hiddenFilesLastTexts,
    ...startDirTexts,
    ...confirmDeleteTexts,
  ])

  $: showSorting = rowMatches(needle, [...sortFieldTexts, ...sortDirectionTexts])

  $: showAppearance = rowMatches(needle, [...densityTexts])

  $: showArchives = rowMatches(needle, [
    ...archiveNameTexts,
    ...archiveLevelTexts,
    ...afterExtractTexts,
    ...rarNoteTexts,
  ])

  $: showThumbnails = rowMatches(needle, [
    ...videoThumbsTexts,
    ...ffmpegPathTexts,
    ...thumbCacheTexts,
  ])

  $: showShortcuts = rowMatches(
    needle,
    [
      ...rowTexts('shortcut', 'shortcuts', 'keys', 'keyboard shortcuts'),
      ...filteredShortcuts.flatMap((s) => rowTexts(s.label, s.accelerator)),
    ],
  )

  $: showPerformance = rowMatches(needle, [
    ...hardwareAccelerationTexts,
    ...mountsPollTexts,
  ])

  $: showInteraction = rowMatches(needle, [...doubleClickTexts])

  $: showData = rowMatches(needle, [
    ...clearThumbTexts,
    ...clearStarsTexts,
    ...clearBookmarksTexts,
    ...clearRecentsTexts,
  ])

  $: showAccessibility = rowMatches(needle, [...highContrastTexts, ...scrollbarWidthTexts])

  $: showAdvanced = rowMatches(needle, [...externalToolsTexts, ...logLevelTexts])
  $: hiddenFilesLastDisabled = !settings.showHidden
  $: thumbsDisabled = !settings.videoThumbs

  const clearTargetCopy = {
    'thumb-cache': {
      title: 'Clear thumbnail cache?',
      message: 'This removes all cached thumbnail files on disk and refreshes the UI.',
      confirmLabel: 'Clear cache',
    },
    stars: {
      title: 'Clear all stars?',
      message: 'This removes all starred items.',
      confirmLabel: 'Clear stars',
    },
    bookmarks: {
      title: 'Clear all bookmarks?',
      message: 'This removes all bookmarks.',
      confirmLabel: 'Clear bookmarks',
    },
    recents: {
      title: 'Clear all recents?',
      message: 'This removes all recent items.',
      confirmLabel: 'Clear recents',
    },
  } satisfies Record<DataClearTarget, { title: string; message: string; confirmLabel: string }>

  let clearTarget: DataClearTarget | null = null
  let clearBusy = false
  let clearError = ''

  const requestClear = (target: DataClearTarget) => {
    if (clearBusy) return
    clearTarget = target
    clearError = ''
  }

  const cancelClear = () => {
    if (clearBusy) return
    clearTarget = null
    clearError = ''
  }

  const confirmClear = async () => {
    if (!clearTarget || clearBusy) return
    clearBusy = true
    clearError = ''
    try {
      if (clearTarget === 'thumb-cache') {
        await onClearThumbCache()
      } else if (clearTarget === 'stars') {
        await onClearStars()
      } else if (clearTarget === 'bookmarks') {
        await onClearBookmarks()
      } else {
        await onClearRecents()
      }
      clearTarget = null
    } catch (err) {
      clearError = getErrorMessage(err)
    } finally {
      clearBusy = false
    }
  }

  $: clearDialog = clearTarget ? clearTargetCopy[clearTarget] : null
  $: clearDialogMessage =
    clearDialog && clearError
      ? `${clearDialog.message}\n\nLast error: ${clearError}`
      : clearDialog?.message ?? ''

  const restoreDefaults = () => {
    settings = { ...DEFAULT_SETTINGS }
    filter = ''
  }

  const beginShortcutCapture = (commandId: ShortcutCommandId) => {
    if (shortcutCaptureBusy) return
    shortcutCaptureId = commandId
    shortcutCaptureError = ''
  }

  const cancelShortcutCapture = () => {
    if (shortcutCaptureBusy) return
    shortcutCaptureId = null
    shortcutCaptureError = ''
  }

  const applyShortcutCapture = async (accelerator: string) => {
    if (!shortcutCaptureId) return
    const commandId = shortcutCaptureId
    shortcutCaptureBusy = true
    shortcutCaptureError = ''
    try {
      await onChangeShortcut(commandId, accelerator)
      shortcutCaptureId = null
    } catch (err) {
      shortcutCaptureError = getErrorMessage(err)
    } finally {
      shortcutCaptureBusy = false
    }
  }

  const handleWindowKeydown = (e: KeyboardEvent) => {
    if (!open) return
    if (shortcutCaptureId) {
      if (e.key === 'Escape') {
        e.preventDefault()
        e.stopPropagation()
        cancelShortcutCapture()
        return
      }
      if (shortcutCaptureBusy || e.repeat) {
        e.preventDefault()
        e.stopPropagation()
        return
      }
      const accelerator = keyboardEventToAccelerator(e)
      e.preventDefault()
      e.stopPropagation()
      if (!accelerator) {
        return
      }
      void applyShortcutCapture(accelerator)
      return
    }
    if (e.key === 'Escape') {
      e.preventDefault()
      if (clearTarget) {
        cancelClear()
        return
      }
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
        {#if rowMatches(needle, defaultViewTexts)}
          <div class="form-label">Default view</div>
          <div class="form-control radios">
            <label class="radio">
              <input
                type="radio"
                name="default-view"
                value="list"
                checked={settings.defaultView === 'list'}
                on:change={() => {
                  settings = { ...settings, defaultView: 'list' }
                  onChangeDefaultView('list')
                }}
              />
              <span>List</span>
            </label>
            <label class="radio">
              <input
                type="radio"
                name="default-view"
                value="grid"
                checked={settings.defaultView === 'grid'}
                on:change={() => {
                  settings = { ...settings, defaultView: 'grid' }
                  onChangeDefaultView('grid')
                }}
              />
              <span>Grid</span>
            </label>
          </div>
        {/if}

        {#if rowMatches(needle, foldersFirstTexts)}
          <div class="form-label">Folders first</div>
          <div class="form-control checkbox">
            <input
              type="checkbox"
              checked={settings.foldersFirst}
              on:change={(e) => {
                const next = (e.currentTarget as HTMLInputElement).checked
                settings = { ...settings, foldersFirst: next }
                onToggleFoldersFirst(next)
              }}
            />
            <span>Show folders before files</span>
          </div>
        {/if}

        {#if rowMatches(needle, showHiddenTexts)}
          <div class="form-label">Show hidden</div>
          <div class="form-control checkbox">
            <input
              type="checkbox"
              checked={settings.showHidden}
              on:change={(e) => {
                const next = (e.currentTarget as HTMLInputElement).checked
                settings = { ...settings, showHidden: next }
                onToggleShowHidden(next)
              }}
            />
            <span>Show hidden files by default</span>
          </div>
        {/if}

        {#if rowMatches(needle, hiddenFilesLastTexts)}
          <div class="form-label">Hidden files last</div>
          <div class="form-control checkbox">
            <input
              type="checkbox"
              checked={settings.hiddenFilesLast}
              disabled={hiddenFilesLastDisabled}
              on:change={(e) => {
                const next = (e.currentTarget as HTMLInputElement).checked
                settings = { ...settings, hiddenFilesLast: next }
                onToggleHiddenFilesLast(next)
              }}
            />
            <span>Place hidden items at the end</span>
            {#if hiddenFilesLastDisabled}
              <small>Enable "Show hidden" to change this</small>
            {/if}
          </div>
          {/if}

        {#if rowMatches(needle, startDirTexts)}
          <div class="form-label">Start directory</div>
          <div class="form-control">
            <input
              type="text"
              value={settings.startDir}
              placeholder="~ or /path"
              on:input={(e) => {
                const next = (e.currentTarget as HTMLInputElement).value
                settings = { ...settings, startDir: next }
                onChangeStartDir(next)
              }}
            />
          </div>
        {/if}

        {#if rowMatches(needle, confirmDeleteTexts)}
          <div class="form-label">Confirm delete</div>
          <div class="form-control checkbox">
            <input
              type="checkbox"
              checked={settings.confirmDelete}
              on:change={(e) => {
                const next = (e.currentTarget as HTMLInputElement).checked
                settings = { ...settings, confirmDelete: next }
                onToggleConfirmDelete(next)
              }}
            />
            <span>Ask before permanent delete</span>
          </div>
        {/if}
        {/if}

        {#if showSorting}
          <div class="group-divider" aria-hidden="true"></div>
          <div class="group-heading">Sorting</div><div class="group-spacer"></div>
        {#if rowMatches(needle, sortFieldTexts)}
          <div class="form-label">Default sort field</div>
          <div class="form-control">
            <ComboBox
              value={settings.sortField}
              on:change={(e) => {
                const next = e.detail as typeof settings.sortField
                settings = { ...settings, sortField: next }
                onChangeSortField(next)
              }}
              options={[
                { value: 'name', label: 'Name' },
                { value: 'type', label: 'Type' },
                { value: 'modified', label: 'Date modified' },
                { value: 'size', label: 'Size' },
              ] satisfies ComboOption[]}
            />
          </div>
        {/if}

        {#if rowMatches(needle, sortDirectionTexts)}
          <div class="form-label">Sort direction</div>
          <div class="form-control radios">
            <label class="radio">
              <input
                type="radio"
                name="sort-direction"
                value="asc"
                checked={settings.sortDirection === 'asc'}
                on:change={() => {
                  settings = { ...settings, sortDirection: 'asc' }
                  onChangeSortDirection('asc')
                }}
              />
              <span>Ascending</span>
            </label>
            <label class="radio">
              <input
                type="radio"
                name="sort-direction"
                value="desc"
                checked={settings.sortDirection === 'desc'}
                on:change={() => {
                  settings = { ...settings, sortDirection: 'desc' }
                  onChangeSortDirection('desc')
                }}
              />
              <span>Descending</span>
            </label>
          </div>
        {/if}
        {/if}

        {#if showAppearance}
          <div class="group-divider" aria-hidden="true"></div>
          <div class="group-heading">Appearance</div><div class="group-spacer"></div>
        {#if rowMatches(needle, densityTexts)}
          <div class="form-label">Density</div>
          <div class="form-control">
              <ComboBox
                value={settings.density}
                on:change={(e) => {
                  const next = e.detail as Density
                  settings = { ...settings, density: next }
                  onChangeDensity(next)
                }}
                options={[
                  { value: 'cozy', label: 'Cozy' },
                  { value: 'compact', label: 'Compact' },
                ] satisfies ComboOption[]}
              />
            </div>
          {/if}
        {/if}

        {#if showArchives}
          <div class="group-divider" aria-hidden="true"></div>
          <div class="group-heading">Archives</div><div class="group-spacer"></div>
        {#if rowMatches(needle, archiveNameTexts)}
          <div class="form-label">Default archive name</div>
          <div class="form-control archive-name">
            <input
              type="text"
              value={settings.archiveName}
              on:input={(e) => {
                const val = (e.currentTarget as HTMLInputElement).value
                settings = { ...settings, archiveName: val }
                onChangeArchiveName(val)
              }}
            />
            <span class="suffix">.zip</span>
          </div>
        {/if}

        {#if rowMatches(needle, archiveLevelTexts)}
          <div class="form-label">ZIP level</div>
          <div class="form-control">
            <input
              type="range"
              min="0"
              max="9"
              step="1"
              value={settings.archiveLevel}
              on:input={(e) => {
                const next = Number((e.currentTarget as HTMLInputElement).value)
                settings = { ...settings, archiveLevel: next }
                onChangeArchiveLevel(next)
              }}
            />
            <small>Level {settings.archiveLevel}</small>
          </div>
        {/if}

        {#if rowMatches(needle, afterExtractTexts)}
          <div class="form-label">After extract</div>
          <div class="form-control checkbox">
            <input
              type="checkbox"
              checked={settings.openDestAfterExtract}
              on:change={(e) => {
                const next = (e.currentTarget as HTMLInputElement).checked
                settings = { ...settings, openDestAfterExtract: next }
                onToggleOpenDestAfterExtract(next)
              }}
            />
            <span>Open destination after extract</span>
          </div>
        {/if}

        {#if rowMatches(needle, rarNoteTexts)}
          <div class="form-label">Note</div>
          <div class="form-control">
            <p class="note">RAR compressed entries are currently unsupported (fail fast).</p>
            </div>
          {/if}
        {/if}

        {#if showThumbnails}
          <div class="group-divider" aria-hidden="true"></div>
          <div class="group-heading">Thumbnails</div><div class="group-spacer"></div>
        {#if rowMatches(needle, videoThumbsTexts)}
          <div class="form-label">Video thumbs</div>
          <div class="form-control checkbox">
            <input
              type="checkbox"
              checked={settings.videoThumbs}
              on:change={(e) => {
                const next = (e.currentTarget as HTMLInputElement).checked
                settings = { ...settings, videoThumbs: next }
                onToggleVideoThumbs(next)
              }}
            />
            <span>Enable video thumbnails (requires ffmpeg)</span>
          </div>
        {/if}

        {#if rowMatches(needle, ffmpegPathTexts)}
          <div class="form-label">FFmpeg path</div>
          <div class="form-control">
            <input
              type="text"
              value={settings.ffmpegPath}
              placeholder="auto-detect if empty"
              disabled={thumbsDisabled}
              on:input={(e) => {
                const next = (e.currentTarget as HTMLInputElement).value
                settings = { ...settings, ffmpegPath: next }
                onChangeFfmpegPath(next)
              }}
            />
            </div>
          {/if}

        {#if rowMatches(needle, thumbCacheTexts)}
          <div class="form-label">Cache size</div>
          <div class="form-control">
            <input
                type="range"
                min="50"
                max="1000"
                step="50"
                value={settings.thumbCacheMb}
                on:input={(e) => {
                  const next = Number((e.currentTarget as HTMLInputElement).value)
                  settings = { ...settings, thumbCacheMb: next }
                  onChangeThumbCacheMb(next)
                }}
                disabled={thumbsDisabled}
              />
              <small>{settings.thumbCacheMb} MB</small>
            </div>
          {/if}

        {/if}

        {#if showShortcuts}
          <div class="group-divider" aria-hidden="true"></div>
          <div class="group-heading">Shortcuts</div><div class="group-spacer"></div>
          <div class="form-control shortcuts-control shortcuts-row">
            <div class="shortcuts-columns">
              {#each shortcutColumns as column, columnIndex (columnIndex)}
                <div class="shortcuts-column">
                  {#each column as shortcut (shortcut.commandId)}
                    <div class="shortcut-item">
                      <span class="shortcut-action">{shortcut.label}</span>
                      <button
                        type="button"
                        class="key shortcut-key"
                        class:capturing={shortcutCaptureId === shortcut.commandId}
                        disabled={shortcutCaptureBusy && shortcutCaptureId !== shortcut.commandId}
                        on:click={() => beginShortcutCapture(shortcut.commandId)}
                      >
                        {#if shortcutCaptureId === shortcut.commandId}
                          {#if shortcutCaptureBusy}
                            Saving...
                          {:else}
                            Press keys
                          {/if}
                        {:else}
                          {shortcut.accelerator}
                        {/if}
                      </button>
                    </div>
                  {/each}
                </div>
              {/each}
            </div>
            {#if shortcutCaptureError}
              <div class="shortcuts-error">{shortcutCaptureError}</div>
            {/if}
          </div>
        {/if}

        {#if showPerformance}
          <div class="group-divider" aria-hidden="true"></div>
          <div class="group-heading">Performance</div><div class="group-spacer"></div>
        {#if rowMatches(needle, hardwareAccelerationTexts)}
          <div class="form-label">Hardware acceleration</div>
          <div class="form-control checkbox">
            <input
              type="checkbox"
              checked={settings.hardwareAcceleration}
              on:change={(e) => {
                const next = (e.currentTarget as HTMLInputElement).checked
                settings = { ...settings, hardwareAcceleration: next }
                onToggleHardwareAcceleration(next)
              }}
            />
            <span>Use GPU acceleration for rendering</span>
            <small>Requires restart to take effect</small>
          </div>
        {/if}
        {#if rowMatches(needle, mountsPollTexts)}
          <div class="form-label">Mounts poll (ms)</div>
          <div class="form-control">
            <input
                type="range"
                min="500"
                max="10000"
                step="100"
                value={settings.mountsPollMs}
                on:input={(e) => {
                  const next = Number((e.currentTarget as HTMLInputElement).value)
                  settings = { ...settings, mountsPollMs: next }
                  onChangeMountsPollMs(next)
                }}
              />
              <small>{settings.mountsPollMs} ms</small>
            </div>
          {/if}

        {/if}

        {#if showInteraction}
          <div class="group-divider" aria-hidden="true"></div>
          <div class="group-heading">Interaction</div><div class="group-spacer"></div>
        {#if rowMatches(needle, doubleClickTexts)}
          <div class="form-label">Double-click speed</div>
          <div class="form-control">
              <input
                type="range"
                min="150"
                max="600"
                step="10"
                value={settings.doubleClickMs}
                on:input={(e) => {
                  const next = Number((e.currentTarget as HTMLInputElement).value)
                  settings = { ...settings, doubleClickMs: next }
                  onChangeDoubleClickMs(next)
                }}
              />
              <small>{settings.doubleClickMs} ms</small>
            </div>
          {/if}

        {/if}

        {#if showData}
          <div class="group-divider" aria-hidden="true"></div>
          <div class="group-heading">Data</div><div class="group-spacer"></div>
          <div class="form-label">Clear thumbnail cache</div>
          <div class="form-control">
            <button
              type="button"
              class="secondary"
              disabled={clearBusy}
              on:click={() => requestClear('thumb-cache')}
            >
              {clearBusy && clearTarget === 'thumb-cache' ? 'Clearing...' : 'Clear'}
            </button>
          </div>
          <div class="form-label">Clear stars</div>
          <div class="form-control">
            <button
              type="button"
              class="secondary"
              disabled={clearBusy}
              on:click={() => requestClear('stars')}
            >
              {clearBusy && clearTarget === 'stars' ? 'Clearing...' : 'Clear'}
            </button>
          </div>
          <div class="form-label">Clear bookmarks</div>
          <div class="form-control">
            <button
              type="button"
              class="secondary"
              disabled={clearBusy}
              on:click={() => requestClear('bookmarks')}
            >
              {clearBusy && clearTarget === 'bookmarks' ? 'Clearing...' : 'Clear'}
            </button>
          </div>
          <div class="form-label">Clear recents</div>
          <div class="form-control">
            <button
              type="button"
              class="secondary"
              disabled={clearBusy}
              on:click={() => requestClear('recents')}
            >
              {clearBusy && clearTarget === 'recents' ? 'Clearing...' : 'Clear'}
            </button>
          </div>
        {/if}

        {#if showAccessibility}
          <div class="group-divider" aria-hidden="true"></div>
          <div class="group-heading">Accessibility</div><div class="group-spacer"></div>
        {#if rowMatches(needle, highContrastTexts)}
          <div class="form-label">High contrast</div>
          <div class="form-control checkbox">
            <input type="checkbox" bind:checked={settings.highContrast} />
            <span>Boost contrast for UI elements</span>
          </div>
        {/if}

        {#if rowMatches(needle, scrollbarWidthTexts)}
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
          <div class="group-divider" aria-hidden="true"></div>
          <div class="group-heading">Advanced</div><div class="group-spacer"></div>
        {#if rowMatches(needle, externalToolsTexts)}
          <div class="form-label">External tools</div>
          <div class="form-control column">
            <textarea rows="2" bind:value={settings.externalTools} placeholder="ffmpeg=/usr/bin/ffmpeg"></textarea>
          </div>
        {/if}

        {#if rowMatches(needle, logLevelTexts)}
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

  <ConfirmActionModal
    open={clearTarget !== null}
    title={clearDialog?.title ?? 'Confirm action'}
    message={clearDialogMessage}
    confirmLabel={clearDialog?.confirmLabel ?? 'Confirm'}
    cancelLabel="Cancel"
    danger={true}
    busy={clearBusy}
    onConfirm={() => void confirmClear()}
    onCancel={cancelClear}
  />
{/if}

<style>
  /* Inherits global modal styles; light tweaks for tabs and spacing */
  :global(.settings-modal) {
    max-height: 80vh;
    min-height: var(--settings-modal-min-height);
    display: flex;
    flex-direction: column;
  }

  .settings-header {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    gap: var(--settings-form-column-gap);
    flex-wrap: wrap;
    margin-bottom: var(--settings-header-margin-bottom);
  }

  .settings-header h2 {
    margin: 0;
    font-size: var(--modal-header-size);
    line-height: var(--modal-header-line);
    flex: 0 0 var(--settings-form-label-width);
    text-align: right;
  }

  .settings-filter {
    min-width: var(--settings-filter-min-width);
    flex: 1 1 var(--settings-filter-width);
  }

  .restore-btn {
    padding: var(--settings-restore-padding-y) var(--settings-restore-padding-x);
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    cursor: pointer;
    font-size: var(--settings-restore-font-size);
    margin-left: auto;
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
    gap: var(--settings-panel-gap);
    padding-right: var(--settings-panel-padding-right);
  }

  .settings-panel.single {
    padding-right: var(--settings-panel-padding-right);
  }

  .group-heading {
    grid-column: 1;
    font-weight: 700;
    color: var(--fg-strong);
    padding-top: 0;
    text-align: right;
    justify-self: end;
  }

  .group-spacer {
    grid-column: 2;
    min-height: 1px;
  }

  .group-divider {
    grid-column: 1 / -1;
    height: 0;
    border-top: 1px solid var(--border);
    margin-top: var(--settings-group-divider-margin-top);
    margin-bottom: var(--settings-group-divider-margin-bottom);
  }

  .settings-table > .group-divider:first-child {
    display: none;
  }

  .form-rows {
    display: grid;
    grid-template-columns: var(--settings-form-label-width) 1fr;
    column-gap: var(--settings-form-column-gap);
    row-gap: var(--settings-form-row-gap);
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
    gap: var(--settings-control-gap);
    width: 100%;
  }

  .form-control.column {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--settings-control-gap-tight);
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
    gap: var(--settings-radios-gap);
  }

  .form-control.radios .radio {
    display: inline-flex;
    align-items: center;
    gap: var(--settings-control-gap-tight);
    cursor: pointer;
    color: var(--fg);
  }

  .archive-name {
    display: flex;
    align-items: center;
    gap: var(--settings-archive-gap);
  }

  .archive-name input {
    flex: 1;
  }

  .archive-name .suffix {
    color: var(--fg-muted);
    font-size: var(--settings-archive-suffix-size);
    min-width: 32px;
    text-align: left;
  }

  .key {
    display: inline-flex;
    min-width: 64px;
    justify-content: center;
    padding: var(--settings-key-padding-y) var(--settings-key-padding-x);
    border: 1px solid var(--border);
    background: var(--bg);
    border-radius: 0;
    font-size: var(--settings-key-font-size);
  }

  .shortcuts-control {
    align-items: stretch;
  }

  .shortcuts-row {
    grid-column: 2;
  }

  .shortcuts-columns {
    display: flex;
    gap: var(--settings-shortcuts-columns-gap);
    width: 100%;
  }

  .shortcuts-column {
    flex: 1 1 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--settings-shortcuts-column-gap);
  }

  .shortcut-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--settings-shortcut-item-gap);
    min-height: var(--settings-shortcut-item-min-height);
  }

  .shortcut-action {
    color: var(--fg);
  }

  .shortcut-key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: var(--settings-shortcut-key-height);
    line-height: 1;
    cursor: pointer;
  }

  .shortcut-key.capturing {
    border-color: var(--border-accent);
    background: var(--bg-hover);
  }

  .shortcuts-error {
    margin-top: 8px;
    color: var(--danger);
    font-size: var(--settings-shortcut-error-size);
  }

  @media (max-width: 760px) {
    .shortcuts-columns {
      flex-direction: column;
      gap: var(--settings-shortcuts-column-gap);
    }
  }
</style>
