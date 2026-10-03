import { describe, expect, it, vi } from 'vitest'
import { get } from 'svelte/store'
import { createEmptyTrashModal } from './emptyTrashModal'

const setup = () => {
  const deps = {
    emptyTrash: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    isTrashView: vi.fn().mockReturnValue(true),
    refresh: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
    showToast: vi.fn(),
  }
  return { deps, modal: createEmptyTrashModal(deps) }
}

describe('empty Wastebasket confirmation', () => {
  it('does not delete before confirmation or after cancellation', async () => {
    const { deps, modal } = setup()
    await modal.confirm()
    modal.open()
    expect(deps.emptyTrash).not.toHaveBeenCalled()
    modal.close()
    await modal.confirm()
    expect(deps.emptyTrash).not.toHaveBeenCalled()
  })

  it('locks confirmation and cancellation until the single operation finishes', async () => {
    const { deps, modal } = setup()
    let finish!: () => void
    deps.emptyTrash.mockReturnValue(new Promise<void>(resolve => { finish = resolve }))
    modal.open()
    const running = modal.confirm()
    await modal.confirm()
    modal.close()
    modal.open()
    expect(get(modal.state)).toEqual({ open: true, busy: true })
    expect(deps.emptyTrash).toHaveBeenCalledTimes(1)
    finish()
    await running
    expect(get(modal.state)).toEqual({ open: false, busy: false })
    expect(deps.refresh).toHaveBeenCalledTimes(1)
    expect(deps.showToast).toHaveBeenCalledWith('Wastebasket emptied')
  })

  it('refreshes partial failure and reports the original error without retrying', async () => {
    const { deps, modal } = setup()
    deps.emptyTrash.mockRejectedValue({ code: 'trash_failed', message: 'permission denied' })
    deps.refresh.mockRejectedValue(new Error('refresh failed'))
    modal.open()
    await modal.confirm()
    expect(deps.emptyTrash).toHaveBeenCalledTimes(1)
    expect(deps.refresh).toHaveBeenCalledTimes(1)
    expect(deps.showToast.mock.calls[0][0]).toContain('permission denied')
    expect(deps.showToast.mock.calls[0][0]).toContain('Some items may already have been permanently deleted')
    expect(deps.showToast.mock.calls[0][0]).toContain('Press F5')
    expect(get(modal.state).busy).toBe(false)
  })

  it('does not mislabel completed deletion when refresh fails', async () => {
    const { deps, modal } = setup()
    deps.refresh.mockRejectedValue(new Error('refresh failed'))
    modal.open()
    await modal.confirm()
    expect(deps.showToast.mock.calls[0][0]).toContain('Wastebasket emptied')
    expect(deps.showToast.mock.calls[0][0]).not.toContain('Could not empty')
  })

  it('does not refresh or navigate from a non-trash view', async () => {
    const { deps, modal } = setup()
    let finish!: () => void
    deps.emptyTrash.mockReturnValue(new Promise<void>(resolve => { finish = resolve }))
    modal.open()
    const running = modal.confirm()
    deps.isTrashView.mockReturnValue(false)
    finish()
    await running
    expect(deps.refresh).not.toHaveBeenCalled()
  })
})
