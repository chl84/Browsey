import { beforeEach, expect, it, vi } from 'vitest'
const invokeMock = vi.fn()
vi.mock('@/shared/lib/tauri', () => ({ invoke: (...args: unknown[]) => invokeMock(...args) }))
import { copyCloudEntry, moveCloudEntry } from './cloud.service'
beforeEach(() => { invokeMock.mockReset() })
it.each(['unsupported', 'invalid_path'])('preserves the actionable %s transfer diagnostic and metadata', async code => {
  const message = code === 'unsupported'
    ? 'Cannot overwrite a file with a folder or a folder with a file on this route; use Auto-rename or Skip'
    : 'Cannot transfer a folder into itself or its descendant'
  invokeMock.mockRejectedValue({ code, message, details: { failed: 1 } })
  const error = await copyCloudEntry('rclone://test/src', 'rclone://test/dst').catch(error => error)
  expect(error.message).toBe(message); expect(error.code).toBe(code); expect(error.details).toEqual({ failed: 1 })
  expect(invokeMock).toHaveBeenCalledTimes(1)
})
it('retains friendly connection feedback and does not replay a failed move', async () => {
  invokeMock.mockRejectedValue({ code: 'network_error', message: 'transport detail' })
  const error = await moveCloudEntry('rclone://test/src', 'rclone://test/dst').catch(error => error)
  expect(error.code).toBe('network_error')
  expect(error.message).toBe('Cloud connection failed. Check the network and try again')
  expect(invokeMock).toHaveBeenCalledTimes(1)
})
