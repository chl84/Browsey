import { mount, tick, unmount } from 'svelte'
import { get } from 'svelte/store'
import { expect, it, vi } from 'vitest'
import PropertiesModal from '../components/PropertiesModal.svelte'
import FileRow from '../components/FileRow.svelte'
import type { Entry } from '../model/types'
import { useExplorerInputHandlers } from './useExplorerInputHandlers'
import { createSelectionBox } from '../selection/createSelectionBox'
import { createListState } from '../state/list.store'

vi.mock('@/shared/lib/tauri', () => ({ invoke: vi.fn(), convertFileSrc: vi.fn() }))

const makeDeps = (onClose: () => void): Parameters<typeof useExplorerInputHandlers>[0] => ({
  getViewMode: () => 'list',
  getMode: () => 'address',
  setPathInput: vi.fn(),
  getPathInput: () => '',
  isInputFocused: () => false,
  getCurrentPath: () => '/',
  isSearchSessionEnabled: () => false,
  getRowsEl: () => null,
  getHeaderEl: () => null,
  getFilteredEntries: () => [],
  getSelected: () => new Set(),
  setSelected: vi.fn(),
  getAnchorIndex: () => null,
  setAnchorIndex: vi.fn(),
  getCaretIndex: () => null,
  setCaretIndex: vi.fn(),
  getRowHeight: () => 24,
  getDoubleClickMs: () => 300,
  isEditableTarget: () => false,
  hasAppShortcut: () => false,
  handleGlobalKeydown: vi.fn(),
  transitionToAddressMode: vi.fn(async () => {}),
  blurPathInput: vi.fn(),
  ensureGridVisible: vi.fn(),
  getRowsKeydownHandler: () => null,
  getRowSelectionHandler: () => null,
  selectionBox: createSelectionBox(),
  getGridCols: () => 1,
  getGridCardWidth: () => 100,
  getGridRowHeight: () => 100,
  getGridGap: () => 0,
  handleRowsScroll: vi.fn(),
  handleGridScroll: vi.fn(),
  handleRowsClick: vi.fn(),
  currentView: () => 'dir',
  loadDir: vi.fn(async () => {}),
  openPartition: vi.fn(async () => {}),
  canExtractPaths: vi.fn(async () => false),
  extractEntries: vi.fn(async () => {}),
  open: vi.fn(),
  isBlockingModalOpen: () => true,
  isDeleteModalOpen: () => false,
  closeDeleteModal: vi.fn(),
  isRenameModalOpen: () => false,
  closeRenameModal: vi.fn(),
  isAdvancedRenameModalOpen: () => false,
  closeAdvancedRenameModal: vi.fn(),
  isOpenWithModalOpen: () => false,
  closeOpenWithModal: vi.fn(),
  isPropertiesModalOpen: () => true,
  closePropertiesModal: onClose,
  isCompressModalOpen: () => false,
  closeCompressModal: vi.fn(),
  isCheckDuplicatesModalOpen: () => false,
  closeCheckDuplicatesModal: vi.fn(),
  isNewFolderModalOpen: () => false,
  closeNewFolderModal: vi.fn(),
  isNewFileModalOpen: () => false,
  closeNewFileModal: vi.fn(),
  isBookmarkModalOpen: () => false,
  closeBookmarkModal: vi.fn(),
  isContextMenuOpen: () => false,
  closeContextMenu: vi.fn(),
  isBlankMenuOpen: () => false,
  closeBlankContextMenu: vi.fn(),
})

const listingHarness = (view: 'list' | 'grid' = 'list') => {
  const rows = document.createElement('div')
  rows.tabIndex = 0
  document.body.append(rows)
  const old: Entry = { name: 'old.txt', path: '/owned/old.txt', kind: 'file', iconId: 0 }
  const created: Entry = { name: 'New folder', path: '/owned/New folder', kind: 'dir', iconId: 4 }
  const list = [old, created]
  const state = createListState()
  state.selected.set(new Set([created.path]))
  const deps = makeDeps(vi.fn())
  Object.assign(deps, {
    getViewMode: () => view,
    getRowsEl: () => rows,
    getFilteredEntries: () => list,
    getSelected: () => get(state.selected),
    getRowsKeydownHandler: () => state.handleRowsKeydown(list),
    isBlockingModalOpen: () => false,
    isEditableTarget: (target: EventTarget | null) => target instanceof HTMLInputElement,
  })
  const handlers = useExplorerInputHandlers(deps)
  rows.addEventListener('keydown', handlers.handleRowsKeydownCombined)
  document.addEventListener('keydown', handlers.handleDocumentKeydown, true)
  const enter = () => {
    const event = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true })
    document.activeElement!.dispatchEvent(event)
    return event
  }
  const cleanup = () => {
    document.removeEventListener('keydown', handlers.handleDocumentKeydown, true)
    rows.remove()
  }
  return { rows, list, old, created, state, deps, handlers, enter, cleanup }
}

it.each(['list', 'grid'] as const)('Enter on the %s collection opens the newly selected folder through normal navigation', async view => {
  const h = listingHarness(view)
  try {
    h.rows.focus()
    expect(h.enter().defaultPrevented).toBe(true)
    await Promise.resolve()
    expect(h.deps.loadDir).toHaveBeenCalledExactlyOnceWith(h.created.path)
    expect(h.deps.open).not.toHaveBeenCalled()
  } finally { h.cleanup() }
})

it.each([{ paths: [] }, { paths: ['/owned/missing'] }])('Enter on the list never opens an unrelated item for an empty/stale selection ($paths)', ({ paths }) => {
  const h = listingHarness()
  try {
    h.state.selected.set(new Set(paths))
    h.deps.getCaretIndex = () => 0
    h.rows.focus()
    h.enter()
    expect(h.deps.loadDir).not.toHaveBeenCalled()
    expect(h.deps.open).not.toHaveBeenCalled()
  } finally { h.cleanup() }
})

it('Enter on the list ignores a stale caret and opens the selected file through normal file opening', () => {
  const h = listingHarness()
  try {
    h.state.selected.set(new Set([h.old.path]))
    h.deps.getCaretIndex = () => 1
    h.rows.focus()
    h.enter()
    expect(h.deps.open).toHaveBeenCalledExactlyOnceWith(h.old)
    expect(h.deps.loadDir).not.toHaveBeenCalled()
  } finally { h.cleanup() }
})

it('a focused row still owns Enter and opens exactly once, independently of collection selection', async () => {
  const h = listingHarness()
  const component = mount(FileRow, {
    target: h.rows, props: { entry: h.old, onOpen: h.handlers.handleOpenEntry },
  })
  try {
    await tick()
    h.rows.querySelector<HTMLButtonElement>('.row')!.focus()
    h.enter()
    expect(h.deps.open).toHaveBeenCalledExactlyOnceWith(h.old)
    expect(h.deps.loadDir).not.toHaveBeenCalled()
  } finally { await unmount(component); h.cleanup() }
})

it('a modal or an editable child keeps Enter out of collection navigation', () => {
  const h = listingHarness()
  try {
    h.deps.isBlockingModalOpen = () => true
    h.rows.focus()
    h.enter()
    h.deps.isBlockingModalOpen = () => false
    const input = document.createElement('input')
    h.rows.append(input); input.focus()
    h.enter()
    expect(h.deps.loadDir).not.toHaveBeenCalled()
    expect(h.deps.open).not.toHaveBeenCalled()
  } finally { h.cleanup() }
})

it('ArrowUp from outside the list still transfers focus and navigates through the document handler', () => {
  const h = listingHarness()
  const button = document.createElement('button')
  document.body.append(button)
  try {
    h.state.caretIndex.set(1)
    h.state.anchorIndex.set(1)
    button.focus()
    button.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowUp', bubbles: true, cancelable: true }))
    expect(document.activeElement).toBe(h.rows)
    expect(get(h.state.selected)).toEqual(new Set([h.old.path]))
    expect(get(h.state.caretIndex)).toBe(0)
  } finally { button.remove(); h.cleanup() }
})

it('a context gesture breaks the double-click sequence while preserving the next genuine double click', () => {
  const deps = makeDeps(vi.fn()), select = vi.fn()
  deps.getRowSelectionHandler = () => select
  const handlers = useExplorerInputHandlers(deps)
  const entry = { name: 'sample.txt', path: '/owned/sample.txt', kind: 'file' as const, iconId: 0 }
  const click = (time: number) => {
    const event = new MouseEvent('click', { button: 0, detail: 1 })
    Object.defineProperty(event, 'timeStamp', { value: time })
    handlers.handleRowClickWithOpen(entry, 0, event)
  }
  click(1000)
  handlers.handleRowsMouseDown(new MouseEvent('mousedown', { button: 2 }))
  click(1100)
  handlers.handleRowsMouseDown(new MouseEvent('mousedown', { button: 0 }))
  click(1200)
  expect(deps.open).not.toHaveBeenCalled()
  expect(select).toHaveBeenCalledTimes(2)
  handlers.handleRowsMouseDown(new MouseEvent('mousedown', { button: 0 }))
  click(1300)
  expect(deps.open).toHaveBeenCalledOnce()
})

it('lets the searchable Properties dropdown consume Escape before the document capture handler', async () => {
  const onClose = vi.fn()
  const onSetOwnership = vi.fn()
  const { handleDocumentKeydown } = useExplorerInputHandlers(makeDeps(onClose))
  document.addEventListener('keydown', handleDocumentKeydown, true)
  const component = mount(PropertiesModal, {
    target: document.body,
    props: {
      open: true,
      entry: { name: 'mock.txt', path: '/mock/mock.txt', kind: 'file', iconId: 0 },
      ownershipUsers: ['alice', 'bob'],
      ownershipGroups: ['users'],
      permissions: {
        accessSupported: true, ownershipSupported: true, ownerName: 'alice', groupName: 'users',
        owner: null, group: null, other: null,
      },
      onClose,
      onSetOwnership,
    },
  })
  try {
    await tick()
    const ownership = Array.from(document.querySelectorAll<HTMLButtonElement>('.tabs button'))
      .find((button) => button.textContent === 'Ownership')!
    ownership.click()
    await tick()
    const button = document.querySelector<HTMLButtonElement>('.combo-btn')!
    button.click()
    await tick()
    const search = document.querySelector<HTMLInputElement>('.combo-search')!
    expect(document.activeElement).toBe(search)
    search.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await tick()
    expect(onClose).not.toHaveBeenCalled()
    expect(button.getAttribute('aria-expanded')).toBe('false')
    expect(document.activeElement).toBe(button)
    expect(onSetOwnership).not.toHaveBeenCalled()
    button.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }))
    await tick()
    expect(onClose).toHaveBeenCalledOnce()
  } finally {
    document.removeEventListener('keydown', handleDocumentKeydown, true)
    await unmount(component)
    document.body.innerHTML = ''
  }
})
