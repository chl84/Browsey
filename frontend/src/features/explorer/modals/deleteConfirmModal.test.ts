import { get } from 'svelte/store'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createDeleteConfirmModal } from './deleteConfirmModal'
import type { Entry } from '../model/types'

const { deleteEntries, moveToTrashMany, purgeTrashItems } = vi.hoisted(() => ({
  deleteEntries: vi.fn(), moveToTrashMany: vi.fn(), purgeTrashItems: vi.fn(),
}))
vi.mock('../services/trash.service', () => ({ deleteEntries, moveToTrashMany, purgeTrashItems }))
const entry: Entry = { name: 'large.bin', path: '/mnt/share/large.bin', kind: 'file', iconId: 0, network: true }
const setup = () => {
  const activityApi = { start: vi.fn(), cleanup: vi.fn(), clearNow: vi.fn(), hasHideTimer: () => false, requestCancel: vi.fn() }
  const deps = { activityApi, reloadCurrent: vi.fn(), showToast: vi.fn() }
  return { ...deps, modal: createDeleteConfirmModal(deps) }
}
describe('network deletion confirmation', () => {
  beforeEach(() => vi.resetAllMocks())

  it('labels network deletion and mutates nothing until confirmed', () => {
    const { modal } = setup()
    modal.open([entry])
    expect(get(modal.state).mode).toBe('network')
    expect(deleteEntries).not.toHaveBeenCalled()
    modal.close()
    expect(deleteEntries).not.toHaveBeenCalled()
    expect(moveToTrashMany).not.toHaveBeenCalled()
  })

  it('approves permanent network deletion only from the open dialog', async () => {
    const { modal, activityApi } = setup()
    modal.open([entry])
    await modal.confirm()
    expect(deleteEntries).toHaveBeenCalledWith([entry.path], expect.stringMatching(/^delete-progress-/), true)
    expect(moveToTrashMany).not.toHaveBeenCalled()
    const [_, progressEvent, cancel] = activityApi.start.mock.calls[0]
    cancel()
    expect(activityApi.requestCancel).toHaveBeenCalledWith(progressEvent)
    await modal.confirm()
    expect(deleteEntries).toHaveBeenCalledTimes(1)
  })

  it('retains trash for supported targets when unsupported network trash is approved', async () => {
    const { modal } = setup()
    modal.open([entry], 'network-trash')
    await modal.confirm()
    expect(moveToTrashMany).toHaveBeenCalledWith([entry.path], expect.any(String), true)
    expect(deleteEntries).not.toHaveBeenCalled()
  })

  it('reports partial failures without retrying the batch', async () => {
    deleteEntries.mockRejectedValueOnce(new Error('1 of 2 items completed; cannot be rolled back'))
    const { modal, showToast } = setup()
    modal.open([entry])
    await modal.confirm()
    expect(showToast).toHaveBeenCalledWith(expect.stringContaining('1 of 2 items completed'))
    expect(deleteEntries).toHaveBeenCalledTimes(1)
    expect(get(modal.state).open).toBe(false)
  })
})
