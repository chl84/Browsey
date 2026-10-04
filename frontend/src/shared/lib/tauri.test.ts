import { beforeEach, describe, expect, it, vi } from 'vitest'
import { getErrorCode, getErrorMessage, normalizeError } from './error'
import { invoke } from './tauri'

const { rawInvoke } = vi.hoisted(() => ({ rawInvoke: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: rawInvoke, convertFileSrc: vi.fn() }))

describe('Tauri error boundary', () => {
  beforeEach(() => rawInvoke.mockReset())

  it('passes successful results and arguments through without alteration', async () => {
    const result = { updated: true }
    const args = { path: '/mock/file.txt' }
    rawInvoke.mockResolvedValueOnce(result)
    await expect(invoke('test_command', args)).resolves.toBe(result)
    expect(rawInvoke).toHaveBeenCalledExactlyOnceWith('test_command', args)
  })

  it.each([
    { error: { code: 'helper_missing', message: 'Nested diagnostic' } },
    JSON.stringify({ code: 'auth_cancelled', message: 'Encoded diagnostic' }),
  ])('preserves typed error classification across IPC (%j)', async raw => {
    rawInvoke.mockRejectedValueOnce(raw)
    const failure = await invoke('test_command').then(
      () => { throw new Error('Expected rejection') },
      (error: unknown) => error,
    )
    expect(failure).toBeInstanceOf(Error)
    expect(getErrorCode(failure)).toBe(getErrorCode(raw))
    expect(getErrorMessage(failure)).toBe(getErrorMessage(raw))
    expect(normalizeError(failure).raw).toBe(raw)
    expect(normalizeError(failure)).toBe(failure)
    expect(rawInvoke).toHaveBeenCalledExactlyOnceWith('test_command', undefined)
  })

  it('retains an existing immutable Error, including its stack and metadata', async () => {
    const original = Object.freeze(Object.assign(new Error('Original diagnostic'), { code: 'readonly_filesystem' }))
    rawInvoke.mockRejectedValueOnce(original)
    await expect(invoke('test_command')).rejects.toBe(original)
    expect(rawInvoke).toHaveBeenCalledOnce()
  })

  it('does not infer a typed code from backend prose or retry a failed command', async () => {
    rawInvoke.mockRejectedValueOnce('permission_denied')
    const failure = await invoke('test_command').then(
      () => { throw new Error('Expected rejection') },
      (error: unknown) => error,
    )
    expect(getErrorCode(failure)).toBeNull()
    expect(getErrorMessage(failure)).toBe('permission_denied')
    expect(rawInvoke).toHaveBeenCalledOnce()
  })
})
