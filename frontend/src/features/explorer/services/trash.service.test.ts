import { beforeEach, describe, expect, it, vi } from 'vitest'

const invokeMock = vi.fn()
const statCloudEntryMock = vi.fn()
const deleteCloudFileMock = vi.fn()
const deleteCloudDirRecursiveMock = vi.fn()

vi.mock('@/shared/lib/tauri', () => ({
  invoke: invokeMock,
}))

vi.mock('@/features/network', async () => {
  const actual = await vi.importActual<object>('@/features/network')
  return {
    ...actual,
    statCloudEntry: (...args: unknown[]) => statCloudEntryMock(...args),
    deleteCloudFile: (...args: unknown[]) => deleteCloudFileMock(...args),
    deleteCloudDirRecursive: (...args: unknown[]) => deleteCloudDirRecursiveMock(...args),
  }
})

describe('deleteEntries', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    invokeMock.mockResolvedValue(undefined)
    statCloudEntryMock.mockResolvedValue(null)
    deleteCloudFileMock.mockResolvedValue(undefined)
    deleteCloudDirRecursiveMock.mockResolvedValue(undefined)
  })

  it('uses local delete command for non-cloud paths', async () => {
    const { deleteEntries } = await import('./trash.service')

    await deleteEntries(['/tmp/report.txt'], 'delete-progress-1')

    expect(invokeMock).toHaveBeenCalledWith('delete_entries', {
      paths: ['/tmp/report.txt'],
      progressEvent: 'delete-progress-1',
    })
    expect(statCloudEntryMock).not.toHaveBeenCalled()
  })

  it('tries file + dir delete when cloud stat is missing', async () => {
    const { deleteEntries } = await import('./trash.service')

    deleteCloudFileMock.mockRejectedValueOnce({
      code: 'not_found',
      message: 'file not found',
    })

    await deleteEntries(['rclone://work/docs/new-folder'], 'delete-progress-2')

    expect(statCloudEntryMock).toHaveBeenCalledWith('rclone://work/docs/new-folder')
    expect(deleteCloudFileMock).toHaveBeenCalledWith(
      'rclone://work/docs/new-folder',
      'delete-progress-2',
    )
    expect(deleteCloudDirRecursiveMock).toHaveBeenCalledWith(
      'rclone://work/docs/new-folder',
      'delete-progress-2',
    )
  })

  it('fails with explicit error when cloud delete cannot be verified', async () => {
    const { deleteEntries } = await import('./trash.service')

    deleteCloudFileMock.mockRejectedValueOnce({
      code: 'not_found',
      message: 'file not found',
    })
    deleteCloudDirRecursiveMock.mockRejectedValueOnce({
      code: 'not_found',
      message: 'directory not found',
    })

    await expect(deleteEntries(['rclone://work/docs/new-folder'])).rejects.toThrow(
      'Cloud delete could not be verified for "rclone://work/docs/new-folder". Refresh and try again.',
    )
  })

  it('uses directory delete when stat says dir', async () => {
    const { deleteEntries } = await import('./trash.service')

    statCloudEntryMock.mockResolvedValueOnce({
      kind: 'dir',
    })

    await deleteEntries(['rclone://work/docs/new-folder'])

    expect(deleteCloudDirRecursiveMock).toHaveBeenCalledWith('rclone://work/docs/new-folder', undefined)
    expect(deleteCloudFileMock).not.toHaveBeenCalled()
  })
})
