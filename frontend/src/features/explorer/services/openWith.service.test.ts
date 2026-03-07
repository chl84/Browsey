import { beforeEach, describe, expect, it, vi } from 'vitest'

const invokeMock = vi.fn()

vi.mock('@/shared/lib/tauri', () => ({
  invoke: invokeMock,
}))

describe('openWith service', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    invokeMock.mockReset()
  })

  it('normalizes app-not-found errors for the open-with UI', async () => {
    const { openWithSelection } = await import('./openWith.service')
    invokeMock.mockRejectedValueOnce({
      code: 'app_not_found',
      message: 'Selected application is unavailable: GNOME Text Editor',
    })

    await expect(
      openWithSelection('/home/chris/docs/report.txt', { appId: 'desktop:test' }),
    ).rejects.toMatchObject({
      code: 'app_not_found',
      message: 'The selected application is unavailable',
    })
  })

  it('normalizes launch-failed errors for the open-with UI', async () => {
    const { openWithSelection } = await import('./openWith.service')
    invokeMock.mockRejectedValueOnce({
      code: 'launch_failed',
      message: 'Failed to launch GNOME Text Editor: exit status 1',
    })

    await expect(
      openWithSelection('/home/chris/docs/report.txt', { appId: 'desktop:test' }),
    ).rejects.toMatchObject({
      code: 'launch_failed',
      message: 'Browsey could not start the selected application',
    })
  })
})
