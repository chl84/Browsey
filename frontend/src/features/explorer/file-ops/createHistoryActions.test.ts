import { describe, expect, it, vi } from 'vitest'
import { createHistoryActions } from './createHistoryActions'

const fixture = () => {
  const deps = {
    undo: vi.fn(async () => {}),
    redo: vi.fn(async () => {}),
    refresh: vi.fn(async () => {}),
    showToast: vi.fn(),
  }
  return { deps, actions: createHistoryActions(deps) }
}

describe('history action recovery', () => {
  it.each(['undo', 'redo'] as const)('refreshes after failed %s and preserves recovery guidance without retrying', async (operation) => {
    const { deps, actions } = fixture()
    deps[operation].mockRejectedValue({ code: 'io_error', message: 'Recovery copy retained at /tmp/fixture-backup' })
    expect(await actions[operation]()).toBe(false)
    expect(deps[operation]).toHaveBeenCalledTimes(1)
    expect(deps.refresh).toHaveBeenCalledTimes(1)
    expect(deps.showToast.mock.calls[0][0]).toContain('/tmp/fixture-backup')
    expect(deps.showToast.mock.calls[0][0]).toContain('Inspect affected paths')
  })

  it('does not misreport successful undo as failed when only refresh fails', async () => {
    const { deps, actions } = fixture()
    deps.refresh.mockRejectedValue(new Error('listing unavailable'))
    expect(await actions.undo()).toBe(true)
    expect(deps.showToast.mock.calls[0][0]).toContain('Undo completed, but refresh failed')
    expect(deps.showToast.mock.calls[0][0]).toContain('Press F5')
    expect(deps.undo).toHaveBeenCalledTimes(1)
  })

  it('keeps both errors and releases the gate after an unsuccessful redo', async () => {
    const { deps, actions } = fixture()
    deps.redo.mockRejectedValueOnce(new Error('complete backup retained'))
    deps.refresh.mockRejectedValueOnce(new Error('listing unavailable'))
    expect(await actions.redo()).toBe(false)
    const message = deps.showToast.mock.calls[0][0]
    expect(message).toContain('complete backup retained')
    expect(message).toContain('Refresh also failed: listing unavailable')
    expect(message).toContain('Press F5')
    expect(await actions.redo()).toBe(true)
    expect(deps.redo).toHaveBeenCalledTimes(2)
  })

  it('does not queue repeat undo or redo while an operation or its refresh is pending', async () => {
    const { deps, actions } = fixture()
    let finishUndo!: () => void
    let finishRefresh!: () => void
    deps.undo.mockImplementationOnce(() => new Promise<void>((resolve) => { finishUndo = resolve }))
    deps.refresh.mockImplementationOnce(() => new Promise<void>((resolve) => { finishRefresh = resolve }))
    const pending = actions.undo()
    expect(await actions.undo()).toBe(true)
    expect(await actions.redo()).toBe(true)
    expect(deps.undo).toHaveBeenCalledTimes(1)
    expect(deps.redo).not.toHaveBeenCalled()
    finishUndo()
    await vi.waitFor(() => expect(deps.refresh).toHaveBeenCalledTimes(1))
    expect(await actions.redo()).toBe(true)
    expect(deps.redo).not.toHaveBeenCalled()
    finishRefresh()
    expect(await pending).toBe(true)
    expect(await actions.redo()).toBe(true)
    expect(deps.redo).toHaveBeenCalledTimes(1)
  })
})
