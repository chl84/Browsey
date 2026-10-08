import { beforeEach, describe, expect, it, vi } from 'vitest'
import { writable } from 'svelte/store'
import { createSelectionActions } from './createSelectionActions'
import type { Entry } from '../model/types'
import type { ClipboardApi } from './createClipboard'

const services = vi.hoisted(() => ({
  deleteEntries: vi.fn(), moveToTrashMany: vi.fn(), purgeTrashItems: vi.fn(),
  copyPathsToSystemClipboard: vi.fn(),
}))
vi.mock('../services/trash.service', () => ({
  ...services,
  needsNetworkDeleteConfirmation: (error: { code?: string }) => error?.code === 'network_confirmation_required',
}))
vi.mock('../services/clipboard.service', () => services)

const entry: Entry = { path: '/tmp/file', name: 'file', kind: 'file', iconId: 0 }
const setup = (overrides: Partial<Parameters<typeof createSelectionActions>[0]> = {}) => {
  const clipboard: ClipboardApi = {
    state: writable({ mode: 'copy', paths: new Set<string>() }), clear: vi.fn(),
    copy: vi.fn(), cut: vi.fn(), paste: vi.fn(),
    copyPaths: vi.fn(async () => ({ ok: true as const })),
    cutPaths: vi.fn(async () => ({ ok: true as const })),
  }
  const deps = {
    clipboard,
    activityApi: { start: vi.fn(), cleanup: vi.fn(), clearNow: vi.fn(),
      requestCancel: vi.fn(), hideSoon: vi.fn(), hasHideTimer: () => false },
    currentView: () => 'dir' as const,
    getCurrentPath: () => '/tmp', confirmDeleteEnabled: () => true,
    confirmDelete: vi.fn(), reloadCurrent: vi.fn(), showToast: vi.fn(),
    isWindows: () => false,
    ...overrides,
  }
  return { ...deps, actions: createSelectionActions(deps) }
}

describe('shared selection actions', () => {
  beforeEach(() => vi.resetAllMocks())

  it.each(['copy', 'cut'] as const)('preserves internal %s when desktop clipboard sync fails', async mode => {
    services.copyPathsToSystemClipboard.mockRejectedValueOnce(new Error('unavailable'))
    const { actions, clipboard, showToast } = setup()
    expect(await actions[mode]([entry.path])).toBe(true)
    expect(clipboard[mode === 'copy' ? 'copyPaths' : 'cutPaths']).toHaveBeenCalledWith([entry.path])
    expect(services.copyPathsToSystemClipboard).toHaveBeenCalledWith([entry.path], mode === 'cut' ? 'cut' : undefined)
    expect(showToast).toHaveBeenCalledWith(`${mode === 'cut' ? 'Cut' : 'Copied'} (system clipboard unavailable: unavailable)`, 2500)
  })

  it('does not export cloud clipboard paths to desktop applications', async () => {
    const { actions } = setup()
    await actions.copy(['rclone://remote/file'])
    await actions.cut(['rclone://remote/file'])
    expect(services.copyPathsToSystemClipboard).not.toHaveBeenCalled()
  })

  it('stops before desktop sync if the internal clipboard fails', async () => {
    const { actions, clipboard, showToast } = setup()
    vi.mocked(clipboard.copyPaths).mockResolvedValueOnce({ ok: false, error: 'Nothing selected' })
    expect(await actions.copy([])).toBe(false)
    expect(showToast).toHaveBeenCalledWith('Copy failed: Nothing selected')
    expect(services.copyPathsToSystemClipboard).not.toHaveBeenCalled()
  })

  it('does not cut or delete connection roots in the Network view', async () => {
    const { actions, clipboard } = setup({ currentView: () => 'network' })
    expect(await actions.cut([entry.path])).toBe(false)
    expect(await actions.trash([entry])).toBe(false)
    expect(await actions.deletePermanently([entry])).toBe(false)
    expect(clipboard.cutPaths).not.toHaveBeenCalled()
    expect(services.deleteEntries).not.toHaveBeenCalled()
  })

  it('uses shared progress and cancellation for normal trash', async () => {
    const { actions, activityApi } = setup()
    await actions.trash([entry])
    const [label, event, cancel] = vi.mocked(activityApi.start).mock.calls[0]
    expect(label).toBe('Moving to trash…')
    expect(services.moveToTrashMany).toHaveBeenCalledWith([entry.path], event)
    cancel?.()
    expect(activityApi.requestCancel).toHaveBeenCalledWith(event)
    expect(activityApi.cleanup).toHaveBeenCalledWith(true)
  })

  it('uses the existing undo-recording trash command for Wastebasket drops', async () => {
    const { actions, activityApi, confirmDelete, reloadCurrent } = setup()
    expect(await actions.trashDropped([entry.path])).toBe(true)
    expect(services.moveToTrashMany).toHaveBeenCalledWith([entry.path], expect.any(String))
    expect(activityApi.start).toHaveBeenCalledWith('Moving to trash…', expect.any(String), expect.any(Function), { completeOnReply: false })
    expect(reloadCurrent).toHaveBeenCalledOnce()
    expect(confirmDelete).not.toHaveBeenCalled()
    expect(services.deleteEntries).not.toHaveBeenCalled()
  })

  it('rejects unavailable trash without entering permanent-delete confirmation', async () => {
    services.moveToTrashMany.mockRejectedValueOnce({ code: 'network_confirmation_required', message: 'No trash' })
    const { actions, confirmDelete, activityApi, showToast } = setup({ confirmDeleteEnabled: () => false })
    expect(await actions.trashDropped([entry.path])).toBe(false)
    expect(confirmDelete).not.toHaveBeenCalled()
    expect(services.deleteEntries).not.toHaveBeenCalled()
    expect(activityApi.cleanup).toHaveBeenCalledWith(true)
    expect(showToast).toHaveBeenCalledWith(expect.stringContaining('Move to trash failed'))
  })

  it('does not turn incoming file drops into a purge while browsing Wastebasket', async () => {
    const { actions } = setup({ currentView: () => 'trash' })
    expect(await actions.trashDropped([entry.path])).toBe(true)
    expect(services.purgeTrashItems).not.toHaveBeenCalled()
    expect(services.moveToTrashMany).toHaveBeenCalledWith([entry.path], expect.any(String))
  })

  it('routes cloud drops to provider trash without permanent-delete confirmation', async () => {
    const { actions, confirmDelete, showToast } = setup()
    const paths = ['rclone://Google Disk/duplicate~id-one', 'rclone://Google Disk/duplicate~id-two']
    expect(await actions.trashDropped(paths)).toBe(true)
    expect(services.moveToTrashMany).toHaveBeenCalledWith(paths, expect.any(String))
    expect(confirmDelete).not.toHaveBeenCalled()
    expect(showToast).toHaveBeenCalledWith('Moved to cloud trash. Restore items from the provider website.')
  })

  it.each(['network-trash', 'network'] as const)('requires backend-requested %s confirmation even when settings disable ordinary confirmation', async mode => {
    const service = mode === 'network-trash' ? services.moveToTrashMany : services.deleteEntries
    service.mockRejectedValueOnce({ code: 'network_confirmation_required' })
    const { actions, confirmDelete, reloadCurrent } = setup({ confirmDeleteEnabled: () => false })
    await (mode === 'network-trash' ? actions.trash([entry]) : actions.deletePermanently([entry]))
    expect(confirmDelete).toHaveBeenCalledWith([entry], mode)
    expect(reloadCurrent).not.toHaveBeenCalled()
    expect(service).toHaveBeenCalledTimes(1)
  })

  it('finishes the old progress cleanup before exposing network confirmation', async () => {
    services.moveToTrashMany.mockRejectedValueOnce({ code: 'network_confirmation_required' })
    let finishCleanup!: () => void
    let cleanupStarted!: () => void
    const started = new Promise<void>(resolve => { cleanupStarted = resolve })
    const { actions, activityApi, confirmDelete } = setup({ confirmDeleteEnabled: () => false })
    vi.mocked(activityApi.cleanup).mockImplementationOnce(() => {
      cleanupStarted()
      return new Promise<void>(resolve => { finishCleanup = resolve })
    })
    const pending = actions.trash([entry])
    await started
    expect(confirmDelete).not.toHaveBeenCalled()
    expect(await actions.trash([entry])).toBe(false)
    finishCleanup()
    expect(await pending).toBe(true)
    expect(confirmDelete).toHaveBeenCalledExactlyOnceWith([entry], 'network-trash')
    expect(vi.mocked(activityApi.clearNow).mock.invocationCallOrder[0])
      .toBeLessThan(vi.mocked(confirmDelete).mock.invocationCallOrder[0])
    expect(services.moveToTrashMany).toHaveBeenCalledOnce()
  })

  it('confirms permanent deletion without starting activity or mutation', async () => {
    const { actions, confirmDelete, activityApi } = setup()
    await actions.deletePermanently([entry])
    expect(confirmDelete).toHaveBeenCalledWith([entry], 'default')
    expect(activityApi.start).not.toHaveBeenCalled()
    expect(services.deleteEntries).not.toHaveBeenCalled()
  })

  it('retains forced confirmation and Windows/cloud unsupported-trash policy', async () => {
    const { actions, confirmDelete } = setup({ confirmDeleteEnabled: () => false })
    await actions.deletePermanently([entry], true)
    await actions.trash([{ ...entry, path: 'rclone://remote/file', capabilities: undefined }])
    expect(confirmDelete).toHaveBeenCalledTimes(2)
    const windows = setup({ isWindows: () => true })
    await windows.actions.trash([{ ...entry, network: true }])
    expect(windows.confirmDelete).toHaveBeenCalled()
    expect(services.moveToTrashMany).not.toHaveBeenCalled()
  })

  it('reports refresh failure after successful deletion without calling deletion a failure', async () => {
    const { actions, reloadCurrent, showToast } = setup({ confirmDeleteEnabled: () => false })
    vi.mocked(reloadCurrent).mockRejectedValueOnce(new Error('refresh unavailable'))
    expect(await actions.deletePermanently([entry])).toBe(true)
    expect(showToast).toHaveBeenLastCalledWith('Delete completed, but refresh took too long. Press F5 to refresh.')
    expect(showToast).not.toHaveBeenCalledWith(expect.stringContaining('Delete failed'))
    expect(services.deleteEntries).toHaveBeenCalledTimes(1)
  })

  it('preserves a partial mutation error when the refresh also fails and never retries', async () => {
    services.moveToTrashMany.mockRejectedValueOnce(new Error('1 of 2 items completed'))
    const { actions, reloadCurrent, showToast } = setup()
    vi.mocked(reloadCurrent).mockRejectedValueOnce(new Error('refresh unavailable'))
    expect(await actions.trash([entry])).toBe(false)
    expect(showToast).toHaveBeenCalledWith('Move to trash failed: 1 of 2 items completed')
    expect(services.moveToTrashMany).toHaveBeenCalledTimes(1)
    expect(services.deleteEntries).not.toHaveBeenCalled()
  })

  it('uses trash IDs and does not enter the native delete/trash services in Wastebasket', async () => {
    const { actions, activityApi } = setup({ currentView: () => 'trash' })
    await actions.trash([{ ...entry, trash_id: 'trash-id' }])
    expect(services.purgeTrashItems).toHaveBeenCalledWith(['trash-id'])
    // The purge command has no cancellation token; do not offer a fake Cancel.
    expect(vi.mocked(activityApi.start).mock.calls[0][2]).toBeUndefined()
    expect(services.moveToTrashMany).not.toHaveBeenCalled()
    expect(services.deleteEntries).not.toHaveBeenCalled()
  })

  it('prevents duplicate mutations and does not refresh a newly navigated location', async () => {
    let release!: () => void
    let location = '/tmp'
    services.deleteEntries.mockImplementationOnce(() => new Promise<void>(resolve => { release = resolve }))
    const { actions, reloadCurrent } = setup({ confirmDeleteEnabled: () => false, getCurrentPath: () => location })
    const first = actions.deletePermanently([entry])
    await Promise.resolve()
    expect(await actions.deletePermanently([entry])).toBe(false)
    location = '/elsewhere'
    release()
    await first
    expect(services.deleteEntries).toHaveBeenCalledTimes(1)
    expect(reloadCurrent).not.toHaveBeenCalled()
  })

  it('does not keep cloud permanent deletion busy while a refresh is pending', async () => {
    const { actions, reloadCurrent, activityApi } = setup({ confirmDeleteEnabled: () => false })
    vi.mocked(reloadCurrent).mockImplementationOnce(() => new Promise<void>(() => {}))
    await actions.deletePermanently([{ ...entry, path: 'rclone://remote/file' }])
    expect(activityApi.cleanup).toHaveBeenCalledWith(true)
    expect(activityApi.hideSoon).toHaveBeenCalled()
  })

  it('reports a cloud mutation failure without waiting indefinitely for partial-result refresh', async () => {
    services.deleteEntries.mockRejectedValueOnce(new Error('network disconnected'))
    const { actions, reloadCurrent, activityApi, showToast } = setup({ confirmDeleteEnabled: () => false })
    vi.mocked(reloadCurrent).mockImplementationOnce(() => new Promise<void>(() => {}))
    expect(await actions.deletePermanently([{ ...entry, path: 'rclone://remote/file' }])).toBe(false)
    expect(showToast).toHaveBeenCalledWith('Delete failed: network disconnected')
    expect(activityApi.cleanup).toHaveBeenCalledWith(true)
    expect(services.deleteEntries).toHaveBeenCalledTimes(1)
  })
})
