import { writable, get } from 'svelte/store'
import type { Entry } from '../model/types'
import { renameEntry } from '../services/files'

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
      await renameEntry(current.target.path, trimmed)
      await loadPath(parentPath(current.target.path))
      close()
      return true
    } catch (err) {
      const msg = invokeErrorMessage(err)
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
