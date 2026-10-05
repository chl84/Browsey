import { beforeEach, describe, expect, it, vi } from 'vitest'

const invokeMock = vi.fn()
const openCloudEntryMock = vi.fn()

vi.mock('@/shared/lib/tauri', () => ({
  invoke: invokeMock,
}))

vi.mock('@/features/network', async () => {
  const actual = await vi.importActual<object>('@/features/network')
  return {
    ...actual,
    openCloudEntry: openCloudEntryMock,
    createCloudFolder: vi.fn(),
    renameCloudEntry: vi.fn(),
  }
})

describe('openEntry', () => {
  beforeEach(() => { invokeMock.mockReset(); openCloudEntryMock.mockReset() })
  it('opens cloud files via open_cloud_entry', async () => {
    const { openEntry } = await import('./files.service')

    await openEntry({
      name: 'report.txt',
      path: 'rclone://work/docs/report.txt',
      kind: 'file',
    } as never, { progressEvent: 'cloud-open-1' })

    expect(openCloudEntryMock).toHaveBeenCalledWith('rclone://work/docs/report.txt', 'cloud-open-1')
  })

  it('opens local files via open_entry', async () => {
    const { openEntry } = await import('./files.service')

    await openEntry({
      name: 'report.txt',
      path: '/tmp/report.txt',
      kind: 'file',
    } as never)

    expect(invokeMock).toHaveBeenCalledWith('open_entry', {
      path: '/tmp/report.txt',
    })
  })
})

describe('cloud write and archive routing', () => {
  beforeEach(() => { invokeMock.mockReset() })

  it('rejects invalid file and folder leaf names before any local or cloud mutation', async () => {
    const { createFile, createFolder } = await import('./files.service')
    const { createCloudFolder } = await import('@/features/network')
    vi.mocked(createCloudFolder).mockClear()
    for (const base of ['/generated/owned', 'rclone://work/generated']) {
      for (const name of ['', ' ', '.', ' .. ', 'bad/name', 'bad\\name', 'bad\0name']) {
        expect(() => createFile(base, name)).toThrow('Invalid file name')
        await expect(createFolder(base, name)).rejects.toThrow('Invalid folder name')
      }
    }
    expect(invokeMock).not.toHaveBeenCalled()
    expect(createCloudFolder).not.toHaveBeenCalled()
  })

  it('creates cloud files through the provider command and rejects escaping names', async () => {
    const { createFile } = await import('./files.service')
    invokeMock.mockResolvedValue('rclone://work/docs/new.txt')
    await createFile('rclone://work/docs/', 'new.txt')
    expect(invokeMock).toHaveBeenCalledExactlyOnceWith('create_cloud_file', { path: 'rclone://work/docs/new.txt' })
    invokeMock.mockClear()
    for (const name of ['../secret', '.', '..', 'bad\\name', ' ', 'bad\0name']) {
      expect(() => createFile('rclone://work/docs', name)).toThrow('Invalid file name')
    }
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('reports partial cloud rename failure and refuses mixed local/cloud plans', async () => {
    const { renameEntries } = await import('./files.service')
    invokeMock.mockResolvedValue({ renamed: ['rclone://work/new.txt'], error: '1 item renamed; refresh and retry remaining items' })
    await expect(renameEntries([{ path: 'rclone://work/old.txt', newName: 'new.txt' }])).rejects.toThrow('1 item renamed')
    expect(invokeMock).toHaveBeenCalledOnce()
    invokeMock.mockClear()
    await expect(renameEntries([
      { path: 'rclone://work/old.txt', newName: 'new.txt' },
      { path: '/tmp/local.txt', newName: 'local-new.txt' },
    ])).rejects.toThrow('separately')
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('routes cloud extraction and password retry through the staged command', async () => {
    const { extractArchive } = await import('./files.service')
    await extractArchive('rclone://work/protected.zip', 'extract-event', ' secret ')
    expect(invokeMock).toHaveBeenCalledExactlyOnceWith('extract_cloud_archive', {
      path: 'rclone://work/protected.zip', progressEvent: 'extract-event', password: ' secret ',
    })
  })

  it('retains typed password errors and stops cloud batch extraction on cancellation', async () => {
    const { extractArchives } = await import('./files.service')
    invokeMock.mockRejectedValueOnce({ code: 'archive_password_required', message: 'Password required' })
      .mockRejectedValueOnce({ code: 'cancelled', message: 'Cancelled' })
    const results = await extractArchives(['rclone://work/a.zip', 'rclone://work/b.zip', 'rclone://work/c.zip'], 'event')
    expect(results).toHaveLength(2)
    expect(results[0]).toMatchObject({ ok: false, error_code: 'archive_password_required' })
    expect(results[1]).toMatchObject({ ok: false, error_code: 'cancelled' })
    expect(invokeMock).toHaveBeenCalledTimes(2)
  })
})
