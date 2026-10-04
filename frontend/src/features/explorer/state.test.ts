import { get } from 'svelte/store'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { Entry, Listing } from './model/types'

const {
  listDirMock,
  listRecentMock,
  listStarredMock,
  listTrashMock,
  watchDirMock,
  listMountsMock,
  listNetworkEntriesMock,
  loadCloudSetupStatusMock,
} = vi.hoisted(() => ({
  listDirMock: vi.fn(),
  listRecentMock: vi.fn(),
  listStarredMock: vi.fn(),
  listTrashMock: vi.fn(),
  watchDirMock: vi.fn(),
  listMountsMock: vi.fn(),
  listNetworkEntriesMock: vi.fn(),
  loadCloudSetupStatusMock: vi.fn(),
}))

vi.mock('./services/listing.service', () => ({
  listDir: listDirMock,
  listRecent: listRecentMock,
  listStarred: listStarredMock,
  listTrash: listTrashMock,
  watchDir: watchDirMock,
  listMounts: listMountsMock,
}))

vi.mock('../network', () => ({
  listNetworkEntries: (...args: unknown[]) => listNetworkEntriesMock(...args),
  loadCloudSetupStatus: (...args: unknown[]) => loadCloudSetupStatusMock(...args),
}))

import { createExplorerState } from './state'

const makeEntry = (name: string, path: string, kind: 'file' | 'dir' = 'file'): Entry => ({
  name,
  path,
  kind,
  iconId: 0,
})

const deferred = <T>() => {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

describe('createExplorerState sort refresh behavior', () => {
  afterEach(() => vi.useRealTimers())
  beforeEach(() => {
    listDirMock.mockReset()
    listRecentMock.mockReset().mockResolvedValue({ current: 'Recent', entries: [] })
    listStarredMock.mockReset().mockResolvedValue({ current: 'Starred', entries: [] })
    listTrashMock.mockReset().mockResolvedValue({ current: 'Trash', entries: [] })
    watchDirMock.mockReset().mockResolvedValue(undefined)
    listMountsMock.mockReset().mockResolvedValue([])
    listNetworkEntriesMock.mockReset().mockResolvedValue([])
    loadCloudSetupStatusMock.mockReset().mockResolvedValue({
      state: 'ready',
      configuredPath: null,
      resolvedBinaryPath: '/usr/bin/rclone',
      detectedRemoteCount: 0,
      supportedRemoteCount: 0,
      unsupportedRemoteCount: 0,
      supportedRemotes: [],
    })
  })

  it.each([true, false])('hands loading ownership to a silent replacement (old finishes first: %s)', async (oldFirst) => {
    vi.useFakeTimers()
    const state = createExplorerState()
    const oldReply = deferred<Listing>(), latestReply = deferred<Listing>()
    listDirMock.mockReturnValueOnce(oldReply.promise).mockReturnValueOnce(latestReply.promise)
    const old = state.load('/dest')
    const latest = state.load('/dest', { silent: true, recordHistory: false })
    if (oldFirst) {
      oldReply.resolve({ current: '/dest', entries: [makeEntry('stale.jpg', '/dest/stale.jpg')] })
      await old
    }
    await vi.advanceTimersByTimeAsync(200)
    expect(get(state.loading)).toBe(true)
    latestReply.resolve({ current: '/dest', entries: [makeEntry('photo.jpg', '/dest/photo.jpg')] })
    await latest
    await vi.advanceTimersByTimeAsync(200)
    expect(get(state.loading)).toBe(false)
    if (!oldFirst) {
      oldReply.resolve({ current: '/old', entries: [] })
      await old
    }
    expect(get(state.current)).toBe('/dest')
    expect(get(state.entries).map(entry => entry.name)).toEqual(['photo.jpg'])
  })

  it('clears inherited loading when the silent replacement fails without surfacing a stale failure', async () => {
    vi.useFakeTimers()
    const state = createExplorerState()
    const oldReply = deferred<Listing>(), latestReply = deferred<Listing>()
    listDirMock.mockReturnValueOnce(oldReply.promise).mockReturnValueOnce(latestReply.promise)
    const old = state.load('/dest')
    const latest = state.load('/dest', { silent: true })
    await vi.advanceTimersByTimeAsync(200)
    latestReply.reject(new Error('Refresh failed'))
    await latest
    await vi.advanceTimersByTimeAsync(200)
    expect(get(state.loading)).toBe(false)
    oldReply.reject(new Error('Stale failure'))
    await old
    expect(get(state.error)).toBe('Refresh failed')
  })

  it('keeps a newer foreground load busy when an older load completes', async () => {
    vi.useFakeTimers()
    const state = createExplorerState()
    const oldReply = deferred<Listing>(), latestReply = deferred<Listing>()
    listDirMock.mockReturnValueOnce(oldReply.promise).mockReturnValueOnce(latestReply.promise)
    const old = state.load('/old')
    const latest = state.load('/new')
    await vi.advanceTimersByTimeAsync(200)
    oldReply.resolve({ current: '/old', entries: [] })
    await old
    await vi.advanceTimersByTimeAsync(200)
    expect(get(state.loading)).toBe(true)
    latestReply.resolve({ current: '/new', entries: [] })
    await latest
    await vi.advanceTimersByTimeAsync(200)
    expect(get(state.loading)).toBe(false)
    expect(get(state.current)).toBe('/new')
  })

  it('clears loading when cancellation abandons a directory request', async () => {
    vi.useFakeTimers()
    const state = createExplorerState()
    const reply = deferred<Listing>()
    listDirMock.mockReturnValueOnce(reply.promise)
    const pending = state.load('/dest')
    await vi.advanceTimersByTimeAsync(200)
    expect(get(state.loading)).toBe(true)
    state.cancelSearch()
    await vi.advanceTimersByTimeAsync(200)
    expect(get(state.loading)).toBe(false)
    reply.resolve({ current: '/dest', entries: [] })
    await pending
    expect(get(state.current)).toBe('')
  })

  it('does not create a loading indicator for an ordinary silent refresh', async () => {
    vi.useFakeTimers()
    const state = createExplorerState()
    const reply = deferred<Listing>()
    listDirMock.mockReturnValueOnce(reply.promise)
    const pending = state.load('/dest', { silent: true })
    await vi.advanceTimersByTimeAsync(500)
    expect(get(state.loading)).toBe(false)
    reply.resolve({ current: '/dest', entries: [] })
    await pending
    expect(get(state.loading)).toBe(false)
  })

  it('does not overwrite a newer mount list with a stale response', async () => {
    const state = createExplorerState()
    let resolveOld!: (mounts: Array<{ path: string; label: string }>) => void
    listMountsMock.mockReturnValueOnce(new Promise((resolve) => { resolveOld = resolve }))
      .mockResolvedValueOnce([{ path: '/new', label: 'New' }])
    const oldRequest = state.loadPartitions()
    await state.loadPartitions()
    resolveOld([{ path: '/old', label: 'Old' }])
    await oldRequest
    expect(get(state.partitions)).toEqual([{ path: '/new', label: 'New' }])
  })

  it('surfaces default-app launch failures without rejecting and clears the error on retry', async () => {
    const onOpenEntry = vi.fn().mockRejectedValueOnce({ code: 'open_failed', message: 'Failed to open: executable missing' }).mockResolvedValueOnce(undefined)
    const state = createExplorerState({ onOpenEntry })
    const file = makeEntry('example.py', '/test/example.py')
    await expect(state.open(file)).resolves.toBeUndefined()
    expect(get(state.error)).toBe('Failed to open: executable missing')
    await state.open(file)
    expect(get(state.error)).toBe('')
    expect(onOpenEntry).toHaveBeenCalledTimes(2)
  })

  it('returns home when the active volume is among several removed volumes', async () => {
    const state = createExplorerState()
    listMountsMock.mockResolvedValueOnce([{ path: '/first', label: 'First' }, { path: '/second', label: 'Second' }])
      .mockResolvedValueOnce([])
    await state.loadPartitions()
    state.current.set('/second/folder')
    listDirMock.mockResolvedValue({ current: '/home', entries: [] })
    await state.loadPartitions()
    await vi.waitFor(() => expect(get(state.current)).toBe('/home'))
  })

  it('does not reload cloud directory from backend on sort toggle', async () => {
    listDirMock.mockResolvedValue({
      current: 'rclone://work/docs',
      entries: [
        makeEntry('alpha.txt', 'rclone://work/docs/alpha.txt'),
        makeEntry('beta.txt', 'rclone://work/docs/beta.txt'),
      ],
    })

    const state = createExplorerState()
    await state.load('rclone://work/docs')

    expect(listDirMock).toHaveBeenCalledTimes(1)
    expect(get(state.entries).map((entry) => entry.name)).toEqual(['alpha.txt', 'beta.txt'])

    await state.changeSort('name')

    expect(listDirMock).toHaveBeenCalledTimes(1)
    expect(get(state.entries).map((entry) => entry.name)).toEqual(['beta.txt', 'alpha.txt'])
  })

  const cameraPath = '/run/user/1000/gvfs/mtp:host=test/DCIM/Camera'
  const photo = (name: string, modified: string | null = null): Entry => ({ ...makeEntry(name, `${cameraPath}/${name}`), modified })

  it('preserves camera file order across metadata-driven background refreshes but allows manual refresh to sort', async () => {
    const state = createExplorerState()
    state.sortField.set('modified')
    listDirMock.mockResolvedValueOnce({ current: cameraPath, entries: [photo('a.jpg'), photo('b.jpg')] })
    await state.load(cameraPath)
    listDirMock.mockResolvedValue({ current: cameraPath, entries: [photo('b.jpg', '2026-01-01 12:00'), photo('a.jpg', '2026-01-02 12:00')] })
    await state.load(cameraPath, { silent: true, recordHistory: false })
    expect(get(state.entries).map(entry => entry.name)).toEqual(['a.jpg', 'b.jpg'])
    expect(get(state.entries)[1].modified).toBe('2026-01-01 12:00')
    expect(watchDirMock).toHaveBeenCalledOnce()
    await state.load(cameraPath, { recordHistory: false })
    expect(get(state.entries).map(entry => entry.name)).toEqual(['b.jpg', 'a.jpg'])
  })

  it('does not replace known metadata with placeholders or move remaining entries when files arrive or disappear', async () => {
    const state = createExplorerState()
    const a = { ...photo('a.jpg', '2026-01-02 12:00'), size: 20, iconId: 14, readOnly: true }
    listDirMock.mockResolvedValueOnce({ current: cameraPath, entries: [a, photo('b.jpg')] })
    await state.load(cameraPath)
    listDirMock.mockResolvedValue({ current: cameraPath, entries: [photo('c.jpg'), { ...photo('a.jpg'), kind: 'dir', iconId: 0, readOnly: false }], pendingMetadataPaths: [`${cameraPath}/a.jpg`] })
    await state.load(cameraPath, { silent: true, recordHistory: false })
    expect(get(state.entries).map(entry => entry.name)).toEqual(['a.jpg', 'c.jpg'])
    expect(get(state.entries)[0]).toMatchObject({ kind: 'file', size: 20, iconId: 14, modified: '2026-01-02 12:00', readOnly: true, metadataPending: true })
  })

  it('uses retained metadata when manually sorting a refreshed placeholder snapshot', async () => {
    const state = createExplorerState()
    state.sortField.set('modified')
    listDirMock.mockResolvedValueOnce({ current: cameraPath, entries: [photo('b.jpg', '2026-01-01 12:00'), photo('a.jpg', '2026-01-02 12:00')] })
    await state.load(cameraPath)
    listDirMock.mockResolvedValue({ current: cameraPath, entries: [photo('a.jpg'), photo('b.jpg')], pendingMetadataPaths: [`${cameraPath}/a.jpg`, `${cameraPath}/b.jpg`] })
    await state.load(cameraPath, { recordHistory: false })
    expect(get(state.entries).map(entry => entry.name)).toEqual(['b.jpg', 'a.jpg'])
  })

  it('applies early metadata when it beats the initial directory reply and ignores updates for other paths', async () => {
    const state = createExplorerState()
    let finish!: (value: unknown) => void
    listDirMock.mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
    const pending = state.load(cameraPath)
    state.applyEntryMetadata([{ ...photo('a.jpg', '2026-01-02 12:00'), size: 200 }, photo('outside.jpg')])
    finish({ current: cameraPath, entries: [photo('a.jpg')], pendingMetadataPaths: [`${cameraPath}/a.jpg`] })
    await pending
    expect(get(state.entries)).toHaveLength(1)
    expect(get(state.entries)[0]).toMatchObject({ size: 200, modified: '2026-01-02 12:00', metadataPending: false })
    state.applyEntryMetadata([{ ...photo('a.jpg'), size: 300 }, photo('outside.jpg')])
    expect(get(state.entries)).toHaveLength(1)
    expect(get(state.entries)[0].size).toBe(300)
  })

  it('ignores a delayed camera refresh after navigation to another directory', async () => {
    const state = createExplorerState()
    listDirMock.mockResolvedValueOnce({ current: cameraPath, entries: [photo('a.jpg')] })
    await state.load(cameraPath)
    let finish!: (value: unknown) => void
    listDirMock.mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
    const old = state.load(cameraPath, { silent: true, recordHistory: false })
    listDirMock.mockResolvedValueOnce({ current: '/local', entries: [makeEntry('local.txt', '/local/local.txt')] })
    await state.load('/local')
    finish({ current: cameraPath, entries: [photo('stale.jpg')] })
    await old
    expect(get(state.current)).toBe('/local')
    expect(get(state.entries).map(entry => entry.path)).toEqual(['/local/local.txt'])
    expect(watchDirMock.mock.calls.map(call => call[0])).toEqual([cameraPath, '/local'])
  })

  it('does not publish unchanged camera snapshots or reset the current-path callback', async () => {
    const onEntriesChanged = vi.fn(), onCurrentChange = vi.fn()
    const state = createExplorerState({ onEntriesChanged, onCurrentChange })
    listDirMock.mockResolvedValue({ current: cameraPath, entries: [photo('a.jpg')] })
    await state.load(cameraPath)
    const before = get(state.entries)
    await state.load(cameraPath, { silent: true, recordHistory: false })
    expect(get(state.entries)).toBe(before)
    expect(onEntriesChanged).toHaveBeenCalledOnce()
    expect(onCurrentChange).toHaveBeenCalledOnce()
  })

  it('sorts GVFS entries in memory and rejects an earlier unsorted background reply', async () => {
    const state = createExplorerState()
    listDirMock.mockResolvedValueOnce({ current: cameraPath, entries: [photo('a.jpg'), photo('b.jpg')] })
    await state.load(cameraPath)
    let finish!: (value: unknown) => void
    listDirMock.mockReturnValueOnce(new Promise(resolve => { finish = resolve }))
    const pending = state.load(cameraPath, { silent: true, recordHistory: false })
    await state.changeSort('name')
    expect(get(state.entries).map(entry => entry.name)).toEqual(['b.jpg', 'a.jpg'])
    finish({ current: cameraPath, entries: [photo('a.jpg'), photo('b.jpg')] })
    await pending
    expect(get(state.entries).map(entry => entry.name)).toEqual(['b.jpg', 'a.jpg'])
    expect(listDirMock).toHaveBeenCalledTimes(2)
  })

  it('reloads local directory from backend on sort toggle', async () => {
    listDirMock
      .mockResolvedValueOnce({
        current: '/tmp/work',
        entries: [makeEntry('a.txt', '/tmp/work/a.txt')],
      })
      .mockResolvedValueOnce({
        current: '/tmp/work',
        entries: [makeEntry('b.txt', '/tmp/work/b.txt')],
      })

    const state = createExplorerState()
    await state.load('/tmp/work')
    await state.changeSort('name')

    expect(listDirMock).toHaveBeenCalledTimes(2)
  })

  it('shows a network onboarding hint when cloud setup is not ready and no cloud entries exist', async () => {
    loadCloudSetupStatusMock.mockResolvedValue({
      state: 'binary_missing',
      configuredPath: null,
      resolvedBinaryPath: null,
      detectedRemoteCount: 0,
      supportedRemoteCount: 0,
      unsupportedRemoteCount: 0,
      supportedRemotes: [],
    })

    const state = createExplorerState()
    state.cloudEnabled.set(true)
    await state.loadNetwork()

    expect(loadCloudSetupStatusMock).toHaveBeenCalledTimes(1)
    expect(get(state.networkNotice)).toContain('Install rclone')
  })

  it.each([
    [
      'invalid_binary_path',
      'Fix the Rclone path in Settings > Cloud.',
    ],
    [
      'config_read_failed',
      'could not read the saved rclone setting',
    ],
    [
      'runtime_unusable',
      'Update rclone',
    ],
    [
      'no_supported_remotes',
      'Run `rclone config`',
    ],
  ] as const)(
    'shows the correct network onboarding hint for %s',
    async (setupState, expectedNoticePart) => {
      loadCloudSetupStatusMock.mockResolvedValue({
        state: setupState,
        configuredPath: null,
        resolvedBinaryPath: '/usr/bin/rclone',
        detectedRemoteCount: 0,
        supportedRemoteCount: 0,
        unsupportedRemoteCount: 0,
        supportedRemotes: [],
      })

      const state = createExplorerState()
      state.cloudEnabled.set(true)
      await state.loadNetwork()

      expect(loadCloudSetupStatusMock).toHaveBeenCalledTimes(1)
      expect(get(state.networkNotice)).toContain(expectedNoticePart)
    },
  )

  it('does not show a network onboarding hint for transient discovery failures', async () => {
    loadCloudSetupStatusMock.mockResolvedValue({
      state: 'discovery_failed',
      configuredPath: null,
      resolvedBinaryPath: '/usr/bin/rclone',
      detectedRemoteCount: 0,
      supportedRemoteCount: 0,
      unsupportedRemoteCount: 0,
      supportedRemotes: [],
    })

    const state = createExplorerState()
    state.cloudEnabled.set(true)
    await state.loadNetwork()

    expect(loadCloudSetupStatusMock).toHaveBeenCalledTimes(1)
    expect(get(state.networkNotice)).toBe('')
  })

  it('does not show a network onboarding hint when cloud setup is ready but no cloud entries are listed yet', async () => {
    loadCloudSetupStatusMock.mockResolvedValue({
      state: 'ready',
      configuredPath: null,
      resolvedBinaryPath: '/usr/bin/rclone',
      detectedRemoteCount: 1,
      supportedRemoteCount: 1,
      unsupportedRemoteCount: 0,
      supportedRemotes: [],
    })

    const state = createExplorerState()
    state.cloudEnabled.set(true)
    await state.loadNetwork()

    expect(loadCloudSetupStatusMock).toHaveBeenCalledTimes(1)
    expect(get(state.networkNotice)).toBe('')
  })

  it('keeps non-cloud network entries usable when cloud setup reports a guided failure state', async () => {
    listNetworkEntriesMock.mockResolvedValue([makeEntry('NAS', 'smb://nas', 'dir')])
    loadCloudSetupStatusMock.mockResolvedValue({
      state: 'runtime_unusable',
      configuredPath: null,
      resolvedBinaryPath: '/usr/bin/rclone',
      detectedRemoteCount: 0,
      supportedRemoteCount: 0,
      unsupportedRemoteCount: 0,
      supportedRemotes: [],
    })

    const state = createExplorerState()
    state.cloudEnabled.set(true)
    await state.loadNetwork()

    expect(get(state.entries).map((entry) => entry.path)).toEqual(['smb://nas'])
    expect(get(state.error)).toBe('')
    expect(get(state.networkNotice)).toContain('Update rclone')
  })

  it('keeps non-cloud network entries usable when cloud setup probing throws', async () => {
    listNetworkEntriesMock.mockResolvedValue([makeEntry('NAS', 'smb://nas', 'dir')])
    loadCloudSetupStatusMock.mockRejectedValue(new Error('rclone rc unavailable'))

    const state = createExplorerState()
    state.cloudEnabled.set(true)
    await state.loadNetwork()

    expect(get(state.entries).map((entry) => entry.path)).toEqual(['smb://nas'])
    expect(get(state.error)).toBe('')
    expect(get(state.networkNotice)).toBe('')
  })

  it('does not show cloud entries or probe cloud setup when cloud is disabled', async () => {
    listNetworkEntriesMock.mockResolvedValue([
      makeEntry('browsey-gdrive (Google Drive)', 'rclone://browsey-gdrive', 'dir'),
      makeEntry('NAS', 'smb://nas', 'dir'),
    ])

    const state = createExplorerState()
    state.cloudEnabled.set(false)
    await state.loadNetwork()

    expect(loadCloudSetupStatusMock).not.toHaveBeenCalled()
    expect(get(state.entries).map((entry) => entry.path)).toEqual(['smb://nas'])
    expect(get(state.networkNotice)).toBe('')
  })

  it('does not probe cloud setup when network already shows cloud entries', async () => {
    listNetworkEntriesMock.mockResolvedValue([
      makeEntry('browsey-gdrive (Google Drive)', 'rclone://browsey-gdrive', 'dir'),
    ])

    const state = createExplorerState()
    state.cloudEnabled.set(true)
    await state.loadNetwork()

    expect(loadCloudSetupStatusMock).not.toHaveBeenCalled()
    expect(get(state.entries)).toHaveLength(1)
    expect(get(state.networkNotice)).toBe('')
  })
})
