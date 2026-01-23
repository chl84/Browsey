import { writable, get } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'
import type { Entry } from '../types'

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
}

export const createDeleteConfirmModal = (deps: Deps) => {
  const { activityApi, reloadCurrent, showToast } = deps
  const state = writable<DeleteConfirmState>({ open: false, targets: [] })
  let deleting = false

  const open = (entries: Entry[]) => {
    state.set({ open: true, targets: entries })
  }

  const close = () => state.set({ open: false, targets: [] })

  const confirm = async () => {
    const current = get(state)
    if (!current.open || current.targets.length === 0 || deleting) return
    deleting = true
    const paths = current.targets.map((t) => t.path)
    const progressEvent = `delete-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      await activityApi.start('Deleting…', progressEvent)
      await invoke('delete_entries', { paths, progressEvent })
      await reloadCurrent()
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
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
