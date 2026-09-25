import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createDragNavigation } from './createDragNavigation'

describe('drag auto-navigation', () => {
  const controllers: ReturnType<typeof createDragNavigation>[] = []
  const setup = () => {
    const element = document.createElement('div')
    element.setAttribute('data-drop-scroll', '')
    document.body.append(element)
    element.getBoundingClientRect = () => ({ top: 0, bottom: 300, left: 0, right: 200, height: 300 } as DOMRect)
    document.elementFromPoint = vi.fn(() => element)
    const options = { canNavigate: vi.fn(() => true), open: vi.fn(), onScroll: vi.fn() }
    const nav = createDragNavigation(options)
    controllers.push(nav)
    return { nav, element, options, target: { element, path: '/folder', openOnHover: true } }
  }
  beforeEach(() => { vi.useFakeTimers() })
  afterEach(() => {
    controllers.splice(0).forEach(nav => nav.stop())
    vi.clearAllTimers()
    vi.useRealTimers()
    document.body.innerHTML = ''
  })
  it('opens a stable target once after 850ms', async () => {
    const { nav, options, target } = setup()
    nav.update({ x: 100, y: 150 }, target)
    await vi.advanceTimersByTimeAsync(600)
    nav.update({ x: 101, y: 150 }, target)
    await vi.advanceTimersByTimeAsync(249)
    expect(options.open).not.toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(1000)
    expect(options.open).toHaveBeenCalledExactlyOnceWith('/folder')
  })
  it('cancels hover when the target changes or a modal opens', async () => {
    const { nav, options, target } = setup()
    nav.update({ x: 100, y: 150 }, target)
    await vi.advanceTimersByTimeAsync(600)
    nav.update({ x: 100, y: 150 }, null)
    await vi.advanceTimersByTimeAsync(300)
    expect(options.open).not.toHaveBeenCalled()
    nav.update({ x: 100, y: 150 }, target)
    options.canNavigate.mockReturnValue(false)
    await vi.advanceTimersByTimeAsync(900)
    expect(options.open).not.toHaveBeenCalled()
  })
  it('scrolls near edges, rechecks targets, and stops cleanly', async () => {
    const { nav, element, options } = setup()
    nav.update({ x: 100, y: 295 }, null)
    await vi.advanceTimersByTimeAsync(100)
    expect(element.scrollTop).toBeGreaterThan(0)
    expect(options.onScroll).toHaveBeenCalled()
    nav.stop()
    const stoppedAt = element.scrollTop
    await vi.advanceTimersByTimeAsync(100)
    expect(element.scrollTop).toBe(stoppedAt)
    nav.update({ x: 100, y: 5 }, null)
    await vi.advanceTimersByTimeAsync(50)
    expect(element.scrollTop).toBeLessThan(stoppedAt)
  })
  it('does not scroll in the middle or outside the scroll surface', async () => {
    const { nav, element } = setup()
    nav.update({ x: 100, y: 150 }, null)
    await vi.advanceTimersByTimeAsync(100)
    expect(element.scrollTop).toBe(0)
    nav.update({ x: 250, y: 299 }, null)
    await vi.advanceTimersByTimeAsync(100)
    expect(element.scrollTop).toBe(0)
  })
})
