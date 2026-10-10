import { describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'
import { createUndoStorageModel, describeUndoStorage, describeUndoStorageOverview, type UndoStorageSummary } from './undoStorage'

const fixture: UndoStorageSummary = {
  directory: '/fixture/browsey/undo-sessions', exists: true, sessions: 3,
  markedSessions: 1, files: 4, logicalBytes: 8192, incomplete: false,
}

describe('undo storage diagnostics', () => {
  it('distinguishes retained recovered backups from pending items using root counts', () => {
    expect(describeUndoStorageOverview({ ...fixture, backupCount: 5, recoveredBackups: 3 }))
      .toBe('8.2 kB stored · 3 of 5 backups recovered')
    expect(describeUndoStorageOverview({ ...fixture, backupCount: 5, recoveredBackups: 5 }))
      .toBe('8.2 kB stored · All backups recovered')
    expect(describeUndoStorageOverview({ ...fixture, backupCount: 1, recoveredBackups: 1, files: 20 }))
      .toBe('8.2 kB stored · All backups recovered')
    for (const counts of [
      { backupCount: 5, recoveredBackups: 0 },
      { backupCount: null, recoveredBackups: null },
      { backupCount: 0, recoveredBackups: 0 },
      { backupCount: 1, recoveredBackups: 2 },
    ]) expect(describeUndoStorageOverview({ ...fixture, ...counts })).toBe('8.2 kB stored in backups.')
    expect(describeUndoStorageOverview({ ...fixture, backupCount: 5, recoveredBackups: 5, incomplete: true }))
      .toBe('Partial scan: at least 8.2 kB stored.')
  })
  it('keeps the overview concise without losing partial-scan or missing-storage states', () => {
    expect(describeUndoStorageOverview(fixture)).toBe('8.2 kB stored in backups.')
    expect(describeUndoStorageOverview({ ...fixture, sessions: 1, allocatedBytes: 4096 }))
      .toBe('8.2 kB stored in backups.')
    expect(describeUndoStorageOverview({ ...fixture, incomplete: true, logicalBytes: 0 }))
      .toBe('Partial scan: at least 0 B stored.')
    expect(describeUndoStorageOverview({ ...fixture, exists: false }))
      .toBe('No backups found.')
    expect(describeUndoStorageOverview({ ...fixture, sessions: 0, files: 0, logicalBytes: 0 }))
      .toBe('No backups found.')
  })
  it('labels allocation separately and supports backends without block accounting', () => {
    expect(describeUndoStorage({ ...fixture, allocatedBytes: 4 * 1024 * 1024 }))
      .toContain('4.2 MB allocated according to the filesystem')
    expect(describeUndoStorage({ ...fixture, allocatedBytes: 0 }))
      .toContain('0 B allocated according to the filesystem')
    expect(describeUndoStorage({ ...fixture, allocatedBytes: null }))
      .not.toContain('allocated')
    expect(describeUndoStorage({ ...fixture, incomplete: true, allocatedBytes: 4096 }))
      .toContain('Incomplete scan')
  })
  it('distinguishes missing storage and partial measurements from complete totals', () => {
    expect(describeUndoStorage(fixture)).toContain('Last scan: 8.2 kB')
    expect(describeUndoStorage(fixture)).toContain('1 session with recovery markers')
    expect(describeUndoStorage({ ...fixture, incomplete: true, logicalBytes: 0 }))
      .toContain('Incomplete scan — counted 0 B')
    expect(describeUndoStorage({ ...fixture, exists: false }))
      .toBe('No undo storage directory exists yet.')
  })

  it('suppresses repeated scans while busy and permits a later refresh', async () => {
    let finish!: (result: UndoStorageSummary) => void
    const inspect = vi.fn(() => new Promise<UndoStorageSummary>(resolve => { finish = resolve }))
    const vm = createUndoStorageModel(inspect)
    const pending = vm.refresh()
    await vm.refresh()
    expect(inspect).toHaveBeenCalledTimes(1)
    expect(get(vm.busy)).toBe(true)
    finish(fixture)
    await pending
    expect(get(vm.summary)).toEqual(fixture)
    expect(get(vm.busy)).toBe(false)
    const again = vm.refresh()
    finish({ ...fixture, markedSessions: 0 })
    await again
    expect(inspect).toHaveBeenCalledTimes(2)
    expect(get(vm.summary)?.markedSessions).toBe(0)
  })

  it('reports a scan failure without pretending that backups were removed or are empty', async () => {
    const inspect = vi.fn().mockResolvedValueOnce(fixture).mockRejectedValueOnce(new Error('Permission denied'))
    const vm = createUndoStorageModel(inspect)
    await vm.refresh()
    await vm.refresh()
    expect(get(vm.summary)).toEqual(fixture)
    expect(get(vm.error)).toBe('Could not inspect backups: Permission denied')
    expect(get(vm.busy)).toBe(false)
    inspect.mockResolvedValueOnce({ ...fixture, logicalBytes: 50 })
    await vm.refresh()
    expect(get(vm.error)).toBe('')
    expect(get(vm.summary)?.logicalBytes).toBe(50)
  })

  it.each(['success', 'failure'])('ignores a late %s after its settings section is destroyed', async outcome => {
    let finish!: (result: UndoStorageSummary) => void
    let fail!: (error: Error) => void
    const inspect = vi.fn(() => new Promise<UndoStorageSummary>((resolve, reject) => {
      finish = resolve
      fail = reject
    }))
    const vm = createUndoStorageModel(inspect)
    const pending = vm.refresh()
    vm.dispose()
    if (outcome === 'success') finish(fixture)
    else fail(new Error('Late failure'))
    await pending
    await vm.refresh()
    expect(get(vm.summary)).toBeNull()
    expect(get(vm.error)).toBe('')
    expect(get(vm.busy)).toBe(false)
    expect(inspect).toHaveBeenCalledTimes(1)
  })

  it('deletes once while busy and refreshes the summary with retained-session information', async () => {
    let finish!: (result: { deletedSessions: number; retainedSessions: number; errors: string[] }) => void
    const remove = vi.fn(() => new Promise<{ deletedSessions: number; retainedSessions: number; errors: string[] }>(resolve => { finish = resolve }))
    const inspect = vi.fn().mockResolvedValue({ ...fixture, sessions: 1 })
    const vm = createUndoStorageModel(inspect, remove)
    const pending = vm.deleteAll()
    expect(get(vm.deleting)).toBe(true)
    expect(await vm.deleteAll()).toBe(false)
    await vm.refresh()
    expect(inspect).not.toHaveBeenCalled()
    finish({ deletedSessions: 2, retainedSessions: 1, errors: ['One session could not be deleted'] })
    expect(await pending).toBe(true)
    expect(remove).toHaveBeenCalledTimes(1)
    expect(get(vm.message)).toContain('1 backup session was kept')
    expect(get(vm.error)).toContain('could not be deleted')
    expect(get(vm.summary)?.sessions).toBe(1)
    expect(get(vm.busy)).toBe(false)
  })

  it('keeps a deletion failure available for retry without reporting success', async () => {
    const remove = vi.fn().mockRejectedValueOnce(new Error('Backups are in use'))
      .mockResolvedValueOnce({ deletedSessions: 3, retainedSessions: 0, errors: [] })
    const vm = createUndoStorageModel(vi.fn().mockResolvedValue(fixture), remove)
    expect(await vm.deleteAll()).toBe(false)
    expect(get(vm.error)).toBe('Backups are in use')
    expect(get(vm.message)).toBe('')
    expect(await vm.deleteAll()).toBe(true)
    expect(get(vm.error)).toBe('')
    expect(get(vm.message)).toBe('Backups and undo history deleted.')
  })
})
