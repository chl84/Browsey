import { beforeEach, describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'
import { createRecoveryBackupsModel, type RecoveryBackup } from './recoveryBackups'

const { invoke, cancelTask, handlers } = vi.hoisted(() => ({
  invoke: vi.fn(), cancelTask: vi.fn(), handlers: new Map<string, (event: { payload: unknown }) => void>(),
}))
vi.mock('@/shared/lib/tauri', () => ({ invoke }))
vi.mock('@/features/explorer/services/activity.service', () => ({ cancelTask }))
vi.mock('@tauri-apps/api/event', () => ({
  listen: async (event: string, handler: (event: { payload: unknown }) => void) => {
    handlers.set(event, handler)
    return () => { handlers.delete(event) }
  },
}))
const backup: RecoveryBackup = {
  id: 'session-fixture/bucket/file.txt', version: 'version', name: 'file.txt',
  kind: 'file', bytes: 2048, modifiedAt: null, blockedReason: null,
}

describe('backup recovery', () => {
  beforeEach(() => { invoke.mockReset(); cancelTask.mockReset(); handlers.clear() })

  it('recovers to the original location without prompting for a folder', async () => {
    invoke.mockResolvedValue('/original/file.txt')
    const model = createRecoveryBackupsModel()
    model.overview.set({ entries: [backup], incomplete: false })
    expect(await model.restore(backup)).toBe('restored')
    expect(invoke).toHaveBeenCalledWith('restore_recovery_backup', expect.objectContaining({ destinationDir: null }))
    expect(get(model.restoredPath)).toBe('/original/file.txt')
    expect(get(model.restoredName)).toBe('file.txt')
    expect(get(model.overview)?.entries).toEqual([])
    expect(handlers.size).toBe(0)
    model.dispose()
  })

  it('offers a chosen folder after the original destination fails, without issuing a second copy automatically', async () => {
    invoke.mockRejectedValueOnce({ code: 'recovery_destination_unavailable', message: 'The original folder is missing.' })
    const model = createRecoveryBackupsModel()
    model.overview.set({ entries: [backup], incomplete: false })
    expect(await model.restore(backup)).toBe('choose-destination')
    expect(invoke).toHaveBeenCalledOnce()
    expect(get(model.restoredPath)).toBe('')
    expect(get(model.overview)?.entries).toEqual([backup])
    expect(get(model.error)).toBe('The original folder is missing.')
    expect(get(model.restoring)).toBe(false)
    expect(handlers.size).toBe(0)
    invoke.mockResolvedValueOnce('/chosen/file.txt')
    expect(await model.restore(backup, '/chosen')).toBe('restored')
    expect(invoke).toHaveBeenCalledTimes(2)
    model.dispose()
  })

  it('does not offer another destination on cancellation or an unavailable backup', async () => {
    for (const code of ['cancelled', 'lock_failed', 'snapshot_mismatch', 'recovery_record_failed']) {
      invoke.mockRejectedValueOnce({ code, message: 'Recovery stopped' })
      const model = createRecoveryBackupsModel()
      model.overview.set({ entries: [backup], incomplete: false })
      expect(await model.restore(backup)).toBeUndefined()
      expect(get(model.restoredPath)).toBe('')
      expect(get(model.overview)?.entries).toEqual([backup])
      expect(handlers.size).toBe(0)
      model.dispose()
    }
  })

  it('presents only a typed original-location conflict as a neutral notice', async () => {
    invoke.mockRejectedValueOnce({ code: 'recovery_destination_unavailable', message: 'Original location is occupied.', details: { reason: 'occupied' } })
    const model = createRecoveryBackupsModel()
    expect(await model.restore(backup)).toBe('choose-destination')
    expect(get(model.notice)).toBe('Original location is occupied. Choose another folder.')
    expect(get(model.error)).toBe('')
    invoke.mockRejectedValueOnce({ code: 'recovery_destination_unavailable', message: 'Inspect incomplete output at /original/file.txt before retrying.' })
    expect(await model.restore(backup)).toBe('choose-destination')
    expect(get(model.notice)).toBe('')
    expect(get(model.error)).toContain('Inspect incomplete output')
    invoke.mockRejectedValueOnce({ code: 'target_exists', message: 'Chosen destination is occupied.', details: { reason: 'occupied' } })
    expect(await model.restore(backup, '/chosen')).toBeUndefined()
    expect(get(model.notice)).toBe('')
    expect(get(model.error)).toBe('Chosen destination is occupied.')
    model.dispose()
  })

  it('keeps a cancelled original-destination check from opening the folder picker', async () => {
    let fail!: (error: unknown) => void
    invoke.mockReturnValue(new Promise((_, reject) => { fail = reject }))
    const model = createRecoveryBackupsModel()
    const pending = model.restore(backup)
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledOnce())
    await model.cancel()
    fail({ code: 'recovery_destination_unavailable', message: 'The original folder is missing.' })
    expect(await pending).toBeUndefined()
    expect(get(model.error)).toContain('Recovery cancelled')
    expect(handlers.size).toBe(0)
    model.dispose()
  })

  it('uses shared byte progress and waits for the verified command reply before showing success', async () => {
    let finish!: (path: string) => void
    invoke.mockReturnValue(new Promise<string>(resolve => { finish = resolve }))
    const model = createRecoveryBackupsModel()
    const other = { ...backup, id: 'session-fixture/other/other.txt', name: 'other.txt' }
    model.overview.set({ entries: [backup, other], incomplete: false })
    const pending = model.restore(backup, '/chosen')
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledOnce())
    const args = invoke.mock.calls[0][1]
    expect(invoke.mock.calls[0][0]).toBe('restore_recovery_backup')
    expect(args).toMatchObject({ id: backup.id, version: backup.version, destinationDir: '/chosen' })
    handlers.get(args.progressEvent)?.({ payload: { bytes: 1024, total: 2048 } })
    expect(get(model.activity)).toMatchObject({ percent: 50, detail: '1.00 KB / 2.00 KB' })
    handlers.get(args.progressEvent)?.({ payload: { bytes: 2048, total: 2048, finished: true } })
    expect(get(model.restoring)).toBe(true)
    expect(get(model.restoredPath)).toBe('')
    expect(get(model.overview)?.entries).toEqual([backup, other])
    finish('/chosen/file.txt')
    await pending
    expect(get(model.restoredPath)).toBe('/chosen/file.txt')
    expect(get(model.overview)?.entries).toEqual([other])
    expect(get(model.restoring)).toBe(false)
    expect(handlers.size).toBe(0)
    expect(invoke).not.toHaveBeenCalledWith('undo_action')
    model.dispose()
  })

  it('refuses unavailable rows and suppresses repeated restoration', async () => {
    const model = createRecoveryBackupsModel()
    await model.restore({ ...backup, blockedReason: 'In use' }, '/chosen')
    await model.restore(backup, '')
    expect(invoke).not.toHaveBeenCalled()
    let finish!: (path: string) => void
    invoke.mockReturnValue(new Promise<string>(resolve => { finish = resolve }))
    const pending = model.restore(backup, '/chosen')
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledOnce())
    await model.restore(backup, '/other')
    expect(invoke).toHaveBeenCalledOnce()
    finish('/chosen/file.txt'); await pending; model.dispose()
  })

  it('cancels without claiming success and keeps the recovery error useful', async () => {
    let fail!: (error: unknown) => void
    invoke.mockReturnValue(new Promise((_, reject) => { fail = reject }))
    const model = createRecoveryBackupsModel()
    const pending = model.restore(backup, '/chosen')
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledOnce())
    await model.cancel()
    expect(cancelTask).toHaveBeenCalledWith(invoke.mock.calls[0][1].progressEvent)
    fail({ code: 'cancelled', message: 'cancelled' }); await pending
    expect(get(model.error)).toContain('backup was kept')
    expect(get(model.error)).toContain('incomplete copy')
    expect(get(model.restoredPath)).toBe('')
    expect(handlers.size).toBe(0); model.dispose()
  })

  it('cancels on destruction and ignores late results while releasing its progress listener', async () => {
    let finish!: (path: string) => void
    invoke.mockReturnValue(new Promise<string>(resolve => { finish = resolve }))
    const model = createRecoveryBackupsModel()
    const pending = model.restore(backup, '/chosen')
    await vi.waitFor(() => expect(invoke).toHaveBeenCalledOnce())
    model.dispose()
    expect(cancelTask).toHaveBeenCalledOnce()
    finish('/chosen/file.txt'); await pending
    expect(get(model.restoredPath)).toBe(''); expect(handlers.size).toBe(0)
  })

  it('does not start file work if destroyed while the progress listener is being prepared', async () => {
    const model = createRecoveryBackupsModel()
    const pending = model.restore(backup, '/chosen')
    model.dispose(); await pending
    expect(invoke).not.toHaveBeenCalled(); expect(handlers.size).toBe(0)
  })

  it('preserves a stale listing on refresh failure and discards late scans', async () => {
    const model = createRecoveryBackupsModel()
    invoke.mockResolvedValueOnce({ entries: [backup], incomplete: false })
    await model.refresh()
    invoke.mockRejectedValueOnce(new Error('Permission denied')); await model.refresh()
    expect(get(model.overview)?.entries).toEqual([backup])
    expect(get(model.error)).toBe('Permission denied')
    let finish!: (value: unknown) => void
    invoke.mockReturnValue(new Promise(resolve => { finish = resolve }))
    const pending = model.refresh(); await model.refresh()
    model.dispose(); finish({ entries: [], incomplete: false }); await pending
    expect(get(model.overview)?.entries).toEqual([backup])
  })
})
