import { describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'
import { createUndoStorageModel, describeUndoStorage, describeUndoStorageOverview, type UndoStorageSummary } from './undoStorage'

const fixture: UndoStorageSummary = {
  directory: '/fixture/browsey/undo-sessions', exists: true, sessions: 3,
  markedSessions: 1, files: 4, logicalBytes: 8192, incomplete: false,
}

describe('undo storage diagnostics', () => {
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
    expect(get(vm.error)).toBe('Permission denied')
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
})
