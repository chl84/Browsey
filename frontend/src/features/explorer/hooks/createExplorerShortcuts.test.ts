import { describe, expect, it, vi } from 'vitest'
import type { Entry } from '../model/types'
import type { CurrentView } from '../context/createContextActions'
import { createExplorerShortcuts } from './createExplorerShortcuts'
import { DEFAULT_SHORTCUTS, matchesShortcut } from '@/features/shortcuts'

type UiState = { mode: 'address' | 'filter'; input: string; focused: boolean; search: boolean; view: CurrentView; bookmarkOpen: boolean }
const folder: Entry = { path: '/mock/folder', name: 'folder', kind: 'dir', iconId: 0 }
const setup = (initial: Partial<UiState> = {}) => {
  const ui: UiState = { mode: 'address', input: '/mock', focused: false, search: false, view: 'dir', bookmarkOpen: false, ...initial }
  const fileActions = {
    open: vi.fn(async () => {}), copy: vi.fn(async () => true), cut: vi.fn(async () => true), paste: vi.fn(async () => true),
    rename: vi.fn(() => true), delete: vi.fn(async (_permanent = false) => true), deletePermanentFast: vi.fn(async () => true),
    properties: vi.fn(() => true), openConsole: vi.fn(async () => true), selectAll: vi.fn(() => true), selectCreated: vi.fn(),
  }
  let selected = [folder.path]
  let entries = [folder]
  const deps = {
    isBookmarkModalOpen: () => ui.bookmarkOpen,
    isShortcut: (event: KeyboardEvent, id: string) => event.key === id,
    currentView: () => ui.view, getMode: () => ui.mode, isInputFocused: () => ui.focused,
    getPathInput: () => ui.input, setPathInput: (value: string) => { ui.input = value },
    getCurrentPath: () => '/mock', isSearchSessionEnabled: () => ui.search,
    setSearchMode: vi.fn(async (value: boolean) => { ui.search = value }), focusPath: vi.fn(),
    transitionToFilterMode: vi.fn(async (path = '') => { ui.mode = 'filter'; ui.input = path }),
    transitionToAddressMode: vi.fn(async (_options?: { path?: string; blur?: boolean }) => { ui.mode = 'address' }),
    enterAddressMode: vi.fn(async () => { ui.mode = 'address'; ui.search = false }),
    getSelectedPaths: () => selected, getEntries: () => entries,
    openBookmarkModal: vi.fn(async (_entry: Entry) => {}), goBack: vi.fn(), goForward: vi.fn(),
    fileActions: () => fileActions, reloadCurrent: vi.fn(async () => {}), toggleViewMode: vi.fn(), toggleShowHidden: vi.fn(),
    undo: vi.fn(async () => true), redo: vi.fn(async () => true), toggleSettings: vi.fn(),
  }
  return {
    ui, deps, fileActions, shortcuts: createExplorerShortcuts(deps),
    select: (paths: string[]) => { selected = paths }, list: (next: Entry[]) => { entries = next },
  }
}
const key = (value: string) => new KeyboardEvent('keydown', { key: value, bubbles: true, cancelable: true })

describe('Explorer shortcut composition', () => {
  it.each([false, true])('prevents F5 browser reload before asynchronous folder refresh finishes (failure: %s)', async failure => {
    const { deps } = setup()
    let finish!: () => void
    const pending = new Promise<void>((resolve, reject) => {
      finish = () => failure ? reject(new Error('folder refresh failed')) : resolve()
    })
    deps.reloadCurrent.mockReturnValueOnce(pending)
    const shortcuts = createExplorerShortcuts({ ...deps,
      isShortcut: (event, id) => matchesShortcut(event, DEFAULT_SHORTCUTS, id),
    })
    const event = key('F5')
    const handling = shortcuts.handleGlobalKeydown(event)
    expect(event.defaultPrevented).toBe(true)
    expect(deps.reloadCurrent).toHaveBeenCalledOnce()
    finish()
    if (failure) await expect(handling).rejects.toThrow('folder refresh failed')
    else await handling
    expect(event.defaultPrevented).toBe(true)
  })

  it('switches typing to filter, appends subsequent characters and leaves focused addresses alone', async () => {
    const { shortcuts, ui, deps } = setup()
    const event = key('a')
    await shortcuts.handleGlobalKeydown(event)
    expect(event.defaultPrevented).toBe(true)
    expect(ui).toMatchObject({ mode: 'filter', input: 'a' })
    await shortcuts.handleGlobalKeydown(key('b'))
    expect(ui.input).toBe('ab')
    expect(deps.transitionToFilterMode).toHaveBeenCalledTimes(1)
    ui.mode = 'address'; ui.focused = true
    const address = key('c')
    await shortcuts.handleGlobalKeydown(address)
    expect(address.defaultPrevented).toBe(false)
    expect(ui.input).toBe('ab')
  })

  it('appends search drafts without entering filter mode', async () => {
    const { shortcuts, ui, deps } = setup({ search: true, input: 'draft' })
    await shortcuts.handleGlobalKeydown(key('a'))
    expect(ui.input).toBe('drafta')
    expect(deps.transitionToFilterMode).not.toHaveBeenCalled()
    expect(deps.focusPath).toHaveBeenCalledOnce()
  })

  it('removes search characters, exits an empty search and never navigates back', async () => {
    const { shortcuts, ui, deps } = setup({ search: true, input: 'a' })
    await shortcuts.handleGlobalKeydown(key('go_back'))
    expect(ui.input).toBe('')
    expect(deps.enterAddressMode).not.toHaveBeenCalled()
    await shortcuts.handleGlobalKeydown(key('go_back'))
    expect(deps.enterAddressMode).toHaveBeenCalledOnce()
    expect(deps.goBack).not.toHaveBeenCalled()
  })

  it('removes filter characters then restores breadcrumbs; address mode still navigates', async () => {
    const { shortcuts, ui, deps } = setup({ mode: 'filter', input: 'ab' })
    await shortcuts.handleGlobalKeydown(key('go_back'))
    expect(ui.input).toBe('a')
    await shortcuts.handleGlobalKeydown(key('go_back'))
    expect(deps.transitionToAddressMode).toHaveBeenCalledWith({ path: '/mock', blur: true })
    expect(deps.goBack).not.toHaveBeenCalled()
    await shortcuts.handleGlobalKeydown(key('go_back'))
    await shortcuts.handleGlobalKeydown(key('go_forward'))
    expect(deps.goBack).toHaveBeenCalledOnce()
    expect(deps.goForward).toHaveBeenCalledOnce()
  })

  it.each([
    ['copy', 'copy'], ['cut', 'cut'], ['paste', 'paste'], ['rename', 'rename'],
    ['delete_permanently', 'deletePermanentFast'], ['properties', 'properties'],
    ['open_console', 'openConsole'], ['select_all', 'selectAll'],
  ] as const)('routes %s to the existing file-action controller', async (command, action) => {
    const { shortcuts, fileActions } = setup()
    const event = key(command)
    await shortcuts.handleGlobalKeydown(event)
    expect(fileActions[action]).toHaveBeenCalledOnce()
    expect(event.defaultPrevented).toBe(true)
  })

  it('routes normal Delete without forcing permanent deletion', async () => {
    const { shortcuts, fileActions } = setup()
    await shortcuts.handleGlobalKeydown(key('delete_to_wastebasket'))
    expect(fileActions.delete).toHaveBeenCalledWith(false)
    expect(fileActions.deletePermanentFast).not.toHaveBeenCalled()
  })

  it('does not intercept editing or shortcuts behind the bookmark modal', async () => {
    const { shortcuts, ui, fileActions, deps } = setup()
    const event = key('copy')
    Object.defineProperty(event, 'target', { value: document.createElement('input') })
    await shortcuts.handleGlobalKeydown(event)
    expect(event.defaultPrevented).toBe(false)
    expect(fileActions.copy).not.toHaveBeenCalled()
    ui.bookmarkOpen = true
    await shortcuts.handleGlobalKeydown(key('copy'))
    await shortcuts.handleGlobalKeydown(key('a'))
    await shortcuts.handleGlobalKeydown(key('open_settings'))
    expect(fileActions.copy).not.toHaveBeenCalled()
    expect(deps.toggleSettings).not.toHaveBeenCalled()
    expect(ui.input).toBe('/mock')
  })

  it('bookmarks one current directory entry, not files or stale selections', async () => {
    const { shortcuts, deps, select, list } = setup()
    await shortcuts.handleGlobalKeydown(key('bookmarks'))
    expect(deps.openBookmarkModal).toHaveBeenCalledWith(folder)
    list([{ ...folder, kind: 'file' }])
    await shortcuts.handleGlobalKeydown(key('bookmarks'))
    select(['/missing'])
    await shortcuts.handleGlobalKeydown(key('bookmarks'))
    select([folder.path, '/another'])
    await shortcuts.handleGlobalKeydown(key('bookmarks'))
    expect(deps.openBookmarkModal).toHaveBeenCalledOnce()
  })

  it('toggles search only when needed, then focuses the existing path input', async () => {
    const { shortcuts, deps } = setup()
    await shortcuts.handleGlobalKeydown(key('search'))
    await shortcuts.handleGlobalKeydown(key('search'))
    expect(deps.setSearchMode).toHaveBeenCalledOnce()
    expect(deps.setSearchMode).toHaveBeenCalledWith(true)
    expect(deps.focusPath).toHaveBeenCalledTimes(2)
  })

  it.each([
    ['refresh', 'reloadCurrent'], ['toggle_view', 'toggleViewMode'], ['toggle_hidden', 'toggleShowHidden'],
    ['undo', 'undo'], ['redo', 'redo'], ['open_settings', 'toggleSettings'],
  ] as const)('preserves %s callbacks', async (command, action) => {
    const { shortcuts, deps } = setup()
    const event = key(command)
    await shortcuts.handleGlobalKeydown(event)
    expect(deps[action]).toHaveBeenCalledOnce()
    expect(event.defaultPrevented).toBe(true)
  })
})
