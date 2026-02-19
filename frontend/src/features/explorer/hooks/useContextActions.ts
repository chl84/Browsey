import { getErrorMessage } from '@/shared/lib/error'
import type { Entry } from '../model/types'
import type { ClipboardApi } from './useClipboard'
import { copyPathsToSystemClipboard } from '../services/clipboard.service'
import {
  restoreTrashItems,
  removeRecent,
  deleteEntries,
  moveToTrashMany,
  purgeTrashItems,
} from '../services/trash.service'

export type CurrentView = 'recent' | 'starred' | 'trash' | 'network' | 'dir'

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
  startAdvancedRename: (entries: Entry[]) => void
  confirmDelete: (entries: Entry[], mode?: 'default' | 'trash') => void
  openProperties: (entries: Entry[]) => Promise<void> | void
  openLocation: (entry: Entry) => Promise<void> | void
  openCompress: (entries: Entry[]) => void
  openCheckDuplicates: (entry: Entry) => void
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
    startAdvancedRename,
    confirmDelete,
    openProperties,
    openLocation,
    openCompress,
    openCheckDuplicates,
    extractEntries,
  } = deps

  return async (id: string, entry: Entry | null) => {
    if (id.startsWith('divider')) return
    if (!entry) return

    const selectedSet = getSelectedSet()
    const paths = selectedSet.has(entry.path) ? Array.from(selectedSet) : [entry.path]
    const filtered = getFilteredEntries()
    const pathSet = new Set(paths)
    let selectionEntriesCache: Entry[] | null = null
    const selectionEntries = () => {
      if (selectionEntriesCache) return selectionEntriesCache
      selectionEntriesCache = paths.length > 1 ? filtered.filter((e) => pathSet.has(e.path)) : [entry]
      return selectionEntriesCache
    }

    if (id === 'restore') {
      if (currentView() === 'trash') {
        const ids = selectionEntries().map((e) => e.trash_id ?? e.path)
        try {
          await restoreTrashItems(ids)
          await reloadCurrent()
        } catch (err) {
          showToast(`Restore failed: ${getErrorMessage(err)}`)
        }
      }
      return
    }

    if (id === 'copy-path') {
      const result = await clipboard.copyPaths(paths, { writeText: true })
      if (result.ok) {
        showToast('Path copied', 1500)
      } else {
        showToast(`Copy failed: ${result.error}`)
      }
      return
    }

    if (id === 'remove-recent') {
      if (currentView() === 'recent') {
        const paths = selectionEntries().map((e) => e.path)
        try {
          await removeRecent(paths)
          await reloadCurrent()
        } catch (err) {
          showToast(`Remove failed: ${getErrorMessage(err)}`)
        }
      }
      return
    }

    if (id === 'cut' || id === 'copy') {
      if (id === 'cut') {
        const result = await clipboard.cutPaths(paths)
        if (!result.ok) {
          showToast(`Cut failed: ${result.error}`)
          return
        }
        showToast('Cut', 1500)
        void copyPathsToSystemClipboard(paths, 'cut').catch((err) => {
          showToast(
            `Cut (system clipboard unavailable: ${getErrorMessage(err)})`,
            2500
          )
        })
        return
      }
      const result = await clipboard.copyPaths(paths)
      if (!result.ok) {
        showToast(`Copy failed: ${result.error}`)
        return
      }
      showToast('Copied', 1500)
      void copyPathsToSystemClipboard(paths).catch((err) => {
        showToast(
          `Copied (system clipboard unavailable: ${getErrorMessage(err)})`,
          2500
        )
      })
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
    if (id === 'rename-advanced') {
      startAdvancedRename(selectionEntries())
      return
    }

    if (id === 'compress') {
      openCompress(selectionEntries())
      return
    }

    if (id === 'check-duplicates') {
      const entries = selectionEntries()
      if (entries.length !== 1 || entries[0].kind !== 'file') {
        showToast('Check for Duplicates is available for one file at a time')
        return
      }
      openCheckDuplicates(entries[0])
      return
    }

    if (id === 'extract') {
      await extractEntries(selectionEntries())
      return
    }

    if (id === 'move-trash') {
      try {
        if (currentView() === 'trash') {
          const ids = selectionEntries().map((e) => e.trash_id ?? e.path)
          await purgeTrashItems(ids)
        } else {
          const paths = selectionEntries().map((e) => e.path)
          await moveToTrashMany(paths)
        }
        await reloadCurrent()
      } catch (err) {
        const message = getErrorMessage(err)
        showToast(
          currentView() === 'trash'
            ? `Delete failed: ${message}`
            : `Failed to move to trash: ${message}`
        )
      }
      return
    }

    if (id === 'delete-permanent') {
      if (currentView() === 'trash') {
        const ids = selectionEntries().map((e) => e.trash_id ?? e.path)
        try {
          await purgeTrashItems(ids)
          await reloadCurrent()
        } catch (err) {
          showToast(`Delete failed: ${getErrorMessage(err)}`)
        }
        return
      }
      if (!confirmDeleteEnabled()) {
        const paths = selectionEntries().map((e) => e.path)
        try {
          await deleteEntries(paths)
          await reloadCurrent()
        } catch (err) {
          const msg = getErrorMessage(err)
          showToast(`Delete failed: ${msg}`)
        }
        return
      }
      confirmDelete(selectionEntries())
      return
    }

    if (id === 'properties') {
      await openProperties(selectionEntries())
      return
    }
  }
}
