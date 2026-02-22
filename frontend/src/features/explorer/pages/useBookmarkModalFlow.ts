import { tick } from 'svelte'
import type { BookmarkModalState, createBookmarkModal } from '../hooks/createBookmarkModal'

type BookmarkEntry = { label: string; path: string }
type BookmarkCandidate = { name: string; path: string }

type Params = {
  bookmarkModal: ReturnType<typeof createBookmarkModal>
  getBookmarkInputEl: () => HTMLInputElement | null
  getBookmarks: () => BookmarkEntry[]
  setBookmarks: (next: BookmarkEntry[]) => void
  addBookmarkToStore: (entry: BookmarkEntry) => void
  persistBookmark: (label: string, path: string) => void
  setModalUiState: (next: { open: boolean; name: string; candidate: BookmarkCandidate | null }) => void
}

export const useBookmarkModalFlow = (params: Params) => {
  const syncFromStoreState = (state: BookmarkModalState) => {
    params.setModalUiState({
      open: state.open,
      name: state.name,
      candidate: state.candidate,
    })
  }

  const syncDraftNameToModal = (open: boolean, name: string) => {
    if (!open) return
    params.bookmarkModal.setName(name)
  }

  const openBookmarkModal = async (entry: { name: string; path: string }) => {
    params.bookmarkModal.openModal(entry)
    await tick()
    const input = params.getBookmarkInputEl()
    if (input) {
      input.focus()
      input.select()
    }
  }

  const closeBookmarkModal = () => {
    params.bookmarkModal.closeModal()
  }

  const confirmBookmark = () => {
    const add = (label: string, path: string) => {
      params.persistBookmark(label, path)
      params.addBookmarkToStore({ label, path })
    }
    const { bookmarks: updated } = params.bookmarkModal.confirm(params.getBookmarks(), add)
    params.setBookmarks(updated)
    closeBookmarkModal()
  }

  return {
    syncFromStoreState,
    syncDraftNameToModal,
    openBookmarkModal,
    closeBookmarkModal,
    confirmBookmark,
  }
}
