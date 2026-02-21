import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { writable, get } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import {
  setClipboardCmd,
  clearSystemClipboard,
  pasteClipboardCmd,
  pasteClipboardPreview,
  getSystemClipboardPaths,
} from '../services/clipboard.service'
import {
  entryKind,
  dirSizes,
  canExtractPaths as canExtractPathsCmd,
  extractArchive,
  extractArchives,
} from '../services/files.service'
import { checkDuplicatesStream, type DuplicateScanProgress } from '../services/duplicates.service'
import { cancelTask } from '../services/activity.service'
import { setClipboardState, clearClipboardState } from './clipboard.store'
import { normalizePath, parentPath } from '../utils'
import type { Entry } from '../model/types'
import type { CurrentView } from '../context/useContextActions'

type ConflictItem = {
  src: string
  target: string
  is_dir: boolean
}

type ActivityApi = {
  start: (label: string, eventName: string, onCancel?: () => void) => Promise<void>
  requestCancel: (eventName: string) => Promise<void> | void
  hideSoon: () => void
  clearNow: () => void
  cleanup: (preserveTimer?: boolean) => Promise<void>
}

type Deps = {
  currentView: () => CurrentView
  getCurrentPath: () => string
  clipboardMode: () => 'copy' | 'cut'
  setClipboardPaths: (paths: Set<string>) => void
  shouldOpenDestAfterExtract: () => boolean
  loadPath: (path: string, opts?: { recordHistory?: boolean; silent?: boolean }) => Promise<void>
  reloadCurrent: () => Promise<void>
  getDuplicateScanInput: () => { target: Entry | null; searchRoot: string; scanning: boolean }
  duplicateModalStart: () => void
  duplicateModalSetProgress: (progressPercent: number, progressLabel: string) => void
  duplicateModalFinish: (paths: string[]) => void
  duplicateModalFail: (error: string) => void
  duplicateModalStop: () => void
  duplicateModalClose: () => void
  showToast: (msg: string, durationMs?: number) => void
  activityApi: ActivityApi
}

export const useExplorerFileOps = (deps: Deps) => {
  let conflictDest: string | null = null
  let extracting = false
  let duplicateScanToken = 0
  let activeDuplicateProgressEvent: string | null = null
  let unlistenDuplicateProgress: UnlistenFn | null = null
  let dirSizeAbort = 0
  let activeDirSizeProgressEvent: string | null = null
  const conflictModalOpen = writable(false)
  const conflictList = writable<ConflictItem[]>([])

  const clearConflictState = () => {
    conflictModalOpen.set(false)
    conflictList.set([])
    conflictDest = null
  }

  const runPaste = async (target: string, policy: 'rename' | 'overwrite' = 'rename') => {
    const progressEvent = `copy-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      await deps.activityApi.start('Copying…', progressEvent, () => deps.activityApi.requestCancel(progressEvent))
      await pasteClipboardCmd(target, policy, progressEvent)
      await deps.reloadCurrent()
      deps.activityApi.hideSoon()
      return true
    } catch (err) {
      deps.activityApi.clearNow()
      await deps.activityApi.cleanup()
      deps.showToast(`Paste failed: ${getErrorMessage(err)}`)
      return false
    }
  }

  const handlePasteOrMove = async (dest: string) => {
    try {
      const conflicts = await pasteClipboardPreview(dest)
      if (conflicts && conflicts.length > 0) {
        const destNorm = normalizePath(dest)
        const selfPaste = conflicts.every((c) => normalizePath(parentPath(c.src)) === destNorm)
        if (selfPaste) {
          return await runPaste(dest, 'rename')
        }
        conflictList.set(conflicts)
        conflictDest = dest
        conflictModalOpen.set(true)
        return false
      }
      return await runPaste(dest, 'rename')
    } catch (err) {
      deps.showToast(`Paste failed: ${getErrorMessage(err)}`)
      return false
    }
  }

  const pasteIntoCurrent = async () => {
    if (deps.currentView() !== 'dir') {
      deps.showToast('Cannot paste here')
      return false
    }

    // Always attempt to sync from system clipboard first, then paste.
    try {
      const sys = await getSystemClipboardPaths()
      if (sys.paths.length > 0) {
        await setClipboardCmd(sys.paths, sys.mode)
        const stubEntries = sys.paths.map((path) => ({
          path,
          name: path.split('/').pop() ?? path,
          kind: 'file',
          iconId: 12,
        }))
        setClipboardState(sys.mode, stubEntries as unknown as Entry[])
      }
    } catch (err) {
      const msg = getErrorMessage(err)
      if (!msg.toLowerCase().includes('no file paths found')) {
        deps.showToast(`System clipboard unavailable: ${msg}`, 2000)
      }
    }

    const ok = await handlePasteOrMove(deps.getCurrentPath())
    if (ok && deps.clipboardMode() === 'cut') {
      // Clear internal and system clipboard after a successful move.
      clearClipboardState()
      try {
        await setClipboardCmd([], 'copy')
        await clearSystemClipboard()
      } catch {
        // Ignore; move already succeeded.
      }
      deps.setClipboardPaths(new Set())
    }
    return ok
  }

  const resolveConflicts = async (policy: 'rename' | 'overwrite') => {
    if (!conflictDest) return
    conflictModalOpen.set(false)
    try {
      await runPaste(conflictDest, policy)
    } finally {
      conflictDest = null
      conflictList.set([])
    }
  }

  const canExtractPaths = async (paths: string[]): Promise<boolean> => {
    if (paths.length === 0) return false
    try {
      return await canExtractPathsCmd(paths)
    } catch {
      return false
    }
  }

  const extractEntries = async (entriesToExtract: Entry[]) => {
    if (extracting) return
    if (entriesToExtract.length === 0) return
    const allArchives = await canExtractPaths(entriesToExtract.map((entry) => entry.path))
    if (!allArchives) {
      deps.showToast('Extraction available for archive files only')
      return
    }

    extracting = true
    const progressEvent = `extract-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    await deps.activityApi.start(
      `Extracting${entriesToExtract.length > 1 ? ` ${entriesToExtract.length} items…` : '…'}`,
      progressEvent,
      () => deps.activityApi.requestCancel(progressEvent),
    )

    const summarize = (skippedSymlinks: number, skippedOther: number) => {
      const skipParts = []
      if (skippedSymlinks > 0) skipParts.push(`${skippedSymlinks} symlink${skippedSymlinks === 1 ? '' : 's'}`)
      if (skippedOther > 0) skipParts.push(`${skippedOther} entr${skippedOther === 1 ? 'y' : 'ies'}`)
      return skipParts.length > 0 ? ` (skipped ${skipParts.join(', ')})` : ''
    }

    try {
      if (entriesToExtract.length === 1) {
        const entry = entriesToExtract[0]
        const result = await extractArchive(entry.path, progressEvent)
        if (deps.shouldOpenDestAfterExtract() && result?.destination) {
          try {
            const kind = await entryKind(result.destination)
            const target = kind === 'dir' ? result.destination : parentPath(result.destination)
            await deps.loadPath(target, { recordHistory: true })
          } catch {
            await deps.reloadCurrent()
          }
        } else {
          await deps.reloadCurrent()
        }
        const suffix = summarize(result?.skipped_symlinks ?? 0, result?.skipped_entries ?? 0)
        deps.showToast(`Extracted to ${result.destination}${suffix}`)
      } else {
        const result = await extractArchives(entriesToExtract.map((entry) => entry.path), progressEvent)
        const successes = result.filter((item) => item.ok && item.result)
        const failures = result.filter((item) => !item.ok)
        // In batch extraction, keep current location stable even if opening destination is enabled.
        await deps.reloadCurrent()
        const totalSkippedSymlinks = successes.reduce(
          (count, item) => count + (item.result?.skipped_symlinks ?? 0),
          0,
        )
        const totalSkippedOther = successes.reduce(
          (count, item) => count + (item.result?.skipped_entries ?? 0),
          0,
        )
        const suffix = summarize(totalSkippedSymlinks, totalSkippedOther)
        if (failures.length === 0) {
          deps.showToast(`Extracted ${successes.length} archives${suffix}`)
        } else if (successes.length === 0) {
          deps.showToast(`Extraction failed for ${failures.length} archives`)
        } else {
          deps.showToast(`Extracted ${successes.length} archives, ${failures.length} failed${suffix}`)
        }
      }
    } catch (err) {
      const msg = getErrorMessage(err)
      if (msg.toLowerCase().includes('cancelled')) {
        deps.showToast('Extraction cancelled')
      } else {
        deps.showToast(`Failed to extract: ${msg}`)
      }
    } finally {
      extracting = false
      deps.activityApi.clearNow()
      await deps.activityApi.cleanup()
    }
  }

  const duplicateProgressLabel = (payload: DuplicateScanProgress) => {
    if (payload.phase === 'collecting') {
      return `Scanning files: ${payload.scannedFiles} checked, ${payload.candidateFiles} candidate${payload.candidateFiles === 1 ? '' : 's'}`
    }
    if (payload.phase === 'comparing') {
      return `Comparing bytes: ${payload.comparedFiles}/${payload.candidateFiles}`
    }
    return `Finished: ${payload.matchedFiles} identical ${payload.matchedFiles === 1 ? 'file' : 'files'}`
  }

  const cleanupDuplicateProgressListener = async () => {
    if (unlistenDuplicateProgress) {
      await unlistenDuplicateProgress()
      unlistenDuplicateProgress = null
    }
    activeDuplicateProgressEvent = null
  }

  const stopDuplicateScan = async (invalidate = true) => {
    if (invalidate) {
      duplicateScanToken += 1
    }
    const cancelId = activeDuplicateProgressEvent
    deps.duplicateModalStop()
    if (cancelId) {
      try {
        await cancelTask(cancelId)
      } catch {
        // Task likely already completed or cleaned up.
      }
    }
    await cleanupDuplicateProgressListener()
  }

  const closeCheckDuplicatesModal = () => {
    void stopDuplicateScan(true)
      .catch(() => {})
      .finally(() => {
        deps.duplicateModalClose()
      })
  }

  const searchCheckDuplicates = async () => {
    const { target, searchRoot, scanning } = deps.getDuplicateScanInput()
    const trimmedRoot = searchRoot.trim()

    if (!target) {
      deps.showToast('No target file selected')
      return
    }
    if (!trimmedRoot) {
      deps.showToast('Choose a start folder first')
      return
    }
    if (scanning) {
      return
    }

    await stopDuplicateScan(true)
    const runToken = ++duplicateScanToken
    const progressEvent = `duplicates-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    activeDuplicateProgressEvent = progressEvent
    deps.duplicateModalStart()

    try {
      unlistenDuplicateProgress = await listen<DuplicateScanProgress>(progressEvent, (event) => {
        if (runToken !== duplicateScanToken) {
          return
        }

        const payload = event.payload
        deps.duplicateModalSetProgress(payload.percent, duplicateProgressLabel(payload))

        if (payload.error) {
          deps.duplicateModalFail(payload.error)
          deps.showToast(`Duplicate scan failed: ${payload.error}`)
          if (activeDuplicateProgressEvent === progressEvent) {
            void cleanupDuplicateProgressListener()
          }
          return
        }

        if (!payload.done) {
          return
        }

        const paths = payload.duplicates ?? []
        deps.duplicateModalFinish(paths)
        if (paths.length === 0) {
          deps.showToast('No identical files found', 1600)
        } else {
          deps.showToast(
            `Found ${paths.length} identical ${paths.length === 1 ? 'file' : 'files'}`,
            1800,
          )
        }

        if (activeDuplicateProgressEvent === progressEvent) {
          void cleanupDuplicateProgressListener()
        }
      })

      if (runToken !== duplicateScanToken) {
        await cleanupDuplicateProgressListener()
        return
      }

      await checkDuplicatesStream({
        targetPath: target.path,
        startPath: trimmedRoot,
        progressEvent,
      })
    } catch (err) {
      if (runToken !== duplicateScanToken) {
        return
      }
      const msg = getErrorMessage(err)
      deps.duplicateModalFail(msg)
      deps.showToast(`Duplicate scan failed: ${msg}`)
      await cleanupDuplicateProgressListener()
    }
  }

  const computeDirStats = async (
    paths: string[],
    onProgress?: (bytes: number, items: number) => void,
  ): Promise<{ total: number; items: number }> => {
    if (paths.length === 0) return { total: 0, items: 0 }
    if (activeDirSizeProgressEvent) {
      void cancelTask(activeDirSizeProgressEvent).catch(() => {})
      activeDirSizeProgressEvent = null
    }
    const token = ++dirSizeAbort
    const progressEvent = `dir-size-progress-${token}-${Math.random().toString(16).slice(2)}`
    activeDirSizeProgressEvent = progressEvent
    let unlisten: UnlistenFn | null = null
    const partials = new Map<string, { bytes: number; items: number }>()
    try {
      if (onProgress) {
        unlisten = await listen<{ path: string; bytes: number; items: number }>(progressEvent, (event) => {
          if (token !== dirSizeAbort) return
          const { path, bytes, items } = event.payload
          partials.set(path, { bytes, items })
          const totals = Array.from(partials.values()).reduce(
            (acc, value) => {
              acc.bytes += value.bytes
              acc.items += value.items
              return acc
            },
            { bytes: 0, items: 0 },
          )
          onProgress(totals.bytes, totals.items)
        })
      }
      const result = await dirSizes(paths, progressEvent)
      if (token !== dirSizeAbort) {
        void cancelTask(progressEvent).catch(() => {})
        return { total: 0, items: 0 }
      }
      return { total: result.total ?? 0, items: result.total_items ?? 0 }
    } catch (err) {
      console.error('Failed to compute dir sizes', err)
      return { total: 0, items: 0 }
    } finally {
      if (activeDirSizeProgressEvent === progressEvent) {
        activeDirSizeProgressEvent = null
      } else {
        void cancelTask(progressEvent).catch(() => {})
      }
      if (unlisten) {
        await unlisten()
      }
    }
  }

  const abortDirStats = () => {
    if (activeDirSizeProgressEvent) {
      void cancelTask(activeDirSizeProgressEvent).catch(() => {})
      activeDirSizeProgressEvent = null
    }
    dirSizeAbort += 1
  }

  return {
    conflictModalOpen,
    conflictList,
    computeDirStats,
    abortDirStats,
    pasteIntoCurrent,
    handlePasteOrMove,
    resolveConflicts,
    canExtractPaths,
    extractEntries,
    stopDuplicateScan,
    closeCheckDuplicatesModal,
    searchCheckDuplicates,
    cancelConflicts: clearConflictState,
    hasPendingConflicts: () => get(conflictModalOpen),
  }
}
