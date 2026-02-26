import { get } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { Entry } from '../model/types'

const renameEntryMock = vi.fn()
const createFolderMock = vi.fn()
const deleteEntriesMock = vi.fn()
const purgeTrashItemsMock = vi.fn()

vi.mock('../services/files.service', () => ({
  renameEntry: (...args: unknown[]) => renameEntryMock(...args),
  createFolder: (...args: unknown[]) => createFolderMock(...args),
}))

vi.mock('../services/trash.service', () => ({
  deleteEntries: (...args: unknown[]) => deleteEntriesMock(...args),
  purgeTrashItems: (...args: unknown[]) => purgeTrashItemsMock(...args),
}))

vi.mock('@/shared/lib/error', () => ({
  getErrorMessage: (err: unknown) => (err instanceof Error ? err.message : String(err)),
}))

import { createDeleteConfirmModal } from './deleteConfirmModal'
import { createNewFolderModal } from './newFolderModal'
import { createRenameModal } from './renameModal'

const flushMicrotasks = async () => {
  await Promise.resolve()
  await Promise.resolve()
}

const createActivityApi = () => ({
  start: vi.fn(async () => {}),
  hideSoon: vi.fn(),
  cleanup: vi.fn(async () => {}),
  clearNow: vi.fn(),
  hasHideTimer: vi.fn(() => false),
})

const makeEntry = (path: string, kind: Entry['kind'] = 'file'): Entry => ({
  name: path.split('/').pop() ?? path,
  path,
  kind,
  iconId: 0,
})

describe('cloud modal background refresh', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    renameEntryMock.mockResolvedValue(undefined)
    createFolderMock.mockResolvedValue('rclone://work/docs/New folder')
    deleteEntriesMock.mockResolvedValue(undefined)
    purgeTrashItemsMock.mockResolvedValue(undefined)
  })

  it('rename modal succeeds and soft-fails cloud refresh in background', async () => {
    const loadPath = vi.fn(async () => {
      throw new Error('Cloud operation timed out')
    })
    const showToast = vi.fn()
    const activityApi = createActivityApi()
    const modal = createRenameModal({
      loadPath,
      parentPath: () => 'rclone://work/docs',
      getCurrentPath: () => 'rclone://work/docs',
      showToast,
      activityApi,
    })

    modal.open(makeEntry('rclone://work/docs/report.txt'))
    const ok = await modal.confirm('report-renamed.txt')
    await flushMicrotasks()

    expect(ok).toBe(true)
    expect(renameEntryMock).toHaveBeenCalledWith('rclone://work/docs/report.txt', 'report-renamed.txt')
    expect(loadPath).toHaveBeenCalledWith('rclone://work/docs')
    expect(showToast).toHaveBeenCalledWith(
      'Rename completed, but refresh took too long. Press F5 to refresh.',
      3500,
    )
    expect(get(modal.state)).toEqual({ open: false, target: null, error: '' })
  })

  it('new folder modal succeeds and soft-fails cloud refresh in background', async () => {
    const loadPath = vi.fn(async () => {
      throw new Error('Cloud operation timed out')
    })
    const showToast = vi.fn()
    const activityApi = createActivityApi()
    const modal = createNewFolderModal({
      getCurrentPath: () => 'rclone://work/docs',
      loadPath,
      showToast,
      activityApi,
    })

    modal.open()
    const created = await modal.confirm('New folder')
    await flushMicrotasks()

    expect(created).toBe('rclone://work/docs/New folder')
    expect(createFolderMock).toHaveBeenCalledWith('rclone://work/docs', 'New folder')
    expect(loadPath).toHaveBeenCalledWith('rclone://work/docs')
    expect(showToast).toHaveBeenCalledWith(
      'Folder created, but refresh took too long. Press F5 to refresh.',
    )
    expect(get(modal.state)).toEqual({ open: false, error: '' })
  })

  it('delete modal succeeds and soft-fails cloud refresh in background', async () => {
    const reloadCurrent = vi.fn(async () => {
      throw new Error('Cloud operation timed out')
    })
    const showToast = vi.fn()
    const activityApi = {
      start: vi.fn(async () => {}),
      cleanup: vi.fn(async () => {}),
      clearNow: vi.fn(),
      hasHideTimer: vi.fn(() => false),
    }
    const modal = createDeleteConfirmModal({
      activityApi,
      reloadCurrent,
      getCurrentPath: () => 'rclone://work/docs',
      showToast,
    })

    modal.open([makeEntry('rclone://work/docs/sample.txt')])
    await modal.confirm()
    await flushMicrotasks()

    expect(deleteEntriesMock).toHaveBeenCalledTimes(1)
    expect(deleteEntriesMock).toHaveBeenCalledWith(
      ['rclone://work/docs/sample.txt'],
      expect.stringMatching(/^delete-progress-/),
    )
    expect(reloadCurrent).toHaveBeenCalledTimes(1)
    expect(showToast).toHaveBeenCalledWith(
      'Delete completed, but refresh took too long. Press F5 to refresh.',
    )
    const toastCalls = showToast.mock.calls.map((args) => String(args[0] ?? ''))
    expect(toastCalls.some((msg) => msg.startsWith('Delete failed:'))).toBe(false)
    expect(get(modal.state)).toEqual({ open: false, targets: [], mode: 'default' })
  })
})
