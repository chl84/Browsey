import { get } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createOpenWithModal } from './openWithModal'

const { invoke, prepareCloudWorkingCopy, cancelTask, handlers } = vi.hoisted(() => ({ invoke: vi.fn(), prepareCloudWorkingCopy: vi.fn(), cancelTask: vi.fn(), handlers: new Map<string, (event: { payload: unknown }) => void>() }))
vi.mock('@/shared/lib/tauri', () => ({ invoke }))
vi.mock('@/features/network', () => ({ prepareCloudWorkingCopy }))
vi.mock('../services/activity.service', () => ({ cancelTask }))
vi.mock('@tauri-apps/api/event', () => ({
  listen: async (name: string, handler: (event: { payload: unknown }) => void) => {
    handlers.set(name, handler)
    return () => { handlers.delete(name) }
  },
}))

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
  beforeEach(() => { invoke.mockReset(); prepareCloudWorkingCopy.mockReset(); cancelTask.mockReset(); handlers.clear() })

  it('shows measured download progress and cancels preparation when the modal closes', async () => {
    let resolve!: (copy: { localPath: string }) => void
    prepareCloudWorkingCopy.mockReturnValue(new Promise(done => { resolve = done }))
    const modal = createOpenWithModal({ showToast: vi.fn() })
    modal.open({ path: 'rclone://work/report.txt', name: 'report.txt', kind: 'file', iconId: 0 })
    await vi.waitFor(() => expect(prepareCloudWorkingCopy).toHaveBeenCalledOnce())
    const event = prepareCloudWorkingCopy.mock.calls[0][1] as string
    handlers.get(event)?.({ payload: { bytes: 1024, total: 2048 } })
    expect(get(modal.state).progress).toMatchObject({ percent: 50, detail: '1.00 KB / 2.00 KB' })
    modal.close()
    expect(cancelTask).toHaveBeenCalledWith(event)
    resolve({ localPath: '/private/work/report.txt' })
    await vi.waitFor(() => expect(handlers.has(event)).toBe(false))
    expect(get(modal.state).open).toBe(false)
    expect(invoke).not.toHaveBeenCalled()
  })

  it('uses the durable local copy for cloud app discovery, default and opening', async () => {
    prepareCloudWorkingCopy.mockResolvedValue({ localPath: '/private/work/report.txt' })
    invoke.mockResolvedValueOnce(apps)
    const showToast = vi.fn()
    const modal = createOpenWithModal({ showToast })
    modal.open({ path: 'rclone://work/report.txt', name: 'report.txt', kind: 'file', iconId: 0 })
    await vi.waitFor(() => expect(get(modal.state).loading).toBe(false))
    expect(prepareCloudWorkingCopy).toHaveBeenCalledExactlyOnceWith('rclone://work/report.txt', expect.stringMatching(/^cloud-open-with-/))
    expect(invoke).toHaveBeenCalledWith('list_open_with_apps', { path: '/private/work/report.txt' })
    invoke.mockClear()
    await modal.confirm({ appId: 'editor', setDefault: true })
    expect(invoke).toHaveBeenNthCalledWith(1, 'set_default_app', {
      path: '/private/work/report.txt', appId: 'editor', contentType: 'text/plain',
    })
    expect(invoke).toHaveBeenNthCalledWith(2, 'open_with', {
      path: '/private/work/report.txt', choice: { appId: 'editor' },
    })
    expect(showToast).toHaveBeenCalledWith(expect.stringContaining('Cloud saves shows upload status'))
  })

  it('does not launch anything when the cloud working copy could not be prepared', async () => {
    prepareCloudWorkingCopy.mockRejectedValue(new Error('Offline'))
    const modal = createOpenWithModal({ showToast: vi.fn() })
    modal.open({ path: 'rclone://work/report.txt', name: 'report.txt', kind: 'file', iconId: 0 })
    await vi.waitFor(() => expect(get(modal.state).loading).toBe(false))
    await modal.confirm({ appId: '__default__' })
    expect(get(modal.state).error).toBe('Offline')
    expect(invoke).not.toHaveBeenCalled()
  })

  it('ignores a cloud preparation completing after another file is selected', async () => {
    let resolve!: (copy: { localPath: string }) => void
    prepareCloudWorkingCopy.mockReturnValue(new Promise((done) => { resolve = done }))
    const modal = createOpenWithModal({ showToast: vi.fn() })
    modal.open({ path: 'rclone://work/old.txt', name: 'old.txt', kind: 'file', iconId: 0 })
    invoke.mockResolvedValueOnce(apps)
    modal.open({ path: '/test/new.txt', name: 'new.txt', kind: 'file', iconId: 0 })
    await vi.waitFor(() => expect(get(modal.state).loading).toBe(false))
    resolve({ localPath: '/private/work/old.txt' })
    await Promise.resolve()
    invoke.mockClear()
    await modal.confirm({ appId: 'editor' })
    expect(invoke).toHaveBeenCalledExactlyOnceWith('open_with', {
      path: '/test/new.txt', choice: { appId: 'editor' },
    })
  })

  it('saves the file-type default before opening the file when checked', async () => {
    const { modal, showToast } = await setup()
    invoke.mockResolvedValueOnce(undefined)
    await modal.confirm({ appId: 'editor', setDefault: true })
    expect(invoke).toHaveBeenNthCalledWith(1, 'set_default_app', {
      path: '/test/file.txt', appId: 'editor', contentType: 'text/plain',
    })
    expect(invoke).toHaveBeenNthCalledWith(2, 'open_with', {
      path: '/test/file.txt', choice: { appId: 'editor' },
    })
    expect(invoke).toHaveBeenCalledTimes(2)
    expect(showToast).toHaveBeenCalledWith('Opening file.txt… Editor is now the default for text/plain')
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
    expect(invoke).toHaveBeenCalledExactlyOnceWith('set_default_app', {
      path: '/test/file.txt', appId: 'editor', contentType: 'text/plain',
    })
    invoke.mockResolvedValueOnce(undefined)
    await modal.confirm({ appId: 'editor', setDefault: true })
    expect(get(modal.state).open).toBe(false)
  })

  it('reports partial success if saving worked but opening failed', async () => {
    const { modal, showToast } = await setup()
    invoke.mockResolvedValueOnce(undefined).mockRejectedValueOnce({ code: 'launch_failed', message: 'Missing executable' })
    await modal.confirm({ appId: 'editor', setDefault: true })
    expect(get(modal.state)).toMatchObject({
      open: true, submitting: false,
      error: 'The default application was saved, but the file could not be opened: Browsey could not start the selected application',
    })
    expect(showToast).not.toHaveBeenCalled()
    invoke.mockClear()
    invoke.mockResolvedValueOnce(undefined)
    await modal.confirm({ appId: 'editor', setDefault: false })
    expect(invoke).toHaveBeenCalledExactlyOnceWith('open_with', { path: '/test/file.txt', choice: { appId: 'editor' } })
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
