import { getErrorMessage } from '@/shared/lib/error'
import { derived, get, writable } from 'svelte/store'
import { keyboardEventToAccelerator, type ShortcutBinding, type ShortcutCommandId } from '@/features/shortcuts'
import {
  clearTargetCopy,
  type DataClearTarget,
  DEFAULT_SETTINGS,
  type Settings,
} from '../settingsTypes'

type ViewModelDeps = {
  onClose: () => void
  onChangeShortcut: (commandId: ShortcutCommandId, accelerator: string) => Promise<void> | void
  onClearThumbCache: () => Promise<void> | void
  onClearStars: () => Promise<void> | void
  onClearBookmarks: () => Promise<void> | void
  onClearRecents: () => Promise<void> | void
}

type FilterModel = {
  showGeneral: boolean
  showSorting: boolean
  showAppearance: boolean
  showArchives: boolean
  showThumbnails: boolean
  showShortcuts: boolean
  showPerformance: boolean
  showInteraction: boolean
  showData: boolean
  showAccessibility: boolean
  showAdvanced: boolean
  showDefaultViewRow: boolean
  showFoldersFirstRow: boolean
  showShowHiddenRow: boolean
  showHiddenFilesLastRow: boolean
  showStartDirRow: boolean
  showConfirmDeleteRow: boolean
  showSortFieldRow: boolean
  showSortDirectionRow: boolean
  showDensityRow: boolean
  showArchiveNameRow: boolean
  showArchiveLevelRow: boolean
  showAfterExtractRow: boolean
  showRarNoteRow: boolean
  showVideoThumbsRow: boolean
  showFfmpegPathRow: boolean
  showThumbCacheRow: boolean
  showHardwareAccelerationRow: boolean
  showMountsPollRow: boolean
  showDoubleClickRow: boolean
  showHighContrastRow: boolean
  showScrollbarWidthRow: boolean
  showExternalToolsRow: boolean
  showLogLevelRow: boolean
  hiddenFilesLastDisabled: boolean
  thumbsDisabled: boolean
  filteredShortcuts: ShortcutBinding[]
  shortcutColumns: ShortcutBinding[][]
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

const rowMatches = (needle: string, texts: string[]) => {
  if (!needle) return true
  return texts.some((t) => t.includes(needle))
}

export const createSettingsModalViewModel = (deps: ViewModelDeps) => {
  const filter = writable('')
  const needle = derived(filter, ($filter) => $filter.trim().toLowerCase())
  const shortcuts = writable<ShortcutBinding[]>([])
  let seededInitialFilter = false

  const shortcutCaptureId = writable<ShortcutCommandId | null>(null)
  const shortcutCaptureBusy = writable(false)
  const shortcutCaptureError = writable('')

  const clearTarget = writable<DataClearTarget | null>(null)
  const clearBusy = writable(false)
  const clearError = writable('')
  const clearDialog = derived(clearTarget, ($clearTarget) =>
    $clearTarget ? clearTargetCopy[$clearTarget] : null,
  )
  const clearDialogMessage = derived([clearDialog, clearError], ([$clearDialog, $clearError]) =>
    $clearDialog && $clearError
      ? `${$clearDialog.message}\n\nLast error: ${$clearError}`
      : $clearDialog?.message ?? '',
  )

  const syncModalState = (open: boolean, initialFilter: string) => {
    if (open && !seededInitialFilter) {
      filter.set(initialFilter)
      seededInitialFilter = true
    } else if (!open) {
      seededInitialFilter = false
    }
  }

  const setShortcuts = (value: ShortcutBinding[]) => {
    shortcuts.set(value)
  }

  const requestClear = (target: DataClearTarget) => {
    if (get(clearBusy)) return
    clearTarget.set(target)
    clearError.set('')
  }

  const cancelClear = () => {
    if (get(clearBusy)) return
    clearTarget.set(null)
    clearError.set('')
  }

  const confirmClear = async () => {
    const target = get(clearTarget)
    if (!target || get(clearBusy)) return
    clearBusy.set(true)
    clearError.set('')
    try {
      if (target === 'thumb-cache') {
        await deps.onClearThumbCache()
      } else if (target === 'stars') {
        await deps.onClearStars()
      } else if (target === 'bookmarks') {
        await deps.onClearBookmarks()
      } else {
        await deps.onClearRecents()
      }
      clearTarget.set(null)
    } catch (err) {
      clearError.set(getErrorMessage(err))
    } finally {
      clearBusy.set(false)
    }
  }

  const beginShortcutCapture = (commandId: ShortcutCommandId) => {
    if (get(shortcutCaptureBusy)) return
    shortcutCaptureId.set(commandId)
    shortcutCaptureError.set('')
  }

  const cancelShortcutCapture = () => {
    if (get(shortcutCaptureBusy)) return
    shortcutCaptureId.set(null)
    shortcutCaptureError.set('')
  }

  const applyShortcutCapture = async (accelerator: string) => {
    const commandId = get(shortcutCaptureId)
    if (!commandId) return
    shortcutCaptureBusy.set(true)
    shortcutCaptureError.set('')
    try {
      await deps.onChangeShortcut(commandId, accelerator)
      shortcutCaptureId.set(null)
    } catch (err) {
      shortcutCaptureError.set(getErrorMessage(err))
    } finally {
      shortcutCaptureBusy.set(false)
    }
  }

  const handleWindowKeydown = (e: KeyboardEvent, open: boolean) => {
    if (!open) return
    const captureId = get(shortcutCaptureId)
    if (captureId) {
      if (e.key === 'Escape') {
        e.preventDefault()
        e.stopPropagation()
        cancelShortcutCapture()
        return
      }
      if (get(shortcutCaptureBusy) || e.repeat) {
        e.preventDefault()
        e.stopPropagation()
        return
      }
      const accelerator = keyboardEventToAccelerator(e)
      e.preventDefault()
      e.stopPropagation()
      if (!accelerator) return
      void applyShortcutCapture(accelerator)
      return
    }
    if (e.key === 'Escape') {
      e.preventDefault()
      if (get(clearTarget)) {
        cancelClear()
        return
      }
      deps.onClose()
    }
  }

  const restoreDefaults = (
    setSettings: (next: Settings) => void,
    options: { clearFilter?: boolean } = {},
  ) => {
    setSettings({ ...DEFAULT_SETTINGS })
    if (options.clearFilter ?? true) {
      filter.set('')
    }
  }

  const buildFilterModel = (settings: Settings): FilterModel => {
    const n = get(needle)
    const shortcutRows = get(shortcuts)
    const sortedShortcuts = [...shortcutRows].sort((a, b) =>
      a.label.localeCompare(b.label, undefined, { sensitivity: 'base' }),
    )
    const filteredShortcuts = sortedShortcuts.filter((s) =>
      rowMatches(n, rowTexts('shortcuts', 'keys', s.label, s.accelerator)),
    )
    const shortcutMidpoint = Math.ceil(filteredShortcuts.length / 2)
    const shortcutColumns = [
      filteredShortcuts.slice(0, shortcutMidpoint),
      filteredShortcuts.slice(shortcutMidpoint),
    ]

    const defaultViewTexts = rowTexts('default view', 'list', 'grid', settings.defaultView)
    const foldersFirstTexts = rowTexts('folders first', 'show folders before files')
    const showHiddenTexts = rowTexts('show hidden', 'show hidden files by default')
    const hiddenFilesLastTexts = rowTexts('hidden files last', 'place hidden items at the end')
    const startDirTexts = rowTexts('start directory', settings.startDir, '~ or /path')
    const confirmDeleteTexts = rowTexts('confirm delete', 'ask before permanent delete')
    const sortFieldTexts = rowTexts(
      'default sort field',
      'sort field',
      'name',
      'type',
      'modified',
      'size',
      settings.sortField,
    )
    const sortDirectionTexts = rowTexts('sort direction', 'ascending', 'descending', settings.sortDirection)
    const densityTexts = rowTexts('density', 'cozy', 'compact', settings.density)
    const archiveNameTexts = rowTexts('default archive name', `${settings.archiveName}.zip`)
    const archiveLevelTexts = rowTexts('zip level', `level ${settings.archiveLevel}`, settings.archiveLevel)
    const afterExtractTexts = rowTexts(
      'after extract',
      'open destination after extract',
      settings.openDestAfterExtract ? 'enabled' : 'disabled',
    )
    const rarNoteTexts = rowTexts(
      'note',
      'rar compressed entries are currently unsupported (fail fast)',
      'rar',
    )
    const videoThumbsTexts = rowTexts(
      'video thumbs',
      'enable video thumbnails',
      'requires ffmpeg',
      settings.videoThumbs ? 'on' : 'off',
    )
    const ffmpegPathTexts = rowTexts('ffmpeg path', settings.ffmpegPath || 'auto-detect if empty', 'ffmpeg')
    const thumbCacheTexts = rowTexts('cache size', 'thumbnail cache size', `${settings.thumbCacheMb} mb`)
    const mountsPollTexts = rowTexts('mounts poll', 'watcher poll', `${settings.mountsPollMs} ms`)
    const hardwareAccelerationTexts = rowTexts(
      'hardware acceleration',
      'gpu',
      'software rendering',
      settings.hardwareAcceleration ? 'enabled' : 'disabled',
      'restart required',
    )
    const doubleClickTexts = rowTexts('double-click speed', `${settings.doubleClickMs} ms`, 'double click speed')
    const clearThumbTexts = rowTexts('clear thumbnail cache', 'clear', 'thumbnail cache')
    const clearStarsTexts = rowTexts('clear stars', 'clear')
    const clearBookmarksTexts = rowTexts('clear bookmarks', 'clear')
    const clearRecentsTexts = rowTexts('clear recents', 'clear')
    const highContrastTexts = rowTexts('high contrast', 'boost contrast for ui elements')
    const scrollbarWidthTexts = rowTexts('scrollbar width', `${settings.scrollbarWidth} px`, settings.scrollbarWidth)
    const externalToolsTexts = rowTexts('external tools', settings.externalTools, 'ffmpeg=/usr/bin/ffmpeg')
    const logLevelTexts = rowTexts('log level', 'error', 'warn', 'info', 'debug', settings.logLevel)

    const showDefaultViewRow = rowMatches(n, defaultViewTexts)
    const showFoldersFirstRow = rowMatches(n, foldersFirstTexts)
    const showShowHiddenRow = rowMatches(n, showHiddenTexts)
    const showHiddenFilesLastRow = rowMatches(n, hiddenFilesLastTexts)
    const showStartDirRow = rowMatches(n, startDirTexts)
    const showConfirmDeleteRow = rowMatches(n, confirmDeleteTexts)
    const showSortFieldRow = rowMatches(n, sortFieldTexts)
    const showSortDirectionRow = rowMatches(n, sortDirectionTexts)
    const showDensityRow = rowMatches(n, densityTexts)
    const showArchiveNameRow = rowMatches(n, archiveNameTexts)
    const showArchiveLevelRow = rowMatches(n, archiveLevelTexts)
    const showAfterExtractRow = rowMatches(n, afterExtractTexts)
    const showRarNoteRow = rowMatches(n, rarNoteTexts)
    const showVideoThumbsRow = rowMatches(n, videoThumbsTexts)
    const showFfmpegPathRow = rowMatches(n, ffmpegPathTexts)
    const showThumbCacheRow = rowMatches(n, thumbCacheTexts)
    const showHardwareAccelerationRow = rowMatches(n, hardwareAccelerationTexts)
    const showMountsPollRow = rowMatches(n, mountsPollTexts)
    const showDoubleClickRow = rowMatches(n, doubleClickTexts)
    const showHighContrastRow = rowMatches(n, highContrastTexts)
    const showScrollbarWidthRow = rowMatches(n, scrollbarWidthTexts)
    const showExternalToolsRow = rowMatches(n, externalToolsTexts)
    const showLogLevelRow = rowMatches(n, logLevelTexts)

    return {
      showGeneral: rowMatches(n, [
        ...defaultViewTexts,
        ...foldersFirstTexts,
        ...showHiddenTexts,
        ...hiddenFilesLastTexts,
        ...startDirTexts,
        ...confirmDeleteTexts,
      ]),
      showSorting: rowMatches(n, [...sortFieldTexts, ...sortDirectionTexts]),
      showAppearance: rowMatches(n, [...densityTexts]),
      showArchives: rowMatches(n, [...archiveNameTexts, ...archiveLevelTexts, ...afterExtractTexts, ...rarNoteTexts]),
      showThumbnails: rowMatches(n, [...videoThumbsTexts, ...ffmpegPathTexts, ...thumbCacheTexts]),
      showShortcuts: rowMatches(
        n,
        [
          ...rowTexts('shortcut', 'shortcuts', 'keys', 'keyboard shortcuts'),
          ...filteredShortcuts.flatMap((s) => rowTexts(s.label, s.accelerator)),
        ],
      ),
      showPerformance: rowMatches(n, [...hardwareAccelerationTexts, ...mountsPollTexts]),
      showInteraction: rowMatches(n, [...doubleClickTexts]),
      showData: rowMatches(n, [...clearThumbTexts, ...clearStarsTexts, ...clearBookmarksTexts, ...clearRecentsTexts]),
      showAccessibility: rowMatches(n, [...highContrastTexts, ...scrollbarWidthTexts]),
      showAdvanced: rowMatches(n, [...externalToolsTexts, ...logLevelTexts]),
      showDefaultViewRow,
      showFoldersFirstRow,
      showShowHiddenRow,
      showHiddenFilesLastRow,
      showStartDirRow,
      showConfirmDeleteRow,
      showSortFieldRow,
      showSortDirectionRow,
      showDensityRow,
      showArchiveNameRow,
      showArchiveLevelRow,
      showAfterExtractRow,
      showRarNoteRow,
      showVideoThumbsRow,
      showFfmpegPathRow,
      showThumbCacheRow,
      showHardwareAccelerationRow,
      showMountsPollRow,
      showDoubleClickRow,
      showHighContrastRow,
      showScrollbarWidthRow,
      showExternalToolsRow,
      showLogLevelRow,
      hiddenFilesLastDisabled: !settings.showHidden,
      thumbsDisabled: !settings.videoThumbs,
      filteredShortcuts,
      shortcutColumns,
    }
  }

  return {
    filter,
    needle,
    setShortcuts,
    syncModalState,
    clearTarget,
    clearBusy,
    clearError,
    clearDialog,
    clearDialogMessage,
    requestClear,
    cancelClear,
    confirmClear,
    shortcutCaptureId,
    shortcutCaptureBusy,
    shortcutCaptureError,
    beginShortcutCapture,
    cancelShortcutCapture,
    handleWindowKeydown,
    restoreDefaults,
    buildFilterModel,
  }
}
