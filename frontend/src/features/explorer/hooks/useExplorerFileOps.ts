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
  canExtractPaths as canExtractPathsCmd,
  extractArchive,
  extractArchives,
} from '../services/files.service'
import { setClipboardState, clearClipboardState } from '../stores/clipboard.store'
import { normalizePath, parentPath } from '../utils'
import type { Entry } from '../model/types'
import type { CurrentView } from './useContextActions'

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
  showToast: (msg: string, durationMs?: number) => void
  activityApi: ActivityApi
}

export const useExplorerFileOps = (deps: Deps) => {
  let conflictDest: string | null = null
  let extracting = false
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

  return {
    conflictModalOpen,
    conflictList,
    pasteIntoCurrent,
    handlePasteOrMove,
    resolveConflicts,
    canExtractPaths,
    extractEntries,
    cancelConflicts: clearConflictState,
    hasPendingConflicts: () => get(conflictModalOpen),
  }
}
