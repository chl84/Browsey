import { afterEach, describe, expect, it, vi } from 'vitest'
import { createWheelZoom, nextZoomState, GRID_ZOOM_SIZES } from './createWheelZoom'

const wheel = (deltaY: number, options: WheelEventInit = {}) => new WheelEvent('wheel', {
  ctrlKey: true, cancelable: true, deltaY, ...options,
})

describe('explorer wheel zoom', () => {
  afterEach(() => vi.restoreAllMocks())

  it('steps from list through five sizes and back without exceeding bounds', () => {
    let state: { viewMode: 'list' | 'grid'; size: number } = { viewMode: 'list', size: 96 }
    expect(nextZoomState(state.viewMode, state.size, -1)).toEqual(state)
    for (const size of GRID_ZOOM_SIZES) {
      state = nextZoomState(state.viewMode, state.size, 1)
      expect(state).toEqual({ viewMode: 'grid', size })
    }
    expect(nextZoomState('grid', 192, 1)).toEqual(state)
    for (const size of [...GRID_ZOOM_SIZES].reverse().slice(1)) {
      state = nextZoomState(state.viewMode, state.size, -1)
      expect(state).toEqual({ viewMode: 'grid', size })
    }
    expect(nextZoomState('grid', 64, -1)).toEqual({ viewMode: 'list', size: 64 })
  })

  it('leaves normal wheel scrolling untouched', () => {
    const step = vi.fn(), handle = createWheelZoom(step)
    const event = wheel(-120, { ctrlKey: false })
    expect(handle(event)).toBe(false)
    expect(event.defaultPrevented).toBe(false)
    expect(step).not.toHaveBeenCalled()
  })

  it('consumes Ctrl-wheel and retains every deliberate notch in a rapid burst', () => {
    let now = 0
    vi.spyOn(performance, 'now').mockImplementation(() => now)
    const step = vi.fn(), handle = createWheelZoom(step)
    const event = wheel(-120)
    expect(handle(event)).toBe(true)
    expect(event.defaultPrevented).toBe(true)
    handle(wheel(-120))
    expect(step).toHaveBeenCalledTimes(2)
    now = 100
    handle(wheel(-120))
    expect(step).toHaveBeenCalledTimes(3)
    handle(wheel(120))
    expect(step).toHaveBeenLastCalledWith(-1)
  })

  it.each([0, 16, 30, 40, 60])('does not discard notches %i ms apart', interval => {
    let now = 0
    vi.spyOn(performance, 'now').mockImplementation(() => now)
    const step = vi.fn(), handle = createWheelZoom(step)
    for (let i = 0; i < 10; i++) {
      now = i * interval
      handle(wheel(-120))
    }
    expect(step).toHaveBeenCalledTimes(10)
  })

  it('does not carry partial trackpad movement across a pause or a modal', () => {
    let now = 0, blocked = false
    vi.spyOn(performance, 'now').mockImplementation(() => now)
    const step = vi.fn(), handle = createWheelZoom(step, () => blocked)
    handle(wheel(-30))
    now = 200
    handle(wheel(-30))
    expect(step).not.toHaveBeenCalled()
    blocked = true
    handle(wheel(-120))
    blocked = false
    handle(wheel(-30))
    expect(step).not.toHaveBeenCalled()
    handle(wheel(-20))
    expect(step).toHaveBeenCalledExactlyOnceWith(1)
  })

  it('accumulates small trackpad deltas and resets on direction change', () => {
    const step = vi.fn(), handle = createWheelZoom(step)
    handle(wheel(-20)); handle(wheel(-20))
    expect(step).not.toHaveBeenCalled()
    handle(wheel(20)); handle(wheel(20))
    expect(step).not.toHaveBeenCalled()
    handle(wheel(20))
    expect(step).toHaveBeenCalledExactlyOnceWith(-1)
  })

  it.each([1, 2])('normalizes wheel delta mode %i', deltaMode => {
    const step = vi.fn(), handle = createWheelZoom(step)
    handle(wheel(deltaMode === 1 ? -3 : -1, { deltaMode }))
    expect(step).toHaveBeenCalledExactlyOnceWith(1)
  })

  it('does not zoom the listing behind a modal or on horizontal wheel input', () => {
    const step = vi.fn(), handle = createWheelZoom(step, () => true)
    const event = wheel(-120)
    handle(event)
    expect(event.defaultPrevented).toBe(true)
    expect(step).not.toHaveBeenCalled()
    createWheelZoom(step)(wheel(0, { deltaX: 100 }))
    expect(step).not.toHaveBeenCalled()
  })
})
