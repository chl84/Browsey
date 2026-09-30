import { beforeEach, describe, expect, it, vi } from 'vitest'

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }))
vi.mock('@/shared/lib/tauri', () => ({ invoke: invokeMock }))

import { fetchWindowControlPolicy } from './windowControls.service'

describe('window control policy service', () => {
  beforeEach(() => {
    invokeMock.mockReset()
  })

  it.each([
    { minimize: false, maximize: false },
    { minimize: true, maximize: true },
    { minimize: false, maximize: true },
  ])('preserves the backend policy: %j', async (policy) => {
    invokeMock.mockResolvedValue(policy)
    await expect(fetchWindowControlPolicy()).resolves.toEqual(policy)
    expect(invokeMock).toHaveBeenCalledWith('get_window_control_policy')
  })

  it('propagates errors so the titlebar can retain its safe default', async () => {
    invokeMock.mockRejectedValue(new Error('IPC unavailable'))
    await expect(fetchWindowControlPolicy()).rejects.toThrow('IPC unavailable')
  })
})
