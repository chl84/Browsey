import { mount, tick, unmount } from 'svelte'
import { expect, it, vi } from 'vitest'
import PropertiesModal from '../components/PropertiesModal.svelte'
import { useExplorerInputHandlers } from './useExplorerInputHandlers'
import { createSelectionBox } from '../selection/createSelectionBox'

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
