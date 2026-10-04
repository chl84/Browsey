import type { Entry } from '../model/types'
import type { CurrentView } from '../context/createContextActions'
import type { ExplorerFileActions } from '../file-ops/createExplorerFileActions'
import type { useExplorerSearchSession } from '../navigation/useExplorerSearchSession'
import { createGlobalShortcuts } from './createGlobalShortcuts'

type GlobalArgs = Parameters<typeof createGlobalShortcuts>[0]
type SearchSession = ReturnType<typeof useExplorerSearchSession>
type Deps = {
  isBookmarkModalOpen: GlobalArgs['isBookmarkModalOpen']
  isShortcut: GlobalArgs['isShortcut']
  currentView: () => CurrentView
  getMode: () => 'address' | 'filter'
  isInputFocused: () => boolean
  getPathInput: () => string
  setPathInput: (value: string) => void
  getCurrentPath: () => string
  isSearchSessionEnabled: () => boolean
  setSearchMode: GlobalArgs['setSearchMode']
  focusPath: () => void
  transitionToFilterMode: SearchSession['transitionToFilterMode']
  transitionToAddressMode: SearchSession['transitionToAddressMode']
  enterAddressMode: SearchSession['enterAddressMode']
  getSelectedPaths: () => string[]
  getEntries: () => Entry[]
  openBookmarkModal: GlobalArgs['openBookmarkModal']
  goBack: GlobalArgs['goBack']
  goForward: GlobalArgs['goForward']
  // Lazy access keeps the page's composition order independent of handlers.
  fileActions: () => ExplorerFileActions
  reloadCurrent: () => Promise<void>
  toggleViewMode: () => void
  toggleShowHidden: () => void
  undo: NonNullable<GlobalArgs['onUndo']>
  redo: NonNullable<GlobalArgs['onRedo']>
  toggleSettings: () => void
}

export const createExplorerShortcuts = (deps: Deps) => createGlobalShortcuts({
  isBookmarkModalOpen: deps.isBookmarkModalOpen,
  isShortcut: deps.isShortcut,
  searchMode: deps.isSearchSessionEnabled,
  setSearchMode: deps.setSearchMode,
  focusPath: deps.focusPath,
  onToggleHidden: () => Promise.resolve(deps.toggleShowHidden()),
  onTypeChar: async char => {
    const canSearch = deps.currentView() === 'dir'
    if (deps.isInputFocused() && deps.getMode() === 'address' && canSearch) return false
    if (deps.isSearchSessionEnabled() && canSearch) {
      deps.setPathInput(`${deps.getPathInput()}${char}`)
      deps.focusPath()
      return true
    }
    if (deps.getMode() !== 'filter') await deps.transitionToFilterMode('')
    deps.setPathInput(`${deps.getPathInput()}${char}`)
    deps.focusPath()
    return true
  },
  onRemoveChar: async () => {
    const input = deps.getPathInput()
    if (deps.isSearchSessionEnabled()) {
      if (input.length === 0) await deps.enterAddressMode()
      else deps.setPathInput(input.slice(0, -1))
      deps.focusPath()
      return true
    }
    if (deps.getMode() === 'filter') {
      if (input.length <= 1) await deps.transitionToAddressMode({ path: deps.getCurrentPath(), blur: true })
      else {
        deps.setPathInput(input.slice(0, -1))
        deps.focusPath()
      }
      return true
    }
    return false
  },
  getSelectedPaths: deps.getSelectedPaths,
  findEntryByPath: path => deps.getEntries().find(entry => entry.path === path) ?? null,
  openBookmarkModal: deps.openBookmarkModal,
  goBack: deps.goBack,
  goForward: deps.goForward,
  onCopy: () => deps.fileActions().copy(),
  onCut: () => deps.fileActions().cut(),
  onPaste: () => deps.fileActions().paste(),
  onRename: () => deps.fileActions().rename(),
  onDelete: permanent => deps.fileActions().delete(permanent),
  onDeletePermanentFast: () => deps.fileActions().deletePermanentFast(),
  onProperties: () => deps.fileActions().properties(),
  onOpenConsole: () => deps.fileActions().openConsole(),
  onSelectAll: () => deps.fileActions().selectAll(),
  onRefresh: async () => { await deps.reloadCurrent(); return true },
  onToggleView: deps.toggleViewMode,
  onUndo: deps.undo,
  onRedo: deps.redo,
  onToggleSettings: () => { deps.toggleSettings(); return true },
})
