import { getErrorMessage } from '@/shared/lib/error'
import type { Entry } from '../model/types'
import type { ClipboardApi } from '../file-ops/createClipboard'
import type { SelectionActions } from '../file-ops/createSelectionActions'
import {
  restoreTrashItems,
  removeRecent,
} from '../services/trash.service'

export type CurrentView = 'recent' | 'starred' | 'trash' | 'network' | 'dir'

type Deps = {
  getSelectedPaths: () => string[]
  getSelectedSet: () => Set<string>
  getFilteredEntries: () => Entry[]
  currentView: () => CurrentView
  reloadCurrent: () => Promise<void>
  clipboard: ClipboardApi
  selectionActions: SelectionActions
  showToast: (msg: string, durationMs?: number) => void
  openWith: (entry: Entry) => void
  startRename: (entry: Entry) => void
  startAdvancedRename: (entries: Entry[]) => void
  openProperties: (entries: Entry[]) => Promise<void> | void
  openLocation: (entry: Entry) => Promise<void> | void
  openCompress: (entries: Entry[]) => void
  openCheckDuplicates: (entry: Entry) => void
  extractEntries: (entries: Entry[]) => Promise<void>
  prepareExternalCopy?: (entries: Entry[]) => void
}

export const createContextActions = (deps: Deps) => {
  const {
    getSelectedSet,
    getFilteredEntries,
    currentView,
    reloadCurrent,
    clipboard,
    selectionActions,
    showToast,
    openWith,
    startRename,
    startAdvancedRename,
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
      await selectionActions[id](paths)
      return
    }

    if (id === 'open-with') {
      openWith(entry)
      return
    }

    if (id === 'cloud-export') {
      deps.prepareExternalCopy?.(selectionEntries())
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
      await selectionActions.trash(selectionEntries())
      return
    }

    if (id === 'delete-permanent') {
      await selectionActions.deletePermanently(selectionEntries())
      return
    }

    if (id === 'properties') {
      await openProperties(selectionEntries())
      return
    }
  }
}
