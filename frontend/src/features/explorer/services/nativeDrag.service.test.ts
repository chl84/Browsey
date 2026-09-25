import { beforeEach, describe, expect, it, vi } from 'vitest'
import { startNativeFileDrag } from './nativeDrag.service'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('../../../shared/lib/tauri', () => ({ invoke: invokeMock }))

describe('native file export', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    invokeMock.mockReset().mockResolvedValue(undefined)
  })

  it('uses the copy-only command without a callback channel or external icon', async () => {
    expect(await startNativeFileDrag(['/tmp/photo.jpg'])).toBe(true)
    expect(invokeMock).toHaveBeenCalledExactlyOnceWith('start_native_file_drag', {
      paths: ['/tmp/photo.jpg'],
    })
  })

  it('does not start empty, cloud, or mixed exports', async () => {
    expect(await startNativeFileDrag([])).toBe(false)
    expect(await startNativeFileDrag(['rclone://drive/photo.jpg'])).toBe(false)
    expect(await startNativeFileDrag(['/tmp/photo.jpg', 'rclone://drive/photo.jpg'])).toBe(false)
    expect(invokeMock).not.toHaveBeenCalled()
  })

  it('reports startup errors without an unhandled rejection', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    invokeMock.mockRejectedValueOnce(new Error('window closed'))
    expect(await startNativeFileDrag(['/tmp/photo.jpg'])).toBe(false)
  })
})
