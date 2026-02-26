import { writable } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import { createFolder } from '../services/files.service'

type ActivityApi = {
  start: (label: string, eventName: string, onCancel?: () => void) => Promise<void>
  hideSoon: () => void
  cleanup: (preserveTimer?: boolean) => Promise<void>
  clearNow: () => void
  hasHideTimer: () => boolean
}

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
  const isCloudPath = (path: string) => path.startsWith('rclone://')

  const defaultName = () => 'New folder'

  const open = () => {
    state.set({ open: true, error: '' })
    return defaultName()
  }

  const close = () => state.set({ open: false, error: '' })

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
    const progressEvent = `mkdir-progress-${Date.now()}-${Math.random().toString(16).slice(2)}`
    try {
      if (activityApi) {
        await activityApi.start('Creating folder…', progressEvent)
      }
      const created: string = await createFolder(base, trimmed)
      if (isCloudPath(base)) {
        void (async () => {
          if (getCurrentPath() !== base) {
            return
          }
          try {
            await loadPath(base)
          } catch {
            if (getCurrentPath() !== base) {
              return
            }
            showToast('Folder created, but refresh took too long. Press F5 to refresh.')
          }
        })()
      } else {
        await loadPath(base)
      }
      close()
      activityApi?.hideSoon()
      return created
    } catch (err) {
      const msg = getErrorMessage(err)
      state.update((s) => ({ ...s, error: msg }))
      if (activityApi) {
        activityApi.clearNow()
        await activityApi.cleanup()
      }
      return null
    } finally {
      if (activityApi) {
        const hadTimer = activityApi.hasHideTimer()
        await activityApi.cleanup(true)
        if (!hadTimer) {
          activityApi.clearNow()
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
