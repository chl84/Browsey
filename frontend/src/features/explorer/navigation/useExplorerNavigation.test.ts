import { get, writable } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { useExplorerNavigation } from './useExplorerNavigation'
import type { Entry } from '../model/types'

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
    filteredEntries: writable<Entry[]>([]),
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

  it('opens the exact connected SFTP folder and reports a non-fatal history warning', async () => {
    const deps = createDeps('Network')
    const nav = useExplorerNavigation(deps)
    isMountUriMock.mockResolvedValue(true)
    const mountedPath = '/run/user/1000/gvfs/sftp:host=server,user=alice/Photos'
    connectNetworkUriMock.mockResolvedValueOnce({ kind: 'mountable', mountedPath, warning: 'Connected, but the address could not be saved.' })
    await nav.openPartition('sftp://alice@server/Photos')
    expect(deps.loadRaw).toHaveBeenCalledWith(mountedPath, {})
    expect(deps.showToast).toHaveBeenCalledWith('Connected, but the address could not be saved.')
  })

  it('never navigates to an existing server after another server connection fails', async () => {
    const deps = createDeps('/run/user/1000/gvfs/sftp:host=server-a,user=alice')
    const nav = useExplorerNavigation(deps)
    isMountUriMock.mockResolvedValue(true)
    connectNetworkUriMock.mockRejectedValueOnce(new Error('Authentication failed'))
    await nav.openPartition('sftp://alice@server-b/')
    expect(deps.loadRaw).not.toHaveBeenCalled()
    expect(deps.showToast).toHaveBeenCalledWith('Connect failed: Authentication failed')
  })

  it('does not open a late cloud file after the user chooses Recent', async () => {
    const deps = createDeps('/demo')
    const nav = useExplorerNavigation(deps)
    let finish!: (entry: { path: string; kind: string }) => void
    statCloudEntryMock.mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
    const pending = nav.goToPath('rclone://demo/report.txt')
    await nav.loadRecent()
    finish({ path: 'rclone://demo/report.txt', kind: 'file' })
    await pending
    expect(deps.open).not.toHaveBeenCalled()
    expect(deps.setPathInput).not.toHaveBeenCalled()
    expect(deps.loadRaw).not.toHaveBeenCalled()
  })

  it('does not open a late local path after newer navigation', async () => {
    const deps = createDeps('/demo')
    const nav = useExplorerNavigation(deps)
    let finish!: (kind: string) => void
    entryKindMock.mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
    const pending = nav.goToPath('/demo/report.txt')
    await vi.waitFor(() => expect(entryKindMock).toHaveBeenCalled())
    await nav.loadNetwork()
    finish('file')
    await pending
    expect(deps.open).not.toHaveBeenCalled()
    expect(deps.setPathInput).not.toHaveBeenCalled()
  })

  it('does not navigate to a phone when its connection completes after choosing Starred', async () => {
    const deps = createDeps('/demo')
    const nav = useExplorerNavigation(deps)
    let finish!: (result: { kind: string; mountedPath: string }) => void
    connectNetworkUriMock.mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
    const pending = nav.openPartition('mtp://DemoPhone/')
    await nav.loadStarred()
    finish({ kind: 'mountable', mountedPath: '/demo/phone' })
    await pending
    expect(deps.loadRaw).not.toHaveBeenCalled()
  })

  it('reuses a pending phone connection when the user returns to it after navigating away', async () => {
    const deps = createDeps('/demo')
    const nav = useExplorerNavigation(deps)
    let finish!: (result: { kind: string; mountedPath: string }) => void
    connectNetworkUriMock.mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
    const pending = nav.openPartition('mtp://DemoPhone/')
    await nav.loadStarred()
    await nav.openPartition('mtp://DemoPhone/')
    expect(connectNetworkUriMock).toHaveBeenCalledOnce()
    finish({ kind: 'mountable', mountedPath: '/demo/phone' })
    await pending
    await vi.waitFor(() => expect(deps.loadRaw).toHaveBeenCalledWith('/demo/phone', {}))
    expect(deps.loadRaw).toHaveBeenCalledOnce()
  })

  it('does not show an obsolete connection error after the user chooses another place', async () => {
    const deps = createDeps('/demo')
    const nav = useExplorerNavigation(deps)
    let fail!: (error: Error) => void
    connectNetworkUriMock.mockReturnValueOnce(new Promise((_, reject) => { fail = reject }))
    const pending = nav.openPartition('mtp://DemoPhone/')
    await nav.loadTrash()
    fail(new Error('Phone disconnected'))
    await pending
    expect(deps.showToast).not.toHaveBeenCalled()
    expect(deps.loadRaw).not.toHaveBeenCalled()
  })

  it.each([true, false])('does not fall back to an obsolete cloud folder (lookup rejects=%s)', async rejects => {
    const deps = createDeps('/demo')
    const nav = useExplorerNavigation(deps)
    let finish!: () => void
    statCloudEntryMock.mockReturnValueOnce(new Promise((resolve, reject) => {
      finish = () => rejects ? reject(new Error('Not found')) : resolve(null)
    }))
    const pending = nav.goToPath('rclone://demo/old-folder')
    await nav.loadDir('/demo/new-folder')
    deps.loadRaw.mockClear()
    finish()
    await pending
    expect(deps.loadRaw).not.toHaveBeenCalled()
  })

  it('flushes only the newest queued partition target once loading finishes', async () => {
    const deps = createDeps('/demo')
    const nav = useExplorerNavigation(deps)
    deps.loading.set(true)
    await nav.loadDirIfIdle('/demo/old-phone')
    await nav.loadDirIfIdle('/demo/new-phone')
    deps.loading.set(false)
    nav.flushPendingNavigation()
    nav.flushPendingNavigation()
    expect(deps.loadRaw).toHaveBeenCalledOnce()
    expect(deps.loadRaw).toHaveBeenCalledWith('/demo/new-phone', {})
  })

  it.each(['local', 'cloud', 'cloud-unknown'])('queues an entered %s directory while another listing is busy', async kind => {
    const deps = createDeps('/demo')
    const nav = useExplorerNavigation(deps)
    const path = kind === 'local' ? '/demo/next' : 'rclone://demo/next'
    if (kind === 'local') entryKindMock.mockResolvedValueOnce('dir')
    else statCloudEntryMock.mockResolvedValueOnce(kind === 'cloud' ? { path, kind: 'dir' } : null)
    deps.loading.set(true)
    await nav.goToPath(path)
    expect(deps.loadRaw).not.toHaveBeenCalled()
    deps.loading.set(false)
    nav.flushPendingNavigation()
    expect(deps.loadRaw).toHaveBeenCalledExactlyOnceWith(path, {})
  })

  it.each(['loadRecent', 'loadStarred', 'loadNetwork', 'loadTrash', 'goBack', 'goForward'] as const)(
    '%s cancels a queued partition navigation', async action => {
      const deps = createDeps('/demo')
      const nav = useExplorerNavigation(deps)
      deps.loading.set(true)
      await nav.loadDirIfIdle('/demo/phone')
      await nav[action]()
      deps.loading.set(false)
      nav.flushPendingNavigation()
      expect(deps.loadRaw).not.toHaveBeenCalled()
    },
  )

  it('keeps a new selection when an older folder request finishes late', async () => {
    const deps = createDeps('/demo')
    const nav = useExplorerNavigation(deps)
    let finish!: () => void
    const entry: Entry = { path: '/new/report.txt', name: 'report.txt', kind: 'file', iconId: 0 }
    deps.loadRaw.mockImplementationOnce(() => new Promise<void>(resolve => { finish = resolve }))
    deps.loadRaw.mockImplementationOnce(async () => {
      deps.current.set('/new')
      deps.filteredEntries.set([entry])
    })
    const earlier = nav.loadDir('/old')
    await nav.loadDir('/new')
    deps.selected.set(new Set([entry.path]))
    deps.anchorIndex.set(0)
    deps.caretIndex.set(0)
    finish()
    await earlier
    expect(get(deps.selected)).toEqual(new Set([entry.path]))
    expect(get(deps.caretIndex)).toBe(0)
  })
})
