import type { Entry } from '../types'
import type { ClipboardApi } from './useClipboard'
import { copyPathsToSystemClipboard } from '../services/clipboard'
import {
  restoreTrashItems,
  removeRecent,
  deleteEntry,
  deleteEntries,
  moveToTrashMany,
  purgeTrashItems,
} from '../services/trash'

export type CurrentView = 'recent' | 'starred' | 'trash' | 'dir'

type Deps = {
  getSelectedPaths: () => string[]
  getSelectedSet: () => Set<string>
  getFilteredEntries: () => Entry[]
  currentView: () => CurrentView
  confirmDeleteEnabled: () => boolean
  reloadCurrent: () => Promise<void>
  clipboard: ClipboardApi
  showToast: (msg: string, durationMs?: number) => void
  openWith: (entry: Entry) => void
  startRename: (entry: Entry) => void
  confirmDelete: (entries: Entry[]) => void
  openProperties: (entries: Entry[]) => Promise<void> | void
  openLocation: (entry: Entry) => Promise<void> | void
  openCompress: (entries: Entry[]) => void
  extractEntries: (entries: Entry[]) => Promise<void>
}

export const createContextActions = (deps: Deps) => {
  const {
    getSelectedPaths,
    getSelectedSet,
    getFilteredEntries,
    currentView,
    confirmDeleteEnabled,
    reloadCurrent,
    clipboard,
    showToast,
    openWith,
    startRename,
    confirmDelete,
    openProperties,
    openLocation,
    openCompress,
    extractEntries,
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
          await restoreTrashItems(ids)
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
          await removeRecent(paths)
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
        const paths = selectionEntries.map((e) => e.path)
        try {
          await copyPathsToSystemClipboard(paths, 'cut')
          showToast('Cut', 1500)
        } catch (err) {
          showToast(
            `Cut (system clipboard unavailable: ${err instanceof Error ? err.message : String(err)})`,
            2500
          )
        }
        return
      }
      const result = await clipboard.copy(selectionEntries, { writeText: true })
      if (!result.ok) {
        showToast(`Copy failed: ${result.error}`)
        return
      }
      const paths = selectionEntries.map((e) => e.path)
      try {
        await copyPathsToSystemClipboard(paths)
        showToast('Copied', 1500)
      } catch (err) {
        showToast(
          `Copied (system clipboard unavailable: ${err instanceof Error ? err.message : String(err)})`,
          2500
        )
      }
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

    if (id === 'extract') {
      await extractEntries(selectionEntries)
      return
    }

    if (id === 'move-trash') {
      try {
        if (currentView() === 'trash') {
          for (const e of selectionEntries) {
            await deleteEntry(e.path)
          }
        } else {
          const paths = selectionEntries.map((e) => e.path)
          await moveToTrashMany(paths)
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
          await purgeTrashItems(ids)
          await reloadCurrent()
        } catch (err) {
          showToast(`Delete failed: ${err instanceof Error ? err.message : String(err)}`)
        }
        return
      }
      if (!confirmDeleteEnabled()) {
        const paths = selectionEntries.map((e) => e.path)
        try {
          await deleteEntries(paths)
          await reloadCurrent()
        } catch (err) {
          const msg = err instanceof Error ? err.message : String(err)
          showToast(`Delete failed: ${msg}`)
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
