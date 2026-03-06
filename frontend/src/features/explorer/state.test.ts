import { get } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Entry } from './model/types'

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

describe('createExplorerState sort refresh behavior', () => {
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
