import { get } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createCompressModal } from './compressModal'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@/shared/lib/tauri', () => ({ invoke }))

const setup = () => {
  const deps = {
    activityApi: { start: vi.fn().mockResolvedValue(undefined), cleanup: vi.fn().mockResolvedValue(undefined), clearNow: vi.fn(), requestCancel: vi.fn().mockResolvedValue(undefined) },
    reloadCurrent: vi.fn().mockResolvedValue(undefined),
    showToast: vi.fn(),
  }
  const modal = createCompressModal(deps)
  modal.open([{ path: '/test/file.txt', name: 'file.txt', kind: 'file', iconId: 0 }], 'Archive')
  return { modal, deps }
}

describe('compression lifecycle', () => {
  beforeEach(() => { invoke.mockReset() })

  it('sends the exact password only for the current operation, without adding it to modal state', async () => {
    const { modal } = setup()
    invoke.mockResolvedValue('/test/Archive.zip')
    await modal.confirm('Archive', 6, ' blåbær🔑 ')
    expect(invoke).toHaveBeenLastCalledWith('compress_entries', expect.objectContaining({ password: ' blåbær🔑 ' }))
    expect(JSON.stringify(get(modal.state))).not.toContain('blåbær')
    modal.open([{ path: '/test/file.txt', name: 'file.txt', kind: 'file', iconId: 0 }], 'Archive')
    await modal.confirm('Archive', 6)
    expect(invoke.mock.calls[1][1]).not.toHaveProperty('password')
  })

  it('keeps errors and targets visible, then closes after a successful retry', async () => {
    const { modal, deps } = setup()
    invoke.mockRejectedValueOnce(new Error('No space left')).mockResolvedValueOnce('/test/Archive.zip')
    expect(await modal.confirm('Archive', 6)).toBe(false)
    expect(get(modal.state)).toMatchObject({ open: true, error: 'No space left' })
    expect(get(modal.state).targets).toHaveLength(1)
    expect(await modal.confirm('Archive', 6)).toBe(true)
    expect(get(modal.state).open).toBe(false)
    expect(deps.reloadCurrent).toHaveBeenCalledOnce()
    expect(deps.activityApi.cleanup).toHaveBeenCalledTimes(2)
  })

  it('ignores a duplicate submit without closing the active dialog', async () => {
    let resolve!: (path: string) => void
    invoke.mockReturnValue(new Promise<string>((done) => { resolve = done }))
    const { modal } = setup()
    const first = modal.confirm('Archive', 6)
    expect(await modal.confirm('Archive', 6)).toBe(false)
    expect(get(modal.state).open).toBe(true)
    resolve('/test/Archive.zip')
    await first
    expect(invoke).toHaveBeenCalledOnce()
  })

  it('requests cancellation while busy without hiding eventual failures', async () => {
    let reject!: (error: Error) => void
    invoke.mockReturnValue(new Promise((_resolve, fail) => { reject = fail }))
    const { modal, deps } = setup()
    const pending = modal.confirm('Archive', 6)
    await Promise.resolve()
    modal.close()
    expect(deps.activityApi.requestCancel).toHaveBeenCalledWith(expect.stringMatching(/^compress-progress-/))
    expect(get(modal.state).open).toBe(true)
    reject(new Error('Compression cancelled'))
    await pending
    expect(get(modal.state).open).toBe(false)
  })
})
