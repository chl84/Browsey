import { writable, get } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import {
  setClipboardCmd,
  clearSystemClipboard,
  pasteClipboardCmd,
  pasteClipboardPreview,
  getSystemClipboardPaths,
} from '../services/clipboard.service'
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
  reloadCurrent: () => Promise<void>
  showToast: (msg: string, durationMs?: number) => void
  activityApi: ActivityApi
}

export const useExplorerFileOps = (deps: Deps) => {
  let conflictDest: string | null = null
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

  return {
    conflictModalOpen,
    conflictList,
    pasteIntoCurrent,
    handlePasteOrMove,
    resolveConflicts,
    cancelConflicts: clearConflictState,
    hasPendingConflicts: () => get(conflictModalOpen),
  }
}
