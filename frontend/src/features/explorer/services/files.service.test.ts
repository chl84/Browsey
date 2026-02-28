import { describe, expect, it, vi } from 'vitest'

const invokeMock = vi.fn()

vi.mock('@/shared/lib/tauri', () => ({
  invoke: invokeMock,
}))

describe('openEntry', () => {
  it('opens cloud files via open_cloud_entry', async () => {
    const { openEntry } = await import('./files.service')

    await openEntry({
      name: 'report.txt',
      path: 'rclone://work/docs/report.txt',
      kind: 'file',
    } as never)

    expect(invokeMock).toHaveBeenCalledWith('open_cloud_entry', {
      path: 'rclone://work/docs/report.txt',
    })
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
