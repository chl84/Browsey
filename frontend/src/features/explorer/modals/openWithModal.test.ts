import { get } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createOpenWithModal } from './openWithModal'

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }))
vi.mock('@/shared/lib/tauri', () => ({ invoke }))

const apps = [{ id: 'editor', name: 'Editor', defaultContentType: 'text/plain', matches: true, terminal: false, exec: 'editor' }]
const setup = async () => {
  invoke.mockResolvedValueOnce(apps)
  const showToast = vi.fn()
  const modal = createOpenWithModal({ showToast })
  modal.open({ path: '/test/file.txt', name: 'file.txt', kind: 'file', iconId: 0 })
  await vi.waitFor(() => expect(get(modal.state).loading).toBe(false))
  invoke.mockClear()
  return { modal, showToast }
}

describe('open-with defaults', () => {
  beforeEach(() => { invoke.mockReset() })

  it('sets the file-type default without opening the file', async () => {
    const { modal, showToast } = await setup()
    invoke.mockResolvedValueOnce(undefined)
    await modal.confirm({ appId: 'editor', setDefault: true })
    expect(invoke).toHaveBeenCalledExactlyOnceWith('set_default_app', {
      path: '/test/file.txt', appId: 'editor', contentType: 'text/plain',
    })
    expect(showToast).toHaveBeenCalledWith('Editor is now the default for text/plain')
    expect(get(modal.state).open).toBe(false)
  })

  it('ordinary Open never changes the default', async () => {
    const { modal } = await setup()
    await modal.confirm({ appId: 'editor' })
    expect(invoke).toHaveBeenCalledExactlyOnceWith('open_with', {
      path: '/test/file.txt', choice: { appId: 'editor' },
    })
  })

  it('rejects unknown selections and the Open normally default pseudo-app', async () => {
    const { modal } = await setup()
    await modal.confirm({ appId: 'unknown', setDefault: true })
    await modal.confirm({ appId: '__default__', setDefault: true })
    expect(invoke).not.toHaveBeenCalled()
    expect(get(modal.state).error).toContain('A default cannot be set')
  })

  it('keeps save failures visible for retry, without opening the file', async () => {
    const { modal, showToast } = await setup()
    invoke.mockRejectedValueOnce({ code: 'default_app_failed', message: 'Could not save the default application: Permission denied' })
    await modal.confirm({ appId: 'editor', setDefault: true })
    expect(get(modal.state)).toMatchObject({ open: true, submitting: false, error: 'Could not save the default application: Permission denied' })
    expect(showToast).not.toHaveBeenCalled()
    invoke.mockResolvedValueOnce(undefined)
    await modal.confirm({ appId: 'editor', setDefault: true })
    expect(get(modal.state).open).toBe(false)
  })

  it('blocks duplicate submissions, closing, and target changes while saving', async () => {
    const { modal } = await setup()
    let resolve!: () => void
    invoke.mockReturnValueOnce(new Promise<void>((done) => { resolve = done }))
    const pending = modal.confirm({ appId: 'editor', setDefault: true })
    await modal.confirm({ appId: 'editor', setDefault: true })
    modal.close()
    modal.open({ path: '/test/other.txt', name: 'other.txt', kind: 'file', iconId: 0 })
    expect(get(modal.state)).toMatchObject({ open: true, submitting: true, entry: { path: '/test/file.txt' } })
    expect(invoke).toHaveBeenCalledOnce()
    resolve()
    await pending
    expect(get(modal.state).open).toBe(false)
  })
})
