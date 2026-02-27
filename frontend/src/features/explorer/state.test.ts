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
} = vi.hoisted(() => ({
  listDirMock: vi.fn(),
  listRecentMock: vi.fn(),
  listStarredMock: vi.fn(),
  listTrashMock: vi.fn(),
  watchDirMock: vi.fn(),
  listMountsMock: vi.fn(),
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
  listNetworkEntries: vi.fn().mockResolvedValue([]),
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
})
