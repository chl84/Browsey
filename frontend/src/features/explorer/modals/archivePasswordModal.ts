import { writable } from 'svelte/store'
import { normalizeError } from '@/shared/lib/error'

export const isArchivePasswordError = (error: unknown) => [
  'archive_password_required', 'archive_invalid_password', 'archive_password_or_corrupt',
].includes(normalizeError(error).code ?? '')

// Only presentation state lives in the store. Passwords are never persisted or reused.
export const createArchivePasswordModal = () => {
  const state = writable<{ open: boolean; path: string; error: string }>({ open: false, path: '', error: '' })
  let resolve: ((password: string | null) => void) | null = null
  const finish = (password: string | null) => {
    const done = resolve
    resolve = null
    state.set({ open: false, path: '', error: '' })
    done?.(password)
  }
  const request = (path: string, error: unknown) => {
    finish(null)
    const normalized = normalizeError(error)
    state.set({ open: true, path, error: normalized.code === 'archive_password_required' ? '' : normalized.message })
    return new Promise<string | null>((done) => { resolve = done })
  }
  return { state, request, submit: (password: string) => finish(password), cancel: () => finish(null) }
}
