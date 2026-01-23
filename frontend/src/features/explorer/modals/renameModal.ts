import { invoke } from '@tauri-apps/api/core'
import { writable, get } from 'svelte/store'
import type { Entry } from '../types'

type Deps = {
  loadPath: (path: string) => Promise<void>
  parentPath: (path: string) => string
}

export type RenameModalState = {
  open: boolean
  target: Entry | null
  error: string
}

export const createRenameModal = (deps: Deps) => {
  const { loadPath, parentPath } = deps
  const state = writable<RenameModalState>({ open: false, target: null, error: '' })
  let busy = false

  const open = (entry: Entry) => {
    state.set({ open: true, target: entry, error: '' })
  }

  const close = () => {
    state.set({ open: false, target: null, error: '' })
  }

  const confirm = async (name: string) => {
    const trimmed = name.trim()
    if (!trimmed) return false
    const current = get(state)
    if (!current.target || busy) return false
    busy = true
    try {
      await invoke('rename_entry', { path: current.target.path, newName: trimmed })
      await loadPath(parentPath(current.target.path))
      close()
      return true
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      state.update((s) => ({ ...s, error: msg }))
      return false
    } finally {
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
