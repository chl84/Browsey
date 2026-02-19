import { get, writable, type Writable } from 'svelte/store'
import { normalizePath } from '../utils'

export type BookmarkModalState = {
  open: boolean
  name: string
  candidate: { name: string; path: string } | null
}

export const createBookmarkModal = () => {
  const store: Writable<BookmarkModalState> = writable({
    open: false,
    name: '',
    candidate: null,
  })

  const openModal = (entry: { name: string; path: string }) => {
    store.set({ open: true, name: entry.name, candidate: entry })
  }

  const closeModal = () => {
    store.set({ open: false, name: '', candidate: null })
  }

  const setName = (name: string) => {
    store.update((state) => ({ ...state, name }))
  }

  const confirm = (bookmarks: { label: string; path: string }[], add: (label: string, path: string) => void) => {
    const state = get(store)
    if (!state.candidate) return { bookmarks, updated: false }
    const label = state.name.trim() || state.candidate.name
    const path = state.candidate.path
    if (!bookmarks.some((b) => normalizePath(b.path) === normalizePath(path))) {
      add(label, path)
      return { bookmarks: [...bookmarks, { label, path }], updated: true }
    }
    return { bookmarks, updated: false }
  }

  return {
    store,
    openModal,
    closeModal,
    setName,
    confirm,
  }
}
