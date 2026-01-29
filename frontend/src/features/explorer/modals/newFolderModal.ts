import { writable } from 'svelte/store'
import { createFolder } from '../services/files'

type Deps = {
  getCurrentPath: () => string | null
  loadPath: (path: string) => Promise<void>
  showToast: (msg: string) => void
}

export type NewFolderState = {
  open: boolean
  error: string
}

export const createNewFolderModal = (deps: Deps) => {
  const { getCurrentPath, loadPath, showToast } = deps
  const state = writable<NewFolderState>({ open: false, error: '' })
  let busy = false

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
    try {
      const created: string = await createFolder(base, trimmed)
      await loadPath(base)
      close()
      return created
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
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
