import { writable } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'
import { createFile } from '../services/files.service'

type Deps = {
  getCurrentPath: () => string | null
  loadPath: (path: string) => Promise<void>
  showToast: (msg: string) => void
}

export type NewFileState = {
  open: boolean
  error: string
}

export const createNewFileModal = (deps: Deps) => {
  const { getCurrentPath, loadPath, showToast } = deps
  const state = writable<NewFileState>({ open: false, error: '' })
  let busy = false

  const defaultName = () => ''

  const open = () => {
    state.set({ open: true, error: '' })
    return defaultName()
  }

  const close = () => state.set({ open: false, error: '' })

  const confirm = async (name: string): Promise<string | null> => {
    const trimmed = name.trim()
    if (!trimmed) {
      state.update((s) => ({ ...s, error: 'File name cannot be empty' }))
      return null
    }
    const base = getCurrentPath()
    if (!base) {
      showToast('No current path')
      return null
    }
    if (busy) return null
    busy = true
    try {
      const created: string = await createFile(base, trimmed)
      await loadPath(base)
      close()
      return created
    } catch (err) {
      const msg = getErrorMessage(err)
      state.update((s) => ({ ...s, error: msg }))
      return null
    } finally {
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
