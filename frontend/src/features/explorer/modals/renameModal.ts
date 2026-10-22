import type { ActivityApi as SharedActivityApi } from '../hooks/createActivity'
import { writable, get } from 'svelte/store'
import type { Entry } from '../model/types'
import { renameEntry } from '../services/files.service'

type ActivityApi = Pick<SharedActivityApi, 'start' | 'hideSoon' | 'cleanup' | 'clearNow' | 'hasHideTimer'>

type Deps = {
  loadPath: (path: string) => Promise<void>
  reloadCurrent?: () => Promise<void>
  parentPath: (path: string) => string
  getCurrentPath?: () => string | null
  showToast?: (msg: string, durationMs?: number) => void
  activityApi?: ActivityApi
}

export type RenameModalState = {
  open: boolean
  target: Entry | null
  error: string
}

export const createRenameModal = (deps: Deps) => {
  const { loadPath, reloadCurrent, parentPath, getCurrentPath, showToast, activityApi } = deps
  const state = writable<RenameModalState>({ open: false, target: null, error: '' })
  let busy = false
  let dialogGeneration = 0
  const isCloudPath = (path: string) => path.startsWith('rclone://')

  const invokeErrorMessage = (err: unknown): string => {
    if (err instanceof Error && err.message.trim().length > 0) return err.message
    if (typeof err === 'string' && err.trim().length > 0) {
      const raw = err.trim()
      try {
        const parsed = JSON.parse(raw) as Record<string, unknown>
        if (typeof parsed.message === 'string' && parsed.message.trim().length > 0) {
          return parsed.message
        }
      } catch {
        // Keep raw string fallback below.
      }
      return raw
    }
    if (err && typeof err === 'object') {
      const record = err as Record<string, unknown>
      if (typeof record.message === 'string' && record.message.trim().length > 0) {
        return record.message
      }
      if (record.error && typeof record.error === 'object') {
        const nested = record.error as Record<string, unknown>
        if (typeof nested.message === 'string' && nested.message.trim().length > 0) {
          return nested.message
        }
      }
    }
    return 'Unknown error'
  }

  const open = (entry: Entry) => {
    ++dialogGeneration
    state.set({ open: true, target: entry, error: '' })
  }

  const close = () => {
    ++dialogGeneration
    state.set({ open: false, target: null, error: '' })
  }

  const confirm = async (name: string) => {
    const current = get(state)
    if (!current.target || busy) return false
    if (!name.trim()) {
      state.update(s => ({ ...s, error: 'New name cannot be empty' }))
      return false
    }
    busy = true
    const generation = dialogGeneration
    const cloud = isCloudPath(current.target.path)
    const refreshPath = getCurrentPath?.() ?? parentPath(current.target.path)
    state.update((s) => ({ ...s, error: '' }))
    const progressEvent = `rename-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      if (activityApi) {
        await activityApi.start('Renaming…', progressEvent)
      }
      if (generation !== dialogGeneration) return false
      if (cloud) await renameEntry(current.target.path, name, progressEvent)
      else await renameEntry(current.target.path, name)
      if (generation === dialogGeneration) close()
      activityApi?.hideSoon(progressEvent)
      const refresh = async () => {
        if (getCurrentPath && getCurrentPath() !== refreshPath) return
        try {
          if (reloadCurrent) await reloadCurrent()
          else await loadPath(refreshPath)
        } catch {
          if (getCurrentPath && getCurrentPath() !== refreshPath) return
          showToast?.(cloud
            ? 'Rename completed, but refresh took too long. Press F5 to refresh.'
            : 'Rename completed, but refresh failed. Press F5 to refresh.', 3500)
        }
      }
      if (cloud) void refresh()
      else await refresh()
      return true
    } catch (err) {
      const msg = invokeErrorMessage(err)
      if (generation === dialogGeneration) state.update((s) => ({ ...s, error: msg }))
      else showToast?.(`Rename failed: ${msg}`, 3500)
      return false
    } finally {
      if (activityApi) {
        const hadTimer = activityApi.hasHideTimer(progressEvent)
        await activityApi.cleanup(true, progressEvent)
        if (!hadTimer) {
          activityApi.clearNow(progressEvent)
        }
      }
      busy = false
    }
  }

  return {
    state,
    open,
    close,
    confirm,
  }
}
