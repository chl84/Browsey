import { invoke } from '@tauri-apps/api/core'
import type { Entry } from '../types'
import type { ClipboardApi } from './useClipboard'

export type CurrentView = 'recent' | 'starred' | 'trash' | 'dir'

type Deps = {
  getSelectedPaths: () => string[]
  getSelectedSet: () => Set<string>
  getFilteredEntries: () => Entry[]
  currentView: () => CurrentView
  reloadCurrent: () => Promise<void>
  clipboard: ClipboardApi
  showToast: (msg: string, durationMs?: number) => void
  openWith: (entry: Entry) => void
  startRename: (entry: Entry) => void
  confirmDelete: (entries: Entry[]) => void
  openProperties: (entries: Entry[]) => Promise<void> | void
  openLocation: (entry: Entry) => Promise<void> | void
  openCompress: (entries: Entry[]) => void
}

export const createContextActions = (deps: Deps) => {
  const {
    getSelectedPaths,
    getSelectedSet,
    getFilteredEntries,
    currentView,
    reloadCurrent,
    clipboard,
    showToast,
    openWith,
    startRename,
    confirmDelete,
    openProperties,
    openLocation,
    openCompress,
  } = deps

  return async (id: string, entry: Entry | null) => {
    if (id.startsWith('divider')) return
    if (!entry) return

    const selectionPaths = getSelectedPaths()
    const paths = selectionPaths.includes(entry.path) ? selectionPaths : [entry.path]
    const filtered = getFilteredEntries()
    const selectionEntries = paths.length > 1 ? filtered.filter((e) => paths.includes(e.path)) : [entry]

    if (id === 'restore') {
      if (currentView() === 'trash') {
        const ids = selectionEntries.map((e) => e.trash_id ?? e.path)
        try {
          await invoke('restore_trash_items', { ids })
          await reloadCurrent()
        } catch (err) {
          showToast(`Restore failed: ${err instanceof Error ? err.message : String(err)}`)
        }
      }
      return
    }

    if (id === 'copy-path') {
      const result = await clipboard.copy(selectionEntries, { writeText: true })
      if (result.ok) {
        showToast('Path copied', 1500)
      } else {
        showToast(`Copy failed: ${result.error}`)
      }
      return
    }

    if (id === 'remove-recent') {
      if (currentView() === 'recent') {
        const paths = selectionEntries.map((e) => e.path)
        try {
          await invoke('remove_recent', { paths })
          await reloadCurrent()
        } catch (err) {
          showToast(`Remove failed: ${err instanceof Error ? err.message : String(err)}`)
        }
      }
      return
    }

    if (id === 'cut' || id === 'copy') {
      if (id === 'cut') {
        const result = await clipboard.cut(selectionEntries)
        if (!result.ok) showToast(`Cut failed: ${result.error}`)
        return
      }
      const result = await clipboard.copy(selectionEntries, { writeText: true })
      if (!result.ok) showToast(`Copy failed: ${result.error}`)
      return
    }

    if (id === 'open-with') {
      openWith(entry)
      return
    }

    if (id === 'open-location') {
      await openLocation(entry)
      return
    }

    if (id === 'rename') {
      startRename(entry)
      return
    }

    if (id === 'compress') {
      openCompress(selectionEntries)
      return
    }

    if (id === 'move-trash') {
      try {
        if (currentView() === 'trash') {
          for (const e of selectionEntries) {
            await invoke('delete_entry', { path: e.path })
          }
        } else {
          const paths = selectionEntries.map((e) => e.path)
          await invoke('move_to_trash_many', { paths })
        }
        await reloadCurrent()
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err)
        showToast(`Failed to move to trash: ${message}`)
      }
      return
    }

    if (id === 'delete-permanent') {
      if (currentView() === 'trash') {
        const ids = selectionEntries.map((e) => e.trash_id ?? e.path)
        try {
          await invoke('purge_trash_items', { ids })
          await reloadCurrent()
        } catch (err) {
          showToast(`Delete failed: ${err instanceof Error ? err.message : String(err)}`)
        }
        return
      }
      confirmDelete(selectionEntries)
      return
    }

    if (id === 'properties') {
      await openProperties(selectionEntries)
      return
    }
  }
}
