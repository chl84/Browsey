import { describe, expect, it, vi } from 'vitest'

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
