import { writable } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useExplorerNavigation } from './useExplorerNavigation'

const entryKindMock = vi.fn()
const isMountUriMock = vi.fn()
const connectNetworkUriMock = vi.fn()
const statCloudEntryMock = vi.fn()

vi.mock('../services/files.service', () => ({
  entryKind: (...args: unknown[]) => entryKindMock(...args),
}))

vi.mock('@/features/network', () => ({
  isMountUri: (...args: unknown[]) => isMountUriMock(...args),
  connectNetworkUri: (...args: unknown[]) => connectNetworkUriMock(...args),
  statCloudEntry: (...args: unknown[]) => statCloudEntryMock(...args),
}))

const createDeps = (currentPath = '~') => {
  const deps = {
    current: writable(currentPath),
    loading: writable(false),
    filteredEntries: writable([]),
    selected: writable(new Set<string>()),
    anchorIndex: writable<number | null>(null),
    caretIndex: writable<number | null>(null),
    rowHeight: writable(32),
    gridTotalHeight: writable(0),
    getViewMode: () => 'list' as const,
    getRowsEl: () => null,
    getHeaderEl: () => null,
    getGridEl: () => null,
    getGridCols: () => 1,
    getGridRowHeight: () => 126,
    getGridGap: () => 6,
    resetScrollPosition: vi.fn(),
    loadRaw: vi.fn(async () => {}),
    loadRecentRaw: vi.fn(async () => {}),
    loadStarredRaw: vi.fn(async () => {}),
    loadNetworkRaw: vi.fn(async () => {}),
    loadTrashRaw: vi.fn(async () => {}),
    goBackRaw: vi.fn(async () => {}),
    goForwardRaw: vi.fn(async () => {}),
    open: vi.fn(),
    loadPartitions: vi.fn(async () => {}),
    showToast: vi.fn(),
    setPathInput: vi.fn(),
  }

  return deps
}

describe('useExplorerNavigation cloud goToPath', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    isMountUriMock.mockResolvedValue(false)
    connectNetworkUriMock.mockResolvedValue({ kind: 'not_uri', normalizedUri: null, mountedPath: null })
  })

  it('opens cloud file path directly when stat reports file', async () => {
    const deps = createDeps('rclone://browsey-gdrive')
    const nav = useExplorerNavigation(deps)
    statCloudEntryMock.mockResolvedValueOnce({
      name: 'Knekklengder-1.pdf',
      path: 'rclone://browsey-gdrive/Knekklengder-1.pdf',
      kind: 'file',
      size: 123,
      modified: null,
      capabilities: {
        canList: true,
        canMkdir: true,
        canDelete: true,
        canRename: true,
        canMove: true,
        canCopy: true,
        canTrash: true,
        canUndo: true,
        canPermissions: false,
      },
    })

    await nav.goToPath('rclone://browsey-gdrive/Knekklengder-1.pdf')

    expect(statCloudEntryMock).toHaveBeenCalledWith('rclone://browsey-gdrive/Knekklengder-1.pdf')
    expect(deps.open).toHaveBeenCalledWith(
      expect.objectContaining({
        kind: 'file',
        path: 'rclone://browsey-gdrive/Knekklengder-1.pdf',
      }),
    )
    expect(deps.loadRaw).not.toHaveBeenCalled()
  })

  it('loads cloud directory when stat reports dir', async () => {
    const deps = createDeps('rclone://browsey-gdrive')
    const nav = useExplorerNavigation(deps)
    statCloudEntryMock.mockResolvedValueOnce({
      name: 'docs',
      path: 'rclone://browsey-gdrive/docs',
      kind: 'dir',
      size: null,
      modified: null,
      capabilities: {
        canList: true,
        canMkdir: true,
        canDelete: true,
        canRename: true,
        canMove: true,
        canCopy: true,
        canTrash: true,
        canUndo: true,
        canPermissions: false,
      },
    })

    await nav.goToPath('rclone://browsey-gdrive/docs')

    expect(deps.loadRaw).toHaveBeenCalledWith('rclone://browsey-gdrive/docs', {})
    expect(deps.open).not.toHaveBeenCalled()
  })

  it('falls back to cloud directory load when stat is missing', async () => {
    const deps = createDeps('rclone://browsey-gdrive')
    const nav = useExplorerNavigation(deps)
    statCloudEntryMock.mockResolvedValueOnce(null)

    await nav.goToPath('rclone://browsey-gdrive/unknown')

    expect(deps.loadRaw).toHaveBeenCalledWith('rclone://browsey-gdrive/unknown', {})
    expect(deps.open).not.toHaveBeenCalled()
  })

  it('mounts a phone before opening the exact returned path', async () => {
    const deps = createDeps('/mock')
    const nav = useExplorerNavigation(deps)
    connectNetworkUriMock.mockResolvedValueOnce({ kind: 'mountable', mountedPath: '/run/user/1000/gvfs/mtp:host=Phone_A' })
    await nav.openPartition('mtp://Phone_A/')
    expect(connectNetworkUriMock).toHaveBeenCalledWith('mtp://Phone_A/')
    expect(deps.loadPartitions).toHaveBeenCalled()
    expect(deps.loadRaw).toHaveBeenCalledWith('/run/user/1000/gvfs/mtp:host=Phone_A', {})
  })

  it('coalesces repeated phone clicks and allows retry after a mount error', async () => {
    const deps = createDeps('/mock')
    const nav = useExplorerNavigation(deps)
    let fail!: (error: Error) => void
    connectNetworkUriMock.mockImplementationOnce(() => new Promise((_, reject) => { fail = reject }))
    const first = nav.openPartition('mtp://Phone_A/')
    await nav.openPartition('mtp://Phone_A/')
    expect(connectNetworkUriMock).toHaveBeenCalledTimes(1)
    fail(new Error('Unlock the phone and allow File transfer (MTP).'))
    await first
    expect(deps.showToast).toHaveBeenCalledWith('Phone connection failed: Unlock the phone and allow File transfer (MTP).')
    expect(deps.loadRaw).not.toHaveBeenCalled()
    connectNetworkUriMock.mockResolvedValueOnce({ kind: 'mountable', mountedPath: '/phone' })
    await nav.openPartition('mtp://Phone_A/')
    expect(connectNetworkUriMock).toHaveBeenCalledTimes(2)
    expect(deps.loadRaw).toHaveBeenCalledWith('/phone', {})
  })
})
