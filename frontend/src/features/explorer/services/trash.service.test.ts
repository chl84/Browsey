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
    invokeMock.mockImplementation(async (cmd: string) => cmd === 'network_delete_paths' ? [] : undefined)
    statCloudEntryMock.mockResolvedValue(null)
    deleteCloudFileMock.mockResolvedValue(undefined)
    deleteCloudDirRecursiveMock.mockResolvedValue(undefined)
  })

  it('only advances cloud item progress after successful deletes and stops at an error', async () => {
    const { deleteEntries } = await import('./trash.service')
    const paths = ['rclone://work/a', 'rclone://work/b', 'rclone://work/c']
    statCloudEntryMock.mockResolvedValue({ kind: 'file' })
    deleteCloudFileMock.mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error('Denied'))
    const progress = vi.fn()
    await expect(deleteEntries(paths, 'delete', false, progress)).rejects.toThrow('Denied')
    expect(progress.mock.calls.map(([payload]) => [payload.items, payload.total])).toEqual([[0, 3], [1, 3]])
    expect(deleteCloudFileMock).toHaveBeenCalledTimes(2)
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

  it('routes network deletion without a local backup and requires explicit approval', async () => {
    const { deleteEntries } = await import('./trash.service')
    const path = '/run/user/1000/gvfs/sftp:host=server/large.bin'
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'network_delete_paths') return [path]
      if (cmd === 'network_delete_entries') throw { code: 'network_confirmation_required', message: 'Confirm' }
    })
    await expect(deleteEntries([path], 'progress')).rejects.toMatchObject({ code: 'network_confirmation_required' })
    expect(invokeMock).toHaveBeenCalledWith('network_delete_entries', { paths: [path], trash: false, confirmed: false, progressEvent: 'progress' })
    expect(invokeMock).not.toHaveBeenCalledWith('delete_entries', expect.anything())
  })

  it('keeps mixed local/network targets in one job with shared progress and confirmation', async () => {
    const { deleteEntries } = await import('./trash.service')
    const network = '/mnt/share/large.bin'
    const paths = ['/tmp/local.txt', network]
    invokeMock.mockImplementation(async (cmd: string) => cmd === 'network_delete_paths' ? [network] : undefined)
    await deleteEntries(paths, 'progress', true)
    expect(invokeMock).toHaveBeenCalledWith('network_delete_entries', { paths, trash: false, confirmed: true, progressEvent: 'progress' })
    expect(invokeMock).not.toHaveBeenCalledWith('delete_entries', expect.anything())
  })

  it('never falls back from a failed network trash call to permanent or local deletion', async () => {
    const { moveToTrashMany } = await import('./trash.service')
    const path = '/mnt/share/file'
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === 'network_delete_paths') return [path]
      throw { code: 'permission_denied', message: 'Denied' }
    })
    await expect(moveToTrashMany([path], 'progress')).rejects.toMatchObject({ code: 'permission_denied' })
    expect(invokeMock).toHaveBeenCalledWith('network_delete_entries', { paths: [path], trash: true, confirmed: false, progressEvent: 'progress' })
    expect(invokeMock).toHaveBeenCalledTimes(2)
  })

  it('sends approval for unsupported trash only after confirmation', async () => {
    const { moveToTrashMany } = await import('./trash.service')
    const path = '/mnt/share/file'
    invokeMock.mockImplementation(async (cmd: string) => cmd === 'network_delete_paths' ? [path] : undefined)
    await moveToTrashMany([path], 'progress', true)
    expect(invokeMock).toHaveBeenCalledWith('network_delete_entries', { paths: [path], trash: true, confirmed: true, progressEvent: 'progress' })
  })

  it('does not mutate files if backend policy lookup fails', async () => {
    const { deleteEntries } = await import('./trash.service')
    invokeMock.mockRejectedValueOnce(new Error('Mount table unavailable'))
    await expect(deleteEntries(['/tmp/file'])).rejects.toThrow('Mount table unavailable')
    expect(invokeMock).toHaveBeenCalledExactlyOnceWith('network_delete_paths', { paths: ['/tmp/file'] })
  })

  it('empties the native system trash without lossy per-entry identifiers', async () => {
    const { emptyTrash } = await import('./trash.service')
    await emptyTrash()
    expect(invokeMock).toHaveBeenCalledExactlyOnceWith('empty_trash')
    expect(deleteCloudFileMock).not.toHaveBeenCalled()
    expect(deleteCloudDirRecursiveMock).not.toHaveBeenCalled()
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
