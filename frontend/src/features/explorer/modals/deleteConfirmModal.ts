import { writable, get } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import type { Entry } from '../model/types'
import { deleteEntries, purgeTrashItems } from '../services/trash'

type ActivityApi = {
  start: (label: string, eventName: string, onCancel?: () => void) => Promise<void>
  cleanup: (preserveTimer?: boolean) => Promise<void>
  clearNow: () => void
  hasHideTimer: () => boolean
}

type Deps = {
  activityApi: ActivityApi
  reloadCurrent: () => Promise<void>
  showToast: (msg: string) => void
}

export type DeleteConfirmState = {
  open: boolean
  targets: Entry[]
  mode: 'default' | 'trash'
}

export const createDeleteConfirmModal = (deps: Deps) => {
  const { activityApi, reloadCurrent, showToast } = deps
  const state = writable<DeleteConfirmState>({ open: false, targets: [], mode: 'default' })
  let deleting = false

  const open = (entries: Entry[], mode: DeleteConfirmState['mode'] = 'default') => {
    state.set({ open: true, targets: entries, mode })
  }

  const close = () => state.set({ open: false, targets: [], mode: 'default' })

  const confirm = async () => {
    const current = get(state)
    if (!current.open || current.targets.length === 0 || deleting) return
    deleting = true
    const progressEvent = `delete-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      await activityApi.start('Deleting…', progressEvent)
      if (current.mode === 'trash') {
        const ids = current.targets.map((t) => t.trash_id ?? t.path)
        await purgeTrashItems(ids)
      } else {
        const paths = current.targets.map((t) => t.path)
        await deleteEntries(paths, progressEvent)
      }
      await reloadCurrent()
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
