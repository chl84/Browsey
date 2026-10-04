import { getErrorMessage } from '@/shared/lib/error'
import type { Entry } from '../model/types'
import type { CurrentView } from '../context/createContextActions'
import type { DeleteConfirmMode } from '../modals/deleteConfirmModal'
import type { ActivityApi } from '../hooks/createActivity'
import { mustUsePermanentDelete } from '../helpers/deletionPolicy'
import { copyPathsToSystemClipboard } from '../services/clipboard.service'
import { deleteEntries, moveToTrashMany, purgeTrashItems, needsNetworkDeleteConfirmation } from '../services/trash.service'
import type { ClipboardApi } from './createClipboard'

type Deps = {
  clipboard: ClipboardApi
  activityApi: ActivityApi
  currentView: () => CurrentView
  getCurrentPath: () => string
  confirmDeleteEnabled: () => boolean
  confirmDelete: (entries: Entry[], mode?: DeleteConfirmMode) => void
  reloadCurrent: () => Promise<void>
  showToast: (message: string, durationMs?: number) => void
  isWindows?: () => boolean
}

const isCloudPath = (path: string) => path.startsWith('rclone://')

// Keyboard and context menus own selection resolution, not mutation policy.
export const createSelectionActions = (deps: Deps) => {
  const { clipboard, activityApi, currentView, getCurrentPath, confirmDeleteEnabled,
    confirmDelete, reloadCurrent, showToast } = deps
  let deleting = false

  const copyOrCut = async (paths: string[], mode: 'copy' | 'cut') => {
    if (mode === 'cut' && currentView() === 'network') return false
    const result = mode === 'cut' ? await clipboard.cutPaths(paths) : await clipboard.copyPaths(paths)
    const label = mode === 'cut' ? 'Cut' : 'Copied'
    if (!result.ok) {
      showToast(`${mode === 'cut' ? 'Cut' : 'Copy'} failed: ${result.error}`)
      return false
    }
    showToast(label, 1500)
    if (!paths.some(isCloudPath)) {
      // Failure here must not discard Browsey's already populated clipboard.
      void copyPathsToSystemClipboard(paths, mode === 'cut' ? 'cut' : undefined).catch(error => {
        showToast(`${label} (system clipboard unavailable: ${getErrorMessage(error)})`, 2500)
      })
    }
    return true
  }

  const remove = async (entries: Entry[], permanent: boolean, alwaysConfirm = false) => {
    if (currentView() === 'network' || entries.length === 0 || deleting) return false
    const inTrash = currentView() === 'trash'
    const windows = deps.isWindows?.() ??
      (typeof navigator !== 'undefined' && navigator.userAgent.toLowerCase().includes('windows'))
    if ((!permanent && mustUsePermanentDelete(entries, false, inTrash, windows)) ||
      (permanent && (alwaysConfirm || confirmDeleteEnabled()))) {
      confirmDelete(entries, inTrash ? 'trash' : 'default')
      return true
    }

    const paths = entries.map(entry => entry.path)
    const cloud = !inTrash && paths.some(isCloudPath)
    const label = permanent || inTrash ? 'Deleting…' : 'Moving to trash…'
    const event = `${permanent || inTrash ? 'delete' : 'trash'}-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    const location = getCurrentPath()
    const view = currentView()
    const isSameLocation = () => getCurrentPath() === location && currentView() === view
    deleting = true
    let completed = false
    let networkConfirmation: DeleteConfirmMode | null = null
    try {
      await activityApi.start(label, event, inTrash ? undefined : () => void activityApi.requestCancel(event), { completeOnReply: cloud })
      if (inTrash) {
        await purgeTrashItems(entries.map(entry => entry.trash_id ?? entry.path))
      } else if (permanent) {
        await deleteEntries(paths, event)
      } else {
        await moveToTrashMany(paths, event)
      }
      completed = true
      if (cloud && !permanent) showToast('Moved to cloud trash. Restore items from the provider website.')
      else if (permanent) showToast('Deleted')
      const refresh = async () => {
        if (!isSameLocation()) return
        try {
          await reloadCurrent()
        } catch {
          if (isSameLocation()) showToast('Delete completed, but refresh took too long. Press F5 to refresh.')
        }
      }
      // Cloud refresh must not hold the operation's progress UI indefinitely.
      if (cloud && permanent) void refresh()
      else await refresh()
      activityApi.hideSoon()
      return true
    } catch (error) {
      if (!inTrash && needsNetworkDeleteConfirmation(error)) {
        networkConfirmation = permanent ? 'network' : 'network-trash'
        return true
      }
      // Refresh partial results without replacing the original failure or retrying.
      if (isSameLocation()) {
        const refreshPartial = async () => {
          try { await reloadCurrent() } catch { /* Keep the mutation error. */ }
        }
        if (cloud) void refreshPartial()
        else await refreshPartial()
      }
      showToast(`${permanent || inTrash ? 'Delete failed' : 'Move to trash failed'}: ${getErrorMessage(error)}`)
      return false
    } finally {
      try {
        await activityApi.cleanup(true)
        if (!completed && !activityApi.hasHideTimer()) activityApi.clearNow()
      } finally {
        deleting = false
      }
      // A fast confirmation can start the next operation immediately. Do not
      // let the old listener cleanup or clearNow remove that new progress UI.
      if (networkConfirmation) confirmDelete(entries, networkConfirmation)
    }
  }

  return {
    copy: (paths: string[]) => copyOrCut(paths, 'copy'),
    cut: (paths: string[]) => copyOrCut(paths, 'cut'),
    trash: (entries: Entry[]) => remove(entries, false),
    deletePermanently: (entries: Entry[], alwaysConfirm = false) => remove(entries, true, alwaysConfirm),
  }
}

export type SelectionActions = ReturnType<typeof createSelectionActions>
