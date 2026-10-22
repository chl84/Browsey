import type { ActivityApi as SharedActivityApi } from '../hooks/createActivity'
import { writable } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import { createFolder } from '../services/files.service'

type ActivityApi = Pick<SharedActivityApi, 'start' | 'hideSoon' | 'cleanup' | 'clearNow' | 'hasHideTimer'>

type Deps = {
  getCurrentPath: () => string | null
  loadPath: (path: string) => Promise<void>
  showToast: (msg: string) => void
  activityApi?: ActivityApi
}

export type NewFolderState = {
  open: boolean
  error: string
}

export const createNewFolderModal = (deps: Deps) => {
  const { getCurrentPath, loadPath, showToast, activityApi } = deps
  const state = writable<NewFolderState>({ open: false, error: '' })
  let busy = false
  let dialogGeneration = 0
  const isCloudPath = (path: string) => path.startsWith('rclone://')

  const defaultName = () => 'New folder'

  const open = () => {
    ++dialogGeneration
    state.set({ open: true, error: '' })
    return defaultName()
  }

  const close = () => {
    ++dialogGeneration
    state.set({ open: false, error: '' })
  }

  const confirm = async (name: string): Promise<string | null> => {
    const trimmed = name.trim()
    if (!trimmed) {
      state.update((s) => ({ ...s, error: 'Folder name cannot be empty' }))
      return null
    }
    const base = getCurrentPath()
    if (!base) {
      showToast('No current path')
      return null
    }
    if (busy) return null
    busy = true
    const generation = dialogGeneration
    state.update((s) => ({ ...s, error: '' }))
    const progressEvent = `mkdir-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      if (activityApi) {
        await activityApi.start('Creating folder…', progressEvent)
      }
      if (generation !== dialogGeneration) return null
      const created: string = await createFolder(base, name)
      if (generation === dialogGeneration) close()
      activityApi?.hideSoon(progressEvent)
      const refresh = async () => {
        if (getCurrentPath() !== base) return
        try {
          await loadPath(base)
        } catch {
          if (getCurrentPath() !== base) return
          showToast(isCloudPath(base)
            ? 'Folder created, but refresh took too long. Press F5 to refresh.'
            : 'Folder created, but refresh failed. Press F5 to refresh.')
        }
      }
      if (isCloudPath(base)) void refresh()
      else await refresh()
      return created
    } catch (err) {
      const msg = getErrorMessage(err)
      if (generation === dialogGeneration) state.update((s) => ({ ...s, error: msg }))
      else showToast(`Folder creation failed: ${msg}`)
      return null
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
    defaultName,
  }
}
