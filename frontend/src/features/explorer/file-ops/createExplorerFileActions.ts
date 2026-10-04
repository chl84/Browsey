import { getErrorMessage } from '@/shared/lib/error'
import type { Entry } from '../model/types'
import type { CurrentView } from '../context/createContextActions'
import type { ActivityApi } from '../hooks/createActivity'
import { openEntry } from '../services/files.service'
import { openConsole } from '../services/console.service'
import type { SelectionActions } from './createSelectionActions'

type Deps = {
  selectionActions: SelectionActions
  activityApi: ActivityApi
  currentView: () => CurrentView
  getCurrentPath: () => string
  getSelected: () => Set<string>
  getEntries: () => Entry[]
  getFilteredEntries: () => Entry[]
  pasteIntoCurrent: () => Promise<boolean>
  startRename: (entry: Entry) => void
  openProperties: (entries: Entry[]) => void | Promise<void>
  setSelection: (paths: Set<string>, anchor: number | null, caret: number | null) => void
  showToast: (message: string) => void
}

// Page-level selection/modal orchestration. Mutation policy stays in the
// shared selection controller and existing file-operation/service modules.
export const createExplorerFileActions = (deps: Deps) => {
  const selectedPaths = () => Array.from(deps.getSelected())
  const selectedEntries = (entries: Entry[]) => {
    const paths = deps.getSelected()
    return entries.filter(entry => paths.has(entry.path))
  }

  const open = async (entry: Entry) => {
    if (!entry.path.startsWith('rclone://') || entry.kind === 'dir') {
      await openEntry(entry)
      return
    }
    const event = `cloud-open-${Date.now()}-${Math.random().toString(16).slice(2)}`
    let completed = false
    try {
      await deps.activityApi.start('Opening cloud file…', event, () => void deps.activityApi.requestCancel(event))
      await openEntry(entry, { progressEvent: event })
      completed = true
      deps.activityApi.hideSoon()
      deps.showToast('Opened a local working copy. Upload edits from Settings → Cloud → Working copies.')
    } catch (error) {
      deps.activityApi.clearNow()
      deps.showToast(getErrorMessage(error))
    } finally {
      // Preserve successful feedback, but release the event listener on both
      // outcomes instead of retaining it until the next operation.
      await deps.activityApi.cleanup(completed)
    }
  }

  return {
    open,
    copy: () => deps.selectionActions.copy(selectedPaths()),
    cut: () => deps.selectionActions.cut(selectedPaths()),
    paste: () => deps.currentView() === 'dir' ? deps.pasteIntoCurrent() : false,
    rename: () => {
      if (deps.currentView() === 'network' || deps.getSelected().size !== 1) return false
      const entry = deps.getEntries().find(item => item.path === selectedPaths()[0])
      if (!entry) return false
      deps.startRename(entry)
      return true
    },
    delete: (permanent = false) => {
      const entries = selectedEntries(deps.getFilteredEntries())
      return permanent ? deps.selectionActions.deletePermanently(entries, true) : deps.selectionActions.trash(entries)
    },
    deletePermanentFast: () => deps.selectionActions.deletePermanently(selectedEntries(deps.getFilteredEntries())),
    properties: () => {
      const entries = selectedEntries(deps.getEntries())
      if (entries.length === 0) return false
      void deps.openProperties(entries)
      return true
    },
    openConsole: async () => {
      if (deps.currentView() !== 'dir') return false
      const path = deps.getCurrentPath()
      if (path.startsWith('rclone://')) {
        deps.showToast('Open in console is not available for cloud folders')
        return true
      }
      try {
        await openConsole(path)
        return true
      } catch (error) {
        deps.showToast(`Open console failed: ${getErrorMessage(error)}`)
        return false
      }
    },
    selectAll: () => {
      const entries = deps.getFilteredEntries()
      if (entries.length === 0) return false
      deps.setSelection(new Set(entries.map(entry => entry.path)), 0, entries.length - 1)
      return true
    },
    selectCreated: (path: string | null | undefined) => {
      if (path) deps.setSelection(new Set([path]), null, null)
    },
  }
}

export type ExplorerFileActions = ReturnType<typeof createExplorerFileActions>
