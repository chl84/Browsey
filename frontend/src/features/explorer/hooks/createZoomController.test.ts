import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createZoomController } from './createZoomController'
import type { ZoomState } from './createWheelZoom'

describe('frame-coalesced zoom controller', () => {
  const frames = new Map<number, FrameRequestCallback>()
  let sequence = 0
  const frame = async () => {
    const callbacks = [...frames.values()]
    frames.clear()
    for (const callback of callbacks) callback(0)
    await Promise.resolve()
    await Promise.resolve()
  }
  const setup = (initial: ZoomState = { viewMode: 'list', size: 96 }) => {
    let state = initial, blocked = false
    const apply = vi.fn(async (next: ZoomState) => { state = next })
    const onError = vi.fn()
    const controller = createZoomController({ getState: () => state, apply, isBlocked: () => blocked, onError })
    return { controller, apply, onError, getState: () => state, block: () => { blocked = true } }
  }
  beforeEach(() => {
    frames.clear()
    sequence = 0
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.set(++sequence, callback)
      return sequence
    })
    vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id))
  })
  afterEach(() => vi.unstubAllGlobals())

  it('coalesces five rapid notches into the final grid size in one frame', async () => {
    const { controller, apply, getState } = setup()
    for (let i = 0; i < 5; i++) controller.step(1)
    expect(frames.size).toBe(1)
    expect(apply).not.toHaveBeenCalled()
    await frame()
    expect(apply).toHaveBeenCalledExactlyOnceWith({ viewMode: 'grid', size: 192 })
    expect(getState()).toEqual({ viewMode: 'grid', size: 192 })
    controller.destroy()
  })

  it('retains steps arriving during an asynchronous view switch', async () => {
    const { controller, apply } = setup()
    let finish!: () => void
    apply.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    controller.step(1)
    await frame()
    controller.step(1)
    controller.step(1)
    expect(frames.size).toBe(0)
    finish()
    await Promise.resolve()
    await frame()
    expect(apply.mock.calls.map(([state]) => state)).toEqual([
      { viewMode: 'grid', size: 64 }, { viewMode: 'grid', size: 128 },
    ])
    controller.destroy()
  })

  it('clamps each notch before reversing at either boundary', async () => {
    const { controller, apply } = setup({ viewMode: 'grid', size: 160 })
    for (let i = 0; i < 20; i++) controller.step(1)
    controller.step(-1)
    await frame()
    expect(apply).not.toHaveBeenCalled()
    for (let i = 0; i < 20; i++) controller.step(-1)
    controller.step(1)
    await frame()
    expect(apply).toHaveBeenCalledExactlyOnceWith({ viewMode: 'grid', size: 64 })
    controller.destroy()
  })

  it('honors opposite notches within the same frame', async () => {
    const { controller, apply } = setup({ viewMode: 'grid', size: 96 })
    controller.step(1)
    controller.step(-1)
    await frame()
    expect(apply).not.toHaveBeenCalled()
    controller.destroy()
  })

  it('discards queued work if a modal opens before the next frame', async () => {
    const { controller, apply, block } = setup()
    controller.step(1)
    block()
    await frame()
    expect(apply).not.toHaveBeenCalled()
    controller.step(1)
    expect(frames.size).toBe(0)
    controller.destroy()
  })

  it('cancels pending work on explicit view changes and teardown', async () => {
    const { controller, apply } = setup()
    controller.step(1)
    controller.clearPending()
    await frame()
    expect(apply).not.toHaveBeenCalled()
    controller.step(1)
    controller.destroy()
    controller.step(1)
    await frame()
    expect(apply).not.toHaveBeenCalled()
  })

  it('never schedules late work after teardown during an apply', async () => {
    const { controller, apply } = setup()
    let finish!: () => void
    apply.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    controller.step(1)
    await frame()
    controller.step(1)
    controller.destroy()
    finish()
    await Promise.resolve()
    expect(frames.size).toBe(0)
    expect(apply).toHaveBeenCalledTimes(1)
  })

  it('reports apply failures and allows a subsequent zoom', async () => {
    const { controller, apply, onError } = setup()
    const error = new Error('Layout failed')
    apply.mockRejectedValueOnce(error)
    controller.step(1)
    await frame()
    expect(onError).toHaveBeenCalledExactlyOnceWith(error)
    controller.step(1)
    await frame()
    expect(apply).toHaveBeenCalledTimes(2)
    controller.destroy()
  })
})
