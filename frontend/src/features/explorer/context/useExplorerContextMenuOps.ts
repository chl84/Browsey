import { getErrorMessage } from '@/shared/lib/error'
import {
  buildNetworkEntryContextActions,
  copyTextToSystemClipboard,
  isMountUri,
  networkBlankContextActions,
} from '@/features/network'
import { shortcutFor, type ShortcutBinding, type ShortcutCommandId } from '@/features/shortcuts'
import { ensureSelectionBeforeMenu } from '../helpers/contextMenuHelpers'
import type { Entry } from '../model/types'
import { openConsole } from '../services/console.service'
import { fetchContextMenuActions } from '../services/contextMenu.service'
import { ejectDrive } from '../services/drives.service'
import type { CurrentView } from './createContextActions'
import type { ContextAction } from './createContextMenus'

type Deps = {
  currentView: () => CurrentView
  isSearchSessionEnabled: () => boolean
  shortcutBindings: () => ShortcutBinding[]
  getCurrentPath: () => string
  getContextMenuEntry: () => Entry | null
  getClipboardPathCount: () => number
  getSelectedSet: () => Set<string>
  getFilteredEntries: () => Entry[]
  setSelection: (paths: Set<string>, anchor: number | null, caret: number | null) => void
  openContextMenu: (entry: Entry, actions: ContextAction[], x: number, y: number) => void
  closeContextMenu: () => void
  openBlankContextMenu: (actions: ContextAction[], x: number, y: number) => void
  closeBlankContextMenu: () => void
  loadNetwork: (recordHistory?: boolean, opts?: { resetScroll?: boolean; forceRefresh?: boolean }) => Promise<void>
  openPartition: (path: string) => Promise<void>
  loadPartitions: (opts?: { forceNetworkRefresh?: boolean }) => Promise<void>
  pasteIntoCurrent: () => Promise<boolean>
  openNewFolderModal: () => void
  openNewFileModal: () => void
  openAdvancedRename: (entries: Entry[]) => void
  startRename: (entry: Entry) => void
  contextActions: (id: string, entry: Entry | null) => Promise<void>
  showToast: (msg: string, durationMs?: number) => void
  onBeforeRowContextMenu?: () => void
}

const isCloudPath = (path: string) => path.startsWith('rclone://')

const filterByCapabilities = (actions: ContextAction[], entries: Entry[]): ContextAction[] => {
  if (entries.length === 0) return actions
  const caps = entries
    .map((entry) => entry.capabilities ?? null)
    .filter((value): value is NonNullable<Entry['capabilities']> => Boolean(value))
  if (caps.length !== entries.length) {
    return actions
  }
  const all = {
    canMkdir: caps.every((c) => c.canMkdir),
    canDelete: caps.every((c) => c.canDelete),
    canRename: caps.every((c) => c.canRename),
    canMove: caps.every((c) => c.canMove),
    canCopy: caps.every((c) => c.canCopy),
    canTrash: caps.every((c) => c.canTrash),
  }

  const unsupported = new Set<string>()
  if (!all.canRename) unsupported.add('rename')
  if (!all.canMove) unsupported.add('cut')
  if (!all.canCopy) unsupported.add('copy')
  if (!all.canDelete) unsupported.add('delete-permanent')
  if (!all.canTrash) unsupported.add('move-trash')

  return actions.filter((action) => !unsupported.has(action.id))
}

const filterUnsupportedCloudActions = (actions: ContextAction[]): ContextAction[] => {
  const unsupported = new Set([
    'move-trash',
    'open-console',
    'new-file',
    'open-with',
    'rename-advanced',
    'compress',
    'extract',
    'check-duplicates',
  ])
  const filtered = actions
    .filter((action) => !unsupported.has(action.id))
    .map((action) => ({
      ...action,
      ...(action.children
        ? { children: filterUnsupportedCloudActions(action.children) }
        : {}),
    }))
    .filter((action) => !action.children || action.children.length > 0)

  const cleaned: ContextAction[] = []
  for (const action of filtered) {
    const isDivider = action.id.startsWith('divider')
    const prevIsDivider = cleaned.length > 0 && cleaned[cleaned.length - 1].id.startsWith('divider')
    if (isDivider && (cleaned.length === 0 || prevIsDivider)) {
      continue
    }
    cleaned.push(action)
  }
  while (cleaned.length > 0 && cleaned[cleaned.length - 1].id.startsWith('divider')) {
    cleaned.pop()
  }
  return cleaned
}

const commandForContextAction = (actionId: string): ShortcutCommandId | null => {
  if (actionId === 'cut') return 'cut'
  if (actionId === 'copy') return 'copy'
  if (actionId === 'paste') return 'paste'
  if (actionId === 'rename') return 'rename'
  if (actionId === 'properties') return 'properties'
  if (actionId === 'move-trash') return 'delete_to_wastebasket'
  if (actionId === 'delete-permanent') return 'delete_permanently'
  if (actionId === 'open-console') return 'open_console'
  return null
}

export const useExplorerContextMenuOps = (deps: Deps) => {
  const applyContextMenuShortcuts = (actions: ContextAction[]): ContextAction[] => {
    return actions.map((action) => {
      const commandId = commandForContextAction(action.id)
      const nextShortcut = commandId
        ? shortcutFor(deps.shortcutBindings(), commandId)?.accelerator ?? action.shortcut
        : action.shortcut
      const nextChildren = action.children ? applyContextMenuShortcuts(action.children) : undefined
      return {
        ...action,
        ...(nextShortcut ? { shortcut: nextShortcut } : {}),
        ...(nextChildren ? { children: nextChildren } : {}),
      }
    })
  }

  const clearSelection = () => {
    deps.setSelection(new Set(), null, null)
  }

  const selectedPathsForEntry = (entry: Entry): string[] => {
    const selected = deps.getSelectedSet()
    return selected.has(entry.path) ? Array.from(selected) : [entry.path]
  }

  const loadAndOpenContextMenu = async (entry: Entry, event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    try {
      const selectionPaths = selectedPathsForEntry(entry)
      const selectionCount = selectionPaths.length
      const selectedPathSet = new Set(selectionPaths)
      const selectionEntries =
        selectionCount > 1
          ? deps.getFilteredEntries().filter((candidate) => selectedPathSet.has(candidate.path))
          : [entry]

      let actions = await fetchContextMenuActions<ContextAction[]>({
        count: selectionCount,
        kind: entry.kind,
        starred: Boolean(entry.starred),
        view: deps.currentView(),
        clipboardHasItems: deps.getClipboardPathCount() > 0,
        selectionPaths,
      })
      actions = actions.filter((action) => action.id !== 'new-folder')
      while (actions.length > 0 && actions[0].id.startsWith('divider')) {
        actions.shift()
      }
      if (deps.currentView() === 'network') {
        const networkActions = await buildNetworkEntryContextActions(entry.path, selectionCount)
        if (networkActions) {
          actions = networkActions
        }
      }
      if (
        deps.isSearchSessionEnabled() &&
        selectionCount === 1 &&
        !actions.some((action) => action.id === 'open-location')
      ) {
        actions.splice(1, 0, { id: 'open-location', label: 'Open item location' })
      }
      actions = filterByCapabilities(actions, selectionEntries)
      if (selectionPaths.every(isCloudPath)) {
        actions = filterUnsupportedCloudActions(actions)
      }
      actions = applyContextMenuShortcuts(actions)
      if (actions.length > 0) {
        deps.openContextMenu(entry, actions, event.clientX, event.clientY)
      }
    } catch (err) {
      console.error('Failed to load context menu actions', err)
    }
  }

  const handleRowContextMenu = (entry: Entry, event: MouseEvent) => {
    deps.onBeforeRowContextMenu?.()
    const idx = deps.getFilteredEntries().findIndex((candidate) => candidate.path === entry.path)
    ensureSelectionBeforeMenu(deps.getSelectedSet(), entry.path, idx, (paths, anchor, caret) => {
      deps.setSelection(paths, anchor, caret)
    })
    void loadAndOpenContextMenu(entry, event)
  }

  const handleBlankContextMenu = (event: MouseEvent) => {
    event.preventDefault()
    event.stopPropagation()
    if (deps.currentView() === 'network') {
      clearSelection()
      deps.openBlankContextMenu(networkBlankContextActions(), event.clientX, event.clientY)
      return
    }
    if (deps.currentView() !== 'dir') {
      clearSelection()
      deps.closeBlankContextMenu()
      return
    }

    const openConsoleShortcut =
      shortcutFor(deps.shortcutBindings(), 'open_console')?.accelerator ?? 'Ctrl+T'
    const pasteShortcut = shortcutFor(deps.shortcutBindings(), 'paste')?.accelerator ?? 'Ctrl+V'
    const isCloudDir = isCloudPath(deps.getCurrentPath())
    const actions: ContextAction[] = [
      { id: 'new-folder', label: 'New Folder…' },
    ]
    if (!isCloudDir) {
      actions.unshift({ id: 'new-file', label: 'New File…' })
      actions.push({ id: 'open-console', label: 'Open in console', shortcut: openConsoleShortcut })
    }
    if (deps.getClipboardPathCount() > 0) {
      actions.push({ id: 'paste', label: 'Paste', shortcut: pasteShortcut })
    }
    clearSelection()
    deps.openBlankContextMenu(actions, event.clientX, event.clientY)
  }

  const handleBlankContextAction = async (id: string) => {
    if (id === 'refresh-network') {
      deps.closeBlankContextMenu()
      if (deps.currentView() === 'network') {
        await deps.loadNetwork(false, { resetScroll: false, forceRefresh: true })
      }
      return
    }
    if (deps.currentView() !== 'dir') return
    deps.closeBlankContextMenu()

    if (id === 'new-folder') {
      deps.openNewFolderModal()
      return
    }
    if (id === 'new-file') {
      deps.openNewFileModal()
      return
    }
    if (id === 'open-console') {
      try {
        await openConsole(deps.getCurrentPath())
      } catch (err) {
        deps.showToast(`Open console failed: ${getErrorMessage(err)}`)
      }
      return
    }
    if (id === 'paste') {
      await deps.pasteIntoCurrent()
    }
  }

  const handleContextSelect = async (id: string) => {
    const entry = deps.getContextMenuEntry()
    deps.closeContextMenu()

    if (entry && id === 'copy-network-address') {
      const selectedPaths = selectedPathsForEntry(entry)
      const uriFlags = await Promise.all(selectedPaths.map((path) => isMountUri(path)))
      const payload = selectedPaths.filter((_, idx) => uriFlags[idx]).join('\n')
      const result = await copyTextToSystemClipboard(payload || entry.path)
      if (result.ok) {
        deps.showToast(selectedPaths.length > 1 ? 'Server addresses copied' : 'Server address copied', 1500)
      } else {
        deps.showToast(`Copy failed: ${result.error}`)
      }
      return
    }

    if (entry && id === 'open-network-target') {
      await deps.openPartition(entry.path)
      return
    }

    if (entry && id === 'disconnect-network') {
      try {
        await ejectDrive(entry.path)
        await deps.loadPartitions({ forceNetworkRefresh: true })
        if (deps.currentView() === 'network') {
          await deps.loadNetwork(false, { resetScroll: false, forceRefresh: true })
        }
        deps.showToast('Disconnected')
      } catch (err) {
        deps.showToast(`Disconnect failed: ${getErrorMessage(err)}`)
      }
      return
    }

    if (id === 'new-folder') {
      if (deps.currentView() !== 'dir') {
        deps.showToast('Cannot create folder here')
        return
      }
      deps.openNewFolderModal()
      return
    }

    if (id === 'new-file') {
      if (deps.currentView() !== 'dir') {
        deps.showToast('Cannot create file here')
        return
      }
      deps.openNewFileModal()
      return
    }

    if (id === 'rename-advanced') {
      const selectedPaths = (() => {
        const selected = deps.getSelectedSet()
        return selected.size > 0 ? Array.from(selected) : entry ? [entry.path] : []
      })()
      const entries =
        selectedPaths.length > 0
          ? (() => {
              const selectedPathSet = new Set(selectedPaths)
              return deps.getFilteredEntries().filter((candidate) => selectedPathSet.has(candidate.path))
            })()
          : entry
            ? [entry]
            : []
      if (entries.length > 1) {
        deps.openAdvancedRename(entries)
      } else if (entry) {
        deps.startRename(entry)
      }
      return
    }

    await deps.contextActions(id, entry)
  }

  return {
    loadAndOpenContextMenu,
    handleRowContextMenu,
    handleBlankContextMenu,
    handleBlankContextAction,
    handleContextSelect,
  }
}
