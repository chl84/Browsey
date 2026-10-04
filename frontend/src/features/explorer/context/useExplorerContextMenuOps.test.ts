import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ContextAction } from './createContextMenus'
import { filterByCapabilities, useExplorerContextMenuOps } from './useExplorerContextMenuOps'
import type { Entry } from '../model/types'

const { forgetNetworkConnection, ejectDrive } = vi.hoisted(() => ({ forgetNetworkConnection: vi.fn(), ejectDrive: vi.fn() }))
vi.mock('@/features/network', () => ({ forgetNetworkConnection }))
vi.mock('../services/drives.service', () => ({ ejectDrive }))

const entryWithCaps = (
  path: string,
  caps: NonNullable<Entry['capabilities']>,
): Entry => ({
  path,
  name: 'item',
  kind: 'file',
  iconId: 0,
  capabilities: caps,
})

const action = (id: string): ContextAction => ({ id, label: id })

describe('filterByCapabilities', () => {
  it('removes actions disabled by entry capabilities', () => {
    const actions: ContextAction[] = [
      action('copy'),
      action('cut'),
      action('rename'),
      action('move-trash'),
      action('delete-permanent'),
      action('properties'),
    ]
    const entry = entryWithCaps('rclone://remote/a.txt', {
      canList: true,
      canMkdir: true,
      canDelete: true,
      canRename: false,
      canMove: false,
      canCopy: true,
      canTrash: false,
      canUndo: false,
      canPermissions: false,
    })

    const filtered = filterByCapabilities(actions, [entry]).map((a) => a.id)

    expect(filtered).toEqual(['copy', 'delete-permanent', 'properties'])
  })

  it('keeps actions unchanged when capabilities are missing', () => {
    const actions: ContextAction[] = [action('copy'), action('rename')]
    const entry: Entry = { path: '/tmp/a', name: 'a', kind: 'file', iconId: 0 }

    expect(filterByCapabilities(actions, [entry]).map((a) => a.id)).toEqual(['copy', 'rename'])
  })

  it('recursively hides unsupported cloud additions while retaining allowed siblings', () => {
    const entry = entryWithCaps('rclone://remote/report.txt', {
      canList: true, canMkdir: true, canDelete: true, canRename: true,
      canMove: true, canCopy: true, canTrash: false, canUndo: false, canPermissions: false,
      canOpenWith: true, canArchive: false, canAdvancedRename: true, canExternalCopy: false,
    })
    const actions: ContextAction[] = [action('move-trash'), action('open-with'), action('rename-advanced'),
      action('cloud-export'), { id: 'archive', label: 'Archive', children: [action('compress'), action('extract')] },
      { id: 'other', label: 'Other', children: [action('copy'), action('compress')] }]
    const filtered = filterByCapabilities(actions, [entry])
    expect(filtered.map((item) => item.id)).toEqual(['open-with', 'rename-advanced', 'other'])
    expect(filtered[2].children?.map((item) => item.id)).toEqual(['copy'])
  })
})

describe('forget saved network connection', () => {
  const path = 'sftp://alice@server/'
  const createDeps = (): Parameters<typeof useExplorerContextMenuOps>[0] => ({
    currentView: () => 'network', isSearchSessionEnabled: () => false, shortcutBindings: () => [],
    getCurrentPath: () => 'Network', getContextMenuEntry: () => ({ name: 'Server', path, kind: 'dir', iconId: 10, network: true }),
    getClipboardPathCount: () => 0, getSelectedSet: () => new Set(), getFilteredEntries: () => [],
    setSelection: vi.fn(), openContextMenu: vi.fn(), closeContextMenu: vi.fn(),
    openBlankContextMenu: vi.fn(), closeBlankContextMenu: vi.fn(), loadNetwork: vi.fn().mockResolvedValue(undefined),
    openPartition: vi.fn(), loadPartitions: vi.fn(), pasteIntoCurrent: vi.fn(), openNewFolderModal: vi.fn(),
    openNewFileModal: vi.fn(), openAdvancedRename: vi.fn(), startRename: vi.fn(), contextActions: vi.fn(), showToast: vi.fn(),
  })

  beforeEach(() => {
    vi.resetAllMocks()
    forgetNetworkConnection.mockResolvedValue(undefined)
  })

  it('removes only the saved address and refreshes without disconnecting or opening it', async () => {
    const deps = createDeps()
    await useExplorerContextMenuOps(deps).handleContextSelect('forget-network-connection')
    expect(forgetNetworkConnection).toHaveBeenCalledWith(path)
    expect(deps.loadNetwork).toHaveBeenCalledWith(false, { resetScroll: false })
    expect(ejectDrive).not.toHaveBeenCalled()
    expect(deps.openPartition).not.toHaveBeenCalled()
    expect(deps.contextActions).not.toHaveBeenCalled()
  })

  it('reports database failure without pretending the address was forgotten', async () => {
    const deps = createDeps()
    forgetNetworkConnection.mockRejectedValueOnce(new Error('Database read-only'))
    await useExplorerContextMenuOps(deps).handleContextSelect('forget-network-connection')
    expect(deps.showToast).toHaveBeenCalledWith('Could not forget connection: Database read-only')
    expect(deps.loadNetwork).not.toHaveBeenCalled()
    expect(ejectDrive).not.toHaveBeenCalled()
  })
})
