import { writable, get } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import type { Entry } from '../model/types'
import { deleteEntries, moveToTrashMany, purgeTrashItems } from '../services/trash.service'

type ActivityApi = {
  start: (label: string, eventName: string, onCancel?: () => void) => Promise<void>
  cleanup: (preserveTimer?: boolean) => Promise<void>
  clearNow: () => void
  hasHideTimer: () => boolean
  requestCancel?: (eventName: string) => Promise<void>
}

type Deps = {
  activityApi: ActivityApi
  reloadCurrent: () => Promise<void>
  getCurrentPath?: () => string | null
  showToast: (msg: string) => void
}

export type DeleteConfirmMode = 'default' | 'trash' | 'network' | 'network-trash'

export type DeleteConfirmState = {
  open: boolean
  targets: Entry[]
  mode: DeleteConfirmMode
}

export const createDeleteConfirmModal = (deps: Deps) => {
  const { activityApi, reloadCurrent, getCurrentPath, showToast } = deps
  const state = writable<DeleteConfirmState>({ open: false, targets: [], mode: 'default' })
  let deleting = false
  const isCloudPath = (path: string) => path.startsWith('rclone://')

  const open = (entries: Entry[], mode: DeleteConfirmState['mode'] = 'default') => {
    if (deleting) { showToast('Wait for the current deletion to finish or cancel it first.'); return }
    if (mode === 'default' && entries.some(entry => entry.network || entry.path.includes('/gvfs/'))) mode = 'network'
    state.set({ open: true, targets: entries, mode })
  }

  const close = () => state.set({ open: false, targets: [], mode: 'default' })

  const confirm = async () => {
    const current = get(state)
    if (!current.open || current.targets.length === 0 || deleting) return
    deleting = true
    // The confirmation is complete. Let the operation's shared progress UI
    // receive cancellation instead of trapping it behind the modal overlay.
    if (current.mode === 'network' || current.mode === 'network-trash') close()
    const progressEvent = `delete-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      await activityApi.start(current.mode === 'network-trash' ? 'Moving to trash / deleting…' : 'Deleting…', progressEvent,
        activityApi.requestCancel ? () => void activityApi.requestCancel?.(progressEvent) : undefined)
      if (current.mode === 'trash') {
        const ids = current.targets.map((t) => t.trash_id ?? t.path)
        await purgeTrashItems(ids)
      } else if (current.mode === 'network-trash') {
        await moveToTrashMany(current.targets.map(t => t.path), progressEvent, true)
      } else {
        const paths = current.targets.map((t) => t.path)
        await deleteEntries(paths, progressEvent, true)
      }
      const cloudDelete =
        current.mode !== 'trash' && current.targets.length > 0 && current.targets.every((t) => isCloudPath(t.path))
      if (cloudDelete) {
        const refreshTarget = getCurrentPath?.() ?? null
        void (async () => {
          if (refreshTarget && getCurrentPath && getCurrentPath() !== refreshTarget) {
            return
          }
          try {
            await reloadCurrent()
          } catch {
            if (refreshTarget && getCurrentPath && getCurrentPath() !== refreshTarget) {
              return
            }
            showToast('Delete completed, but refresh took too long. Press F5 to refresh.')
          }
        })()
      } else {
        await reloadCurrent()
      }
    } catch (err) {
      const msg = getErrorMessage(err)
      console.error('Failed to delete', err)
      showToast(`Delete failed: ${msg}`)
    } finally {
      deleting = false
      const hadTimer = activityApi.hasHideTimer()
      await activityApi.cleanup(true)
      if (!hadTimer) {
        activityApi.clearNow()
      }
      close()
    }
  }

  return {
    state,
    open,
    close,
    confirm,
  }
}
