import { invoke } from '@tauri-apps/api/core'
import type { Entry } from '../types'
import type { ClipboardApi } from './useClipboard'

type CurrentView = 'recent' | 'starred' | 'trash' | 'dir'

type Deps = {
  getSelectedPaths: () => string[]
  getSelectedSet: () => Set<string>
  getFilteredEntries: () => Entry[]
  currentView: () => CurrentView
  selectedDirBytes: () => number
  reloadCurrent: () => Promise<void>
  clipboard: ClipboardApi
  showToast: (msg: string) => void
  openWith: (entry: Entry) => void
  startRename: (entry: Entry) => void
  confirmDelete: (entries: Entry[]) => void
  openProperties: (entry: Entry, size: number | null) => Promise<void> | void
}

export const createContextActions = (deps: Deps) => {
  const {
    getSelectedPaths,
    getSelectedSet,
    getFilteredEntries,
    currentView,
    selectedDirBytes,
    reloadCurrent,
    clipboard,
    showToast,
    openWith,
    startRename,
    confirmDelete,
    openProperties,
  } = deps

  return async (id: string, entry: Entry | null) => {
    if (id.startsWith('divider')) return
    if (!entry) return

    const selectionPaths = getSelectedPaths()
    const paths = selectionPaths.includes(entry.path) ? selectionPaths : [entry.path]
    const filtered = getFilteredEntries()
    const selectionEntries = paths.length > 1 ? filtered.filter((e) => paths.includes(e.path)) : [entry]

    if (id === 'copy-path') {
      const result = await clipboard.copy(selectionEntries, { writeText: true })
      if (!result.ok) showToast(`Copy failed: ${result.error}`)
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

    if (id === 'rename') {
      startRename(entry)
      return
    }

    if (id === 'compress') {
      console.warn('Compress not implemented yet')
      return
    }

    if (id === 'move-trash') {
      try {
        if (currentView() === 'trash') {
          for (const e of selectionEntries) {
            await invoke('delete_entry', { path: e.path })
          }
        } else {
          for (const e of selectionEntries) {
            await invoke('move_to_trash', { path: e.path })
          }
        }
        await reloadCurrent()
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err)
        showToast(`Failed to move to trash: ${message}`)
      }
      return
    }

    if (id === 'delete-permanent') {
      confirmDelete(selectionEntries)
      return
    }

    if (id === 'properties' && selectionEntries.length === 1) {
      const e = selectionEntries[0]
      const isDir = e.kind === 'dir'
      const selectedOnlyThis = getSelectedSet().size === 1 && getSelectedSet().has(e.path)
      const size = isDir && selectedOnlyThis ? selectedDirBytes() : e.size ?? null
      await openProperties(e, size)
      return
    }
  }
}
