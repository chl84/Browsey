import { get, writable } from 'svelte/store'
import { getErrorMessage } from '@/shared/lib/error'

type Deps = {
  emptyTrash: () => Promise<void>
  isTrashView: () => boolean
  refresh: () => Promise<void>
  showToast: (message: string) => void
}

export const createEmptyTrashModal = (deps: Deps) => {
  const state = writable({ open: false, busy: false })
  const open = () => { if (!get(state).busy) state.set({ open: true, busy: false }) }
  const close = () => { if (!get(state).busy) state.set({ open: false, busy: false }) }
  const confirm = async () => {
    const current = get(state)
    if (!current.open || current.busy) return
    state.set({ open: true, busy: true })
    let message = 'Wastebasket emptied'
    try {
      await deps.emptyTrash()
    } catch (error) {
      message = `Could not empty Wastebasket: ${getErrorMessage(error)}. Some items may already have been permanently deleted.`
    }
    // Refresh partial outcomes too, without navigating to another view or
    // interpreting a refresh error as a failed deletion.
    try {
      if (deps.isTrashView()) await deps.refresh()
    } catch {
      message += ' Could not refresh the listing. Press F5 to refresh.'
    } finally {
      state.set({ open: false, busy: false })
    }
    deps.showToast(message)
  }
  return { state, open, close, confirm }
}
